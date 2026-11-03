//! Handlers de doação (prompt e acknowledge).
//!
//! Lógica (docs/fases.md §MVP-0):
//! - Se doação foi feita este mês → suppress
//! - Se hoje é um "dia de prompt" (cada 3 dias desde a primeira ativação) → show
//! - Track via app_preferences chave `donation_mes` (YYYY-MM)
//!
//! O ack é apenas um lembrete local; não concede licença nem altera plano.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::middleware::Ator;
use crate::state::AppState;

/// `GET /v1/donation/prompt` — indica se o prompt de doação deve aparecer.
pub async fn donation_prompt(State(state): State<AppState>, ator: Ator) -> impl IntoResponse {
    tracing::debug!(usuario = %ator.0.username, "consultando donation prompt");

    let db = state.db.lock().await;

    // 1. Verifica se doação foi feita este mês
    let doado_mes: i64 = match db.conn().query_row(
        "SELECT COUNT(*) FROM app_preferences WHERE chave = 'donation_mes' AND valor = strftime('%Y-%m', 'now')",
        [],
        |r| r.get(0),
    ) {
        Ok(n) => n,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao consultar doação do mês");
            return erro_interno();
        }
    };

    if doado_mes > 0 {
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "show": false, "message": null })),
        );
    }

    // 2. Calcula dias desde a primeira ativação
    let dias_desde_ativacao: i64 = match db.conn().query_row(
        "SELECT CAST(julianday('now') - julianday(created_at) AS INTEGER) FROM instance_identity WHERE id = 1",
        [],
        |r| r.get::<_, i64>(0),
    ) {
        Ok(d) => d.max(0),
        Err(e) => {
            tracing::error!(erro = %e, "falha ao calcular dias desde a ativação");
            return erro_interno();
        }
    };

    // Prompt a cada 3 dias (dia 0, 3, 6, 9...)
    if dias_desde_ativacao % 3 == 0 {
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "show": true,
                "message": "Apoie o Mikella com uma doação mensal. Isso também suprime futuros avisos.",
            })),
        )
    } else {
        (
            StatusCode::OK,
            Json(serde_json::json!({ "show": false, "message": null })),
        )
    }
}

/// `POST /v1/donation/ack` — grava que uma doação foi feita neste mês.
/// Suprime futuros prompts até o próximo mês.
pub async fn donation_ack(State(state): State<AppState>, ator: Ator) -> impl IntoResponse {
    tracing::debug!(usuario = %ator.0.username, "registro de doação recebido");

    let db = state.db.lock().await;
    let mes_atual = chrono::Utc::now().format("%Y-%m").to_string();

    if let Err(e) = db.conn().execute(
        "INSERT OR REPLACE INTO app_preferences (chave, valor) VALUES ('donation_mes', ?1)",
        [&mes_atual],
    ) {
        tracing::error!(erro = %e, "falha ao registrar doação");
        return erro_interno();
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "ok": true, "suprimido": true })),
    )
}

fn erro_interno() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "erro": "erro interno" })),
    )
}
