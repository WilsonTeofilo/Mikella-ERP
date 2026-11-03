//! Handler de health check.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::dto::HealthResponse;
use crate::state::AppState;

/// `GET /v1/health` — verifica integridade do banco e status do servidor.
pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let db = state.db.lock().await;

    let schema = match db.versao_schema() {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao ler versão do schema");
            "indisponivel".to_string()
        }
    };
    match db.verificar_integridade() {
        Ok(()) => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok",
                versao: env!("CARGO_PKG_VERSION"),
                schema,
                integridade: "ok",
                hora: chrono::Utc::now().to_rfc3339(),
            }),
        ),
        Err(erro) => {
            tracing::error!(%erro, "falha de integridade do banco");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "degradado",
                    versao: env!("CARGO_PKG_VERSION"),
                    schema,
                    integridade: "falhou",
                    hora: chrono::Utc::now().to_rfc3339(),
                }),
            )
        }
    }
}
