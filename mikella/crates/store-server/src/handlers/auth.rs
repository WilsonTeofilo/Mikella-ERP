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
use rusqlite::OptionalExtension;
use uuid::Uuid;

use mikella_domain_people::{Papel, Usuario};
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
    match db.conn().query_row(
        "SELECT COUNT(*) FROM instance_identity WHERE id = 1",
        [],
        |row| row.get::<_, i64>(0),
    ) {
        Ok(onboarded) => (
            StatusCode::OK,
            Json(OnboardingStatus {
                onboarded: onboarded > 0,
            }),
        ),
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

        let ja_onboarded: i64 = match db.conn().query_row(
            "SELECT COUNT(*) FROM instance_identity WHERE id = 1",
            [],
            |row| row.get(0),
        ) {
            Ok(n) => n,
            Err(e) => {
                tracing::error!(%e, "falha ao consultar instance_identity");
                return erro(StatusCode::INTERNAL_SERVER_ERROR, "erro interno");
            }
        };
        if ja_onboarded > 0 {
            return erro(StatusCode::CONFLICT, "onboarding já realizado");
        }

        let tx = match db.transacao() {
            Ok(tx) => tx,
            Err(e) => {
                tracing::error!(%e, "falha ao abrir transação de onboarding");
                return erro(StatusCode::INTERNAL_SERVER_ERROR, "erro interno");
            }
        };

        let resultado = (|| -> Result<(), rusqlite::Error> {
            tx.execute(
                "INSERT INTO instance_identity (id, account_id, store_id, db_epoch, manifest_json)
                 VALUES (1, ?1, ?2, 1, '{}')",
                rusqlite::params![&account_id, &store_id],
            )?;
            tx.execute(
                "INSERT INTO usuarios (id, nome, username, senha_hash, email, papel, ativo)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'DONO', 1)",
                rusqlite::params![
                    usuario.id.to_string(),
                    &usuario.nome,
                    &usuario.username,
                    &usuario.senha_hash,
                    usuario.email.as_deref(),
                ],
            )?;
            Ok(())
        })();

        if let Err(e) = resultado {
            tracing::error!(%e, "falha ao gravar onboarding");
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
        match db.conn().query_row(
            "SELECT id, nome, username, email, senha_hash, papel, pin_hash, ativo,
                    created_at, updated_at
             FROM usuarios WHERE username = ?1 COLLATE NOCASE AND ativo = 1",
            [&username],
            usuario_da_linha,
        ).optional() {
            Ok(Some(u)) => u,
            Ok(None) => {
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
        match db
            .conn()
            .query_row(
                "SELECT id, nome, username, email, senha_hash, papel, pin_hash, ativo,
                        created_at, updated_at
                 FROM usuarios WHERE username = ?1 COLLATE NOCASE AND ativo = 1",
                [username.trim()],
                usuario_da_linha,
            )
            .optional()
        {
            Ok(opt) => opt,
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

    match enviar_codigo_2fa(state, &usuario).await {
        Ok(codigo) => {
            let mut corpo = serde_json::json!({
                "sucesso": true,
                "token_pending": true,
                "username": usuario.username,
            });
            if state.email.disponivel() {
                corpo["codigo_debug"] = serde_json::Value::String(codigo);
            }
            (StatusCode::OK, Json(corpo))
        }
        Err(resposta) => resposta,
    }
}

/// Gera, armazena (hash) e envia um código 2FA para o usuário.
async fn enviar_codigo_2fa(
    state: &AppState,
    usuario: &Usuario,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
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
    Ok(codigo)
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

/// Lê uma linha de `usuarios` no formato usado pelo login.
fn usuario_da_linha(row: &rusqlite::Row<'_>) -> rusqlite::Result<Usuario> {
    let papel_str: String = row.get(5)?;
    let papel = Papel::from_db(&papel_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(Usuario {
        id: UsuarioId::from_uuid(
            row.get::<_, String>(0)?
                .parse()
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
        ),
        nome: row.get(1)?,
        username: row.get(2)?,
        email: row.get(3)?,
        senha_hash: row.get(4)?,
        papel,
        pin_hash: row.get(6)?,
        capacidades_extras: Vec::new(),
        ativo: row.get::<_, i64>(7)? != 0,
        criado_em: parse_data_hora(row.get::<_, String>(8)?),
        atualizado_em: parse_data_hora(row.get::<_, String>(9)?),
    })
}

fn parse_data_hora(valor: String) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(&valor)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

fn erro(
    status: StatusCode,
    mensagem: &str,
) -> (StatusCode, Json<serde_json::Value>) {
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
}
