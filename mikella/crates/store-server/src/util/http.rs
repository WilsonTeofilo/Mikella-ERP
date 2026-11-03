//! Utilitários HTTP: mapeamento de erros e respostas de erro comuns.
//!
//! Mantém o `main.rs` livre de detalhes de camada de transporte.

use axum::http::StatusCode;
use axum::Json;
use mikella_shared_kernel::error::DomainError;

/// Mapeia erros de domínio para respostas HTTP apropriadas.
///
/// SEG-081: erros expostos à UI e à rede são **estáveis e genéricos**,
/// com ID de correlação. O detalhe técnico fica apenas no log local.
/// A mensagem genérica é derivada do tipo de erro, nunca do conteúdo interno.
pub fn error_para_http(erro: DomainError) -> (StatusCode, Json<serde_json::Value>) {
    let (status, msg_generico) = match &erro {
        DomainError::NaoEncontrado { .. } => (StatusCode::NOT_FOUND, "recurso não encontrado"),
        DomainError::Invalido(_) => (StatusCode::BAD_REQUEST, "argumento inválido"),
        DomainError::PermissaoNegada(_) => (StatusCode::FORBIDDEN, "permissão negada"),
        DomainError::EstadoInvalido(_) => (StatusCode::CONFLICT, "estado inválido"),
        DomainError::CotaEsgotada(_) => (StatusCode::TOO_MANY_REQUESTS, "cota esgotada"),
        DomainError::IdempotenciaConflito(_) => (StatusCode::CONFLICT, "operação já processada"),
        DomainError::InvarianteViolada(_) | DomainError::Infraestrutura(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "erro interno")
        }
    };
    // SEG-081: a UI recebe uma referência curta para o suporte; o detalhe
    // técnico completo fica somente no log local.
    let incidencia = uuid::Uuid::new_v4().simple().to_string();
    tracing::warn!(
        erro = %erro,
        status = ?status,
        incidencia = %incidencia,
        "erro de domínio mapeado para HTTP"
    );
    (
        status,
        Json(serde_json::json!({ "erro": msg_generico, "incidencia": incidencia })),
    )
}

/// Helper: retorna 400 com mensagem amigável.
/// Usado para validações de entrada do cliente (não expõe informações internas).
pub fn bad_request(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"erro": msg})),
    )
}
