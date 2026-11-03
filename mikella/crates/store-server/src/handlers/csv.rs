//! Handler de importação CSV de produtos (PROD-010).
//!
//! Rota: `POST /v1/import/csv`
//!
//! Recebe o conteúdo CSV como texto UTF-8 no body. Cada linha é processada
//! independentemente; falhas são reportadas por linha sem abortar o lote.
//!
//! Limites aplicados antes de parsear: 2 MiB de body (protege o PC fraco) e
//! 5.000 linhas no caso de uso. Erro interno nunca vaza detalhe técnico — a
//! resposta segue SEG-081 com um ID de incidência.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use mikella_app_inventory::import_csv::importar_produtos_csv;
use mikella_app_inventory::ProdutoRepos;
use mikella_infra_sqlite::ProdutoRepo;

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;

/// Tamanho máximo do body CSV (2 MiB).
const CSV_MAX_BYTES: usize = 2 * 1024 * 1024;

/// `POST /v1/import/csv`
///
/// Body: texto CSV puro (`text/plain` ou `text/csv`).
/// O header deve conter `nome` e `preco`; demais colunas são opcionais.
pub async fn import_csv_handler(
    State(state): State<AppState>,
    ator: Ator,
    body: Bytes,
) -> impl IntoResponse {
    exigir!(ator, "produto.criar");
    tracing::debug!(operador = %ator.0.username, bytes = body.len(), "importando produtos CSV");

    if body.len() > CSV_MAX_BYTES {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({ "erro": "arquivo CSV excede o limite de 2 MiB" })),
        );
    }

    let csv_text = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "erro": "body deve ser texto UTF-8 válido" })),
            )
        }
    };

    let db = state.db.lock().await;
    let produtos_repo = ProdutoRepo::novo(db.conn());
    let repos = ProdutoRepos {
        produtos: &produtos_repo,
    };

    match importar_produtos_csv(&repos, csv_text) {
        Ok(resultado) => {
            tracing::info!(
                total = resultado.total,
                sucessos = resultado.sucessos,
                erros = resultado.erros,
                "importação CSV concluída"
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "total": resultado.total,
                    "sucessos": resultado.sucessos,
                    "erros": resultado.erros,
                    "detalhes": resultado.detalhes,
                })),
            )
        }
        Err(e) => {
            tracing::warn!(erro = %e, "falha na importação CSV");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "erro": "CSV inválido" })),
            )
        }
    }
}
