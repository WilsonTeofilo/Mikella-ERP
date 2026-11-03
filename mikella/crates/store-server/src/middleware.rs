//! Middleware de autorização e identidade do ator autenticado.
//!
//! O token Bearer `mikella_<uuid>` é validado em memória (SEG-012).
//! Depois da validação, o usuário é recarregado do banco em cada request:
//! desativar um usuário passa a valer imediatamente, sem esperar o token
//! expirar. O ator (com papel e capacidades) é inserido nas extensions e
//! exposto pelo extractor [`Ator`].
//!
//! SEG-020: negar por padrão. Handlers que exigem capacidade chamam
//! [`Ator::exigir`]; não existe caminho que trate ausência de middleware
//! como usuário anônimo privilegiado.

use std::time::Instant;

use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use mikella_domain_people::ports::UsuarioRepository;
use mikella_domain_people::Usuario;
use mikella_infra_sqlite::UsuarioRepo;

use crate::state::{AppState, SESSAO_IDLE, SESSAO_TTL};
use crate::util::senha::TOKEN_PREFIXO;

/// Erro HTTP padronizado (status + corpo JSON genérico).
pub type ErroHttp = (StatusCode, Json<serde_json::Value>);

/// Resposta 403 com mensagem estável e sem detalhe interno.
pub fn proibido(mensagem: &str) -> ErroHttp {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({ "erro": mensagem })),
    )
}

/// Ator autenticado (usuário completo, com papel e capacidades).
#[derive(Clone)]
pub struct Ator(pub Usuario);

impl Ator {
    pub fn id(&self) -> mikella_shared_kernel::ids::UsuarioId {
        self.0.id
    }

    pub fn papel(&self) -> &mikella_domain_people::Papel {
        &self.0.papel
    }

    /// Exige uma capacidade do catálogo (SEG-020).
    /// O papel DONO (Host / Administrador Mestre) tem acesso total irrestrito a tudo (RF-001).
    pub fn exigir(&self, capacidade: &str) -> Result<(), ErroHttp> {
        if matches!(self.0.papel, mikella_domain_people::Papel::Dono) || self.0.tem_capacidade(capacidade) {
            Ok(())
        } else {
            tracing::warn!(
                usuario = %self.0.username,
                capacidade,
                "capacidade negada"
            );
            Err(proibido("permissão negada"))
        }
    }
}

/// Atalho para handlers: `exigir!(ator, "produto.criar");`.
#[macro_export]
macro_rules! exigir {
    ($ator:expr, $capacidade:expr) => {
        if let Err(erro) = $ator.exigir($capacidade) {
            return erro;
        }
    };
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for Ator
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Ator>().cloned().ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "erro": "sessão inválida" })),
            )
                .into_response()
        })
    }
}

/// Middleware: valida token, expiração e usuário ativo; injeta [`Ator`].
pub async fn exigir_token(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let token = match extract_token(req.headers()) {
        Some(t) => t,
        None => return nao_autorizado("token ausente ou inválido"),
    };

    let agora = Instant::now();
    let username = {
        let mut sessoes = state.sessoes.lock().await;
        let sessao = sessoes.get_mut(&token);
        match sessao {
            Some(s) if agora.duration_since(s.criada_em) <= SESSAO_TTL
                && agora.duration_since(s.ultimo_uso) <= SESSAO_IDLE =>
            {
                s.ultimo_uso = agora;
                s.username.clone()
            }
            Some(_) => {
                sessoes.remove(&token);
                return nao_autorizado("sessão expirada");
            }
            None => return nao_autorizado("sessão expirada ou inválida"),
        }
    };

    // Recarrega o usuário do banco. Se foi desativado, a sessão cai agora.
    // O guard do banco é solto antes de qualquer `.await` seguinte: rusqlite
    // não é `Sync` e manter `&Connection` através de um await quebraria o
    // requisito `Send` do handler do Axum.
    let resultado = {
        let db = state.db.lock().await;
        let repo = UsuarioRepo::novo(db.conn());
        repo.buscar_por_username(&username)
    };

    let usuario = match resultado {
        Ok(Some(u)) if u.ativo => u,
        Ok(_) => {
            tracing::warn!(usuario = %username, "sessão de usuário inativo removida");
            state.sessoes.lock().await.remove(&token);
            return nao_autorizado("sessão inválida");
        }
        Err(erro) => {
            tracing::error!(erro = %erro, "falha ao carregar ator autenticado");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "erro interno" })),
            )
                .into_response();
        }
    };

    req.extensions_mut().insert(Ator(usuario));
    next.run(req).await
}

fn nao_autorizado(mensagem: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "erro": mensagem })),
    )
        .into_response()
}

/// Extrai o token Bearer válido (prefixo `mikella_`).
fn extract_token(headers: &HeaderMap<HeaderValue>) -> Option<String> {
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    auth.strip_prefix("Bearer ")
        .filter(|t| t.starts_with(TOKEN_PREFIXO))
        .map(|t| t.to_string())
}
