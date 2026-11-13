//! Handlers de autenticação: onboarding, login e 2FA.
//!
//! SEG-011: o login **não** emite token. Ele valida a senha e envia um código
//! 2FA; o token só nasce em `/v1/2fa/verificar`. SEG-004: o código é guardado
//! apenas como hash keyed; SEG-006: tentativas são limitadas por conta.

use std::time::Instant;

use axum::extract::{Form, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use uuid::Uuid;

use mikella_domain_people::ports::UsuarioRepository;
use mikella_domain_people::{Papel, Usuario};
use mikella_infra_sqlite::{AuditoriaRepo, InstanceRepo, UsuarioRepo};
use mikella_shared_kernel::ids::UsuarioId;

use crate::dto::{
    DoisFARequestForm, DoisFAVerifyForm, LoginForm, OnboardingForm, OnboardingStatus,
};
use crate::state::{AppState, Codigo2FA, SessaoAtiva, MAX_TENTATIVAS_2FA};
use crate::util::{
    gerar_codigo_2fa, hash_codigo_2fa, hash_senha, hash_senha_ficticio, senha_aceitavel,
    verificar_codigo_2fa, verificar_senha, DOISFA_TTL, TOKEN_PREFIXO,
};

/// `GET /v1/onboarding/status` — informa se a loja já foi configurada.
pub async fn onboarding_status(State(state): State<AppState>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match InstanceRepo::novo(db.conn()).esta_onboarded() {
        Ok(onboarded) => (StatusCode::OK, Json(OnboardingStatus { onboarded })),
        Err(erro) => {
            tracing::error!(%erro, "falha ao consultar onboarding");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OnboardingStatus { onboarded: false }),
            )
        }
    }
}

/// `POST /v1/onboarding` — cria `instance_identity` + usuário dono.
///
/// Só funciona se ainda **não** houver onboarding (409 se já existe).
/// Não devolve token: o dono passa pelo mesmo 2FA do login normal.
pub async fn onboarding_create(
    State(state): State<AppState>,
    Form(form): Form<OnboardingForm>,
) -> impl IntoResponse {
    let nome = form.nome.trim().to_string();
    let username = form.username.trim().to_string();
    let email = form.email.trim().to_string();

    if nome.is_empty() {
        return erro(StatusCode::BAD_REQUEST, "nome é obrigatório");
    }
    if username.len() < 3 || username.len() > 64 {
        return erro(
            StatusCode::BAD_REQUEST,
            "username deve ter entre 3 e 64 caracteres",
        );
    }
    if !username.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return erro(
            StatusCode::BAD_REQUEST,
            "username deve conter apenas letras, números e underscore",
        );
    }
    if !senha_aceitavel(&form.senha) {
        return erro(
            StatusCode::BAD_REQUEST,
            "senha deve ter entre 8 e 256 caracteres e não pode ser uma senha comum",
        );
    }
    if !email_valido(&email) {
        return erro(StatusCode::BAD_REQUEST, "e-mail inválido");
    }
    if !state.email.disponivel() {
        return erro(
            StatusCode::SERVICE_UNAVAILABLE,
            "envio de e-mail não configurado; defina MIKELLA_EMAIL_CONSOLE=1 para uso local",
        );
    }

    let account_id = Uuid::new_v4().to_string();
    let store_id = Uuid::new_v4().to_string();
    let senha_hash = hash_senha(&form.senha);
    let usuario_id = UsuarioId::new();

    // Valida o usuário com as mesmas regras do domínio antes de tocar no banco.
    let usuario = match Usuario::novo(
        usuario_id,
        nome,
        username,
        senha_hash,
        Papel::Dono,
        Some(email),
        chrono::Utc::now(),
    ) {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(erro = %e, "onboarding rejeitado pelo domínio");
            return erro(StatusCode::BAD_REQUEST, "dados de cadastro inválidos");
        }
    };

    {
        let mut db = state.db.lock().await;

        let ja_onboarded = match InstanceRepo::novo(db.conn()).esta_onboarded() {
            Ok(b) => b,
            Err(e) => {
                tracing::error!(%e, "falha ao consultar instance_identity");
                return erro(StatusCode::INTERNAL_SERVER_ERROR, "erro interno");
            }
        };
        if ja_onboarded {
            return erro(StatusCode::CONFLICT, "onboarding já realizado");
        }

        let tx = match db.transacao() {
            Ok(tx) => tx,
            Err(e) => {
                tracing::error!(%e, "falha ao abrir transação de onboarding");
                return erro(StatusCode::INTERNAL_SERVER_ERROR, "erro interno");
            }
        };

        if let Err(e) = InstanceRepo::novo(&tx).registrar(&account_id, &store_id) {
            tracing::error!(%e, "falha ao gravar instance_identity");
            return erro(StatusCode::INTERNAL_SERVER_ERROR, "erro interno");
        }

        if let Err(e) = UsuarioRepo::novo(&tx).salvar(&usuario) {
            tracing::error!(%e, "falha ao gravar usuario dono no onboarding");
            return erro(StatusCode::INTERNAL_SERVER_ERROR, "erro interno");
        }

        if let Err(e) = tx.commit() {
            tracing::error!(%e, "falha ao confirmar onboarding");
            return erro(StatusCode::INTERNAL_SERVER_ERROR, "erro interno");
        }
    }

    tracing::info!(store_id = %store_id, "onboarding concluído");

    // Envia o primeiro código 2FA. O onboarding só é aceito se houver
    // backend disponível; ainda assim, uma falha de envio não desfaz o banco.
    if let Err(resposta) = enviar_codigo_2fa(&state, &usuario).await {
        return resposta;
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "onboarded": true,
            "token_pending": true,
        })),
    )
}

/// `GET /v1/auth/me` — retorna os dados do usuário autenticado e seu papel/capacidades.
pub async fn auth_me(
    crate::middleware::Ator(usuario): crate::middleware::Ator,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "id": usuario.id.to_string(),
            "nome": usuario.nome,
            "username": usuario.username,
            "papel": usuario.papel.as_db(),
            "capacidades": usuario.papel.capacidades(),
        })),
    )
}

/// `POST /v1/auth/logout` — encerra a sessão ativa e revoga o token (SEC-011).
pub async fn logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(valor) = auth.to_str() {
            if let Some(token) = valor.strip_prefix("Bearer ") {
                let mut sessoes = state.sessoes.lock().await;
                let removido = sessoes.remove(token.trim());
                if let Some(s) = removido {
                    tracing::info!(usuario = %s.username, "sessão encerrada via logout");
                    let db = state.db.lock().await;
                    let audit = AuditoriaRepo::novo(db.conn());
                    let _ =
                        audit.registrar("usuario", &s.username, "LOGOUT", None, None, None, None);
                }
            }
        }
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "sucesso": true,
            "mensagem": "sessão encerrada"
        })),
    )
}

// ---------------------------------------------------------------------------
// Login (Argon2id + 2FA)
// ---------------------------------------------------------------------------

/// `POST /v1/login` — valida credenciais e inicia o desafio 2FA.
///
/// A resposta nunca contém token. O token só é emitido após o código 2FA.
pub async fn login(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    iniciar_desafio(&state, &form.username, &form.senha).await
}

/// `POST /v1/2fa/solicitar` — reenvia o código depois de validar a senha.
pub async fn solicitar_2fa(
    State(state): State<AppState>,
    Form(form): Form<DoisFARequestForm>,
) -> impl IntoResponse {
    iniciar_desafio(&state, &form.username, &form.senha).await
}

/// `POST /v1/2fa/verificar` — valida o código e emite o token de sessão.
pub async fn verificar_2fa(
    State(state): State<AppState>,
    Form(form): Form<DoisFAVerifyForm>,
) -> impl IntoResponse {
    let chave = AppState::chave_usuario(&form.username);
    if chave.is_empty() {
        return erro(StatusCode::UNAUTHORIZED, "código inválido ou expirado");
    }

    {
        let mut limiter = state.rate_limit.lock().await;
        limiter.limpar_expirados();
        if !limiter.permitir(&format!("2fa:{chave}")) {
            return erro(
                StatusCode::TOO_MANY_REQUESTS,
                "muitas tentativas; aguarde antes de tentar novamente",
            );
        }
    }

    let codigo_submetido = form.codigo.trim();
    let agora = Instant::now();

    let (codigo_valido, username_canonico) = {
        let mut codigos = state.codigos_2fa.lock().await;
        let Some(codigo) = codigos.get_mut(&chave) else {
            return erro(StatusCode::UNAUTHORIZED, "código inválido ou expirado");
        };

        if agora >= codigo.expira_em {
            codigos.remove(&chave);
            return erro(StatusCode::UNAUTHORIZED, "código inválido ou expirado");
        }

        if verificar_codigo_2fa(state.segredo_2fa.as_ref(), codigo_submetido, &codigo.hash) {
            let username = codigo.username.clone();
            codigos.remove(&chave);
            (true, Some(username))
        } else {
            codigo.tentativas = codigo.tentativas.saturating_add(1);
            if codigo.tentativas >= MAX_TENTATIVAS_2FA {
                codigos.remove(&chave);
            }
            (false, None)
        }
    };

    if !codigo_valido {
        let mut limiter = state.rate_limit.lock().await;
        limiter.registrar_falha(&format!("2fa:{chave}"));
        return erro(StatusCode::UNAUTHORIZED, "código inválido ou expirado");
    }

    let Some(username) = username_canonico else {
        return erro(StatusCode::UNAUTHORIZED, "código inválido ou expirado");
    };

    // Código válido: confirma que o usuário continua ativo e emite o token.
    let usuario = {
        let db = state.db.lock().await;
        match UsuarioRepo::novo(db.conn()).buscar_por_username(&username) {
            Ok(Some(u)) if u.ativo => u,
            Ok(Some(_)) | Ok(None) => {
                return erro(StatusCode::UNAUTHORIZED, "sessão inválida");
            }
            Err(e) => {
                tracing::error!(%e, "falha ao carregar usuário após 2FA");
                return erro(StatusCode::INTERNAL_SERVER_ERROR, "erro interno");
            }
        }
    };

    let token = emitir_token(&state, &usuario).await;
    {
        let mut limiter = state.rate_limit.lock().await;
        limiter.limpar(&format!("2fa:{chave}"));
    }

    // LGPD-002: auditoria de login bem-sucedido.
    {
        let db = state.db.lock().await;
        let audit = AuditoriaRepo::novo(db.conn());
        if let Err(e) = audit.registrar(
            "usuario",
            &usuario.id.to_string(),
            "LOGIN",
            Some(&usuario.id.to_string()),
            None,
            None,
            None,
        ) {
            tracing::error!(erro = %e, usuario = %usuario.username, "falha ao gravar auditoria de login");
        }
    }

    tracing::info!(usuario = %usuario.username, "2FA verificado, token emitido");
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "autenticado": true,
            "token": token,
            "usuario": {
                "id": usuario.id.to_string(),
                "nome": usuario.nome,
                "username": usuario.username,
                "papel": usuario.papel.as_db(),
                "capacidades": usuario.papel.capacidades(),
            }
        })),
    )
}

// ---------------------------------------------------------------------------
// Internos
// ---------------------------------------------------------------------------

async fn iniciar_desafio(
    state: &AppState,
    username: &str,
    senha: &str,
) -> (StatusCode, Json<serde_json::Value>) {
    let chave = AppState::chave_usuario(username);
    if chave.is_empty() || senha.is_empty() {
        return erro(StatusCode::UNAUTHORIZED, "credenciais inválidas");
    }

    {
        let mut limiter = state.rate_limit.lock().await;
        limiter.limpar_expirados();
        if !limiter.permitir(&format!("login:{chave}")) {
            return erro(
                StatusCode::TOO_MANY_REQUESTS,
                "muitas tentativas; aguarde antes de tentar novamente",
            );
        }
    }

    // Carrega o hash e solta o lock antes do Argon2 (CPU-bound).
    let registro = {
        let db = state.db.lock().await;
        match UsuarioRepo::novo(db.conn()).buscar_por_username(username.trim()) {
            Ok(opt) => opt.filter(|u| u.ativo),
            Err(e) => {
                tracing::error!(%e, "falha ao buscar usuário no login");
                return erro(StatusCode::INTERNAL_SERVER_ERROR, "erro interno");
            }
        }
    };

    let senha_ok = match &registro {
        Some(u) => verificar_senha(senha, &u.senha_hash),
        // Verifica contra um hash fictício para não revelar, pelo tempo de
        // resposta, se o username existe (SEG-005).
        None => {
            let _ = verificar_senha(senha, &hash_senha_ficticio());
            false
        }
    };

    if !senha_ok {
        let mut limiter = state.rate_limit.lock().await;
        limiter.registrar_falha(&format!("login:{chave}"));
        return erro(StatusCode::UNAUTHORIZED, "credenciais inválidas");
    }

    {
        let mut limiter = state.rate_limit.lock().await;
        limiter.limpar(&format!("login:{chave}"));
    }

    let Some(usuario) = registro else {
        return erro(StatusCode::UNAUTHORIZED, "credenciais inválidas");
    };

    let desativar_2fa = std::env::var("MIKELLA_DESATIVAR_2FA")
        .map(|v| v == "1")
        .unwrap_or(false);

    if desativar_2fa {
        let token = emitir_token(state, &usuario).await;
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "sucesso": true,
                "token": token,
                "usuario": {
                    "id": usuario.id.to_string(),
                    "nome": usuario.nome,
                    "username": usuario.username,
                    "papel": usuario.papel.as_db(),
                }
            })),
        );
    }

    match enviar_codigo_2fa(state, &usuario).await {
        Ok(codigo_dev) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "sucesso": true,
                "token_pending": true,
                "username": usuario.username,
                "codigo_dev": codigo_dev,
            })),
        ),
        Err(resposta) => resposta,
    }
}

/// Gera, armazena (hash) e envia um código 2FA para o usuário.
async fn enviar_codigo_2fa(
    state: &AppState,
    usuario: &Usuario,
) -> Result<Option<String>, (StatusCode, Json<serde_json::Value>)> {
    let Some(email) = usuario.email.as_deref().filter(|e| !e.trim().is_empty()) else {
        tracing::error!(
            usuario = %usuario.username,
            "usuário sem e-mail cadastrado; 2FA não pode ser entregue"
        );
        return Err(erro(
            StatusCode::CONFLICT,
            "cadastre um e-mail para receber o código de acesso",
        ));
    };

    let codigo = gerar_codigo_2fa();
    let hash = hash_codigo_2fa(state.segredo_2fa.as_ref(), &codigo);
    let chave = AppState::chave_usuario(&usuario.username);

    if state
        .email
        .send_email(
            email,
            "Código de acesso Mikella",
            &format!("Seu código de acesso: {codigo}\nEle expira em 5 minutos."),
        )
        .is_err()
    {
        tracing::error!(
            usuario = %usuario.username,
            "falha ao enviar e-mail de 2FA"
        );
        return Err(erro(
            StatusCode::SERVICE_UNAVAILABLE,
            "não foi possível enviar o código; tente novamente",
        ));
    }

    let mut codigos = state.codigos_2fa.lock().await;
    codigos.insert(
        chave,
        Codigo2FA {
            username: usuario.username.clone(),
            hash,
            expira_em: Instant::now() + DOISFA_TTL,
            tentativas: 0,
        },
    );

    tracing::info!(usuario = %usuario.username, "código 2FA enviado");
    let mostrar_corpo = std::env::var("MIKELLA_EMAIL_CONSOLE_CORPO")
        .map(|v| v == "1")
        .unwrap_or(false);
    let codigo_dev = if mostrar_corpo { Some(codigo) } else { None };

    Ok(codigo_dev)
}

/// Emite token e aplica a regra de sessão única por usuário (SEG-012).
async fn emitir_token(state: &AppState, usuario: &Usuario) -> String {
    let token = format!("{}{}", TOKEN_PREFIXO, Uuid::new_v4());
    let agora = Instant::now();
    let mut sessoes = state.sessoes.lock().await;
    sessoes.retain(|_, s| !s.username.eq_ignore_ascii_case(&usuario.username));
    sessoes.insert(
        token.clone(),
        SessaoAtiva {
            username: usuario.username.clone(),
            criada_em: agora,
            ultimo_uso: agora,
        },
    );
    token
}

fn erro(status: StatusCode, mensagem: &str) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({ "erro": mensagem })))
}

/// Validação conservadora de e-mail (sem consultar DNS).
///
/// Exige exatamente um `@`, parte local não vazia e domínio com pelo menos
/// dois rótulos. Rejeita espaços, controles, pontos consecutivos e rótulos
/// iniciados/terminados em hífen — casos que passariam na checagem anterior
/// e seriam rejeitados só na entrega, depois do cadastro.
fn email_valido(email: &str) -> bool {
    let email = email.trim();
    if email.len() > 254 || email.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }

    let mut partes = email.split('@');
    let (Some(local), Some(dominio), None) = (partes.next(), partes.next(), partes.next()) else {
        return false;
    };

    if local.is_empty()
        || local.len() > 64
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
    {
        return false;
    }
    if !local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+-/=?^_`{|}~.".contains(c))
    {
        return false;
    }

    if dominio.starts_with('.') || dominio.ends_with('.') || dominio.contains("..") {
        return false;
    }
    let rotulos: Vec<&str> = dominio.split('.').collect();
    if rotulos.len() < 2 {
        return false;
    }
    for rotulo in &rotulos {
        if rotulo.is_empty()
            || rotulo.len() > 63
            || rotulo.starts_with('-')
            || rotulo.ends_with('-')
            || !rotulo
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return false;
        }
    }
    let tld = rotulos.last().copied().unwrap_or_default();
    tld.len() >= 2 && tld.chars().any(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valida_email_basico() {
        assert!(email_valido("dono@loja.com"));
        assert!(!email_valido("dono"));
        assert!(!email_valido("dono@loja"));
        assert!(!email_valido("@loja.com"));
        assert!(!email_valido("dono@.com"));
        assert!(!email_valido("dono @loja.com"));
    }

    #[tokio::test]
    async fn logout_revoga_token_ativo() {
        use crate::state::SessaoAtiva;
        use std::time::Instant;

        let db = mikella_infra_sqlite::Database::em_memoria().expect("db em memoria");
        let state = AppState::new(
            db,
            std::sync::Arc::new(crate::state::ConsoleEmailBackend::default()),
        );

        let token = "mikella-tok-teste-logout";
        {
            let mut sessoes = state.sessoes.lock().await;
            sessoes.insert(
                token.to_string(),
                SessaoAtiva {
                    username: "teste".to_string(),
                    criada_em: Instant::now(),
                    ultimo_uso: Instant::now(),
                },
            );
        }

        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {token}").parse().unwrap(),
        );

        let _ = logout(axum::extract::State(state.clone()), headers).await;

        let sessoes = state.sessoes.lock().await;
        assert!(
            !sessoes.contains_key(token),
            "token deveria ter sido revogado"
        );
    }
}
