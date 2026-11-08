//! Aprovação gerencial para operações sensíveis (SEC-004/SEC-006).
//!
//! Regra única para estorno de venda e sangria de caixa:
//! - Dono/Gerente (`nível >= 2`) aprova a própria operação;
//! - Operador precisa de usuário e senha de **outro** usuário gerencial;
//! - a senha é verificada com Argon2id e rate limit por username;
//! - a resposta nunca revela se o username existe ou se a senha está errada.
//!
//! O aprovador devolvido é gravado no livro-caixa e no comprovante.

use axum::http::StatusCode;
use serde::Deserialize;

use mikella_domain_people::ports::UsuarioRepository;
use mikella_shared_kernel::ids::UsuarioId;

use crate::middleware::{Ator, ErroHttp};
use crate::state::AppState;
use crate::util::{hash_senha_ficticio, verificar_senha};

/// Credenciais opcionais enviadas pelo operador para chamar um gerente.
#[derive(Debug, Clone, Deserialize)]
pub struct CredenciaisAprovador {
    pub username: String,
    pub senha: String,
}

/// Identidade de quem autorizou a operação.
#[derive(Debug, Clone)]
pub struct Aprovacao {
    pub id: UsuarioId,
    pub nome: String,
    pub papel: String,
}

fn nao_autorizado() -> ErroHttp {
    (
        StatusCode::UNAUTHORIZED,
        axum::Json(serde_json::json!({ "erro": "credenciais de autorização inválidas" })),
    )
}

fn proibido(msg: &str) -> ErroHttp {
    (
        StatusCode::FORBIDDEN,
        axum::Json(serde_json::json!({ "erro": msg })),
    )
}

/// Resolve o aprovador da operação.
///
/// `finalidade` entra na chave de rate limit (`estorno`, `sangria`) para que
/// tentativas em uma operação não bloqueiem a outra.
pub async fn exigir_aprovador(
    state: &AppState,
    ator: &Ator,
    credenciais: Option<CredenciaisAprovador>,
    finalidade: &str,
) -> Result<Aprovacao, ErroHttp> {
    // Gerente/Dono não precisa pedir senha a ninguém: ele é a autoridade.
    if ator.papel().nivel() >= 2 {
        return Ok(Aprovacao {
            id: ator.id(),
            nome: ator.0.nome.clone(),
            papel: ator.papel().as_db().to_string(),
        });
    }

    let Some(cred) = credenciais else {
        return Err(proibido(
            "operação exige autorização de gerente ou dono (informe usuário e senha)",
        ));
    };

    let username = cred.username.trim();
    if username.is_empty() || cred.senha.is_empty() {
        return Err(crate::util::bad_request(
            "usuário e senha de autorização são obrigatórios",
        ));
    }

    let chave = format!(
        "aprovacao:{finalidade}:{}",
        AppState::chave_usuario(username)
    );
    {
        let mut rl = state.rate_limit.lock().await;
        if !rl.permitir(&chave) {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                axum::Json(serde_json::json!({
                    "erro": "muitas tentativas de autorização; aguarde alguns minutos"
                })),
            ));
        }
    }

    let resultado = {
        let db = state.db.lock().await;
        let usuarios = mikella_infra_sqlite::UsuarioRepo::novo(db.conn());
        usuarios.buscar_por_username(username)
    };

    let usuario = match resultado {
        Ok(Some(u)) if u.ativo => u,
        Ok(_) => {
            // Equaliza o tempo com o caso "usuário existe" (SEG-005).
            let _ = verificar_senha(&cred.senha, &hash_senha_ficticio());
            state.rate_limit.lock().await.registrar_falha(&chave);
            return Err(nao_autorizado());
        }
        Err(e) => {
            tracing::error!(erro = %e, "falha ao buscar aprovador");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({ "erro": "erro interno" })),
            ));
        }
    };

    if !verificar_senha(&cred.senha, &usuario.senha_hash) {
        state.rate_limit.lock().await.registrar_falha(&chave);
        return Err(nao_autorizado());
    }

    if usuario.papel.nivel() < 2 {
        state.rate_limit.lock().await.registrar_falha(&chave);
        return Err(proibido("o aprovador precisa ser gerente ou dono da loja"));
    }

    if usuario.id == ator.id() {
        return Err(proibido(
            "o aprovador deve ser um usuário diferente do operador",
        ));
    }

    state.rate_limit.lock().await.limpar(&chave);
    Ok(Aprovacao {
        id: usuario.id,
        nome: usuario.nome,
        papel: usuario.papel.as_db().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use mikella_domain_people::{Papel, Usuario};
    use mikella_shared_kernel::ids::UsuarioId;

    fn usuario(username: &str, papel: Papel) -> Usuario {
        Usuario {
            id: UsuarioId::new(),
            nome: username.to_string(),
            username: username.to_string(),
            email: None,
            senha_hash: "hash".to_string(),
            papel,
            pin_hash: None,
            capacidades_extras: Vec::new(),
            ativo: true,
            criado_em: Utc::now(),
            atualizado_em: Utc::now(),
        }
    }

    fn estado() -> AppState {
        let db = mikella_infra_sqlite::Database::em_memoria().unwrap();
        let email: std::sync::Arc<dyn crate::state::EmailBackend> =
            std::sync::Arc::new(crate::state::ConsoleEmailBackend::new());
        AppState::new(db, email)
    }

    #[tokio::test]
    async fn gerente_aprova_a_propria_operacao() {
        let state = estado();
        let ator = Ator(usuario("gerente", Papel::Gerente));

        let aprovacao = exigir_aprovador(&state, &ator, None, "estorno")
            .await
            .expect("gerente deve aprovar sem pedir credenciais");

        assert_eq!(aprovacao.id, ator.id());
        assert_eq!(aprovacao.papel, "GERENTE");
    }

    #[tokio::test]
    async fn operador_sem_credenciais_e_bloqueado() {
        let state = estado();
        let ator = Ator(usuario("operador", Papel::OperadorCaixa));

        let erro = exigir_aprovador(&state, &ator, None, "estorno")
            .await
            .expect_err("operador não pode se autoaprovar");
        assert_eq!(erro.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn operador_nao_aprova_a_si_mesmo() {
        let state = estado();
        let ator = Ator(usuario("operador", Papel::OperadorCaixa));

        // Mesmo que o username exista no banco, operador não é gerente.
        let erro = exigir_aprovador(
            &state,
            &ator,
            Some(CredenciaisAprovador {
                username: "operador".into(),
                senha: "senha".into(),
            }),
            "estorno",
        )
        .await
        .expect_err("operador não pode autorizar a si mesmo");
        assert_eq!(erro.0, StatusCode::UNAUTHORIZED);
    }
}
