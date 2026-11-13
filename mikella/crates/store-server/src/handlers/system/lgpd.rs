//! Expurgo LGPD: anonimização de clientes sem movimentação dentro da retenção.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use mikella_infra_sqlite::{expurgar_clientes_por_retencao, DiagnosticoRepo};

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExpurgoLgpdRequest {
    /// Retenção em anos. O mínimo aceito é 5 (guarda fiscal).
    pub anos: Option<u32>,
    /// `false` (padrão) apenas simula; `true` anonimiza de verdade.
    pub aplicar: bool,
}

/// `POST /v1/sistema/lgpd/expurgo` — simula ou aplica a anonimização de
/// clientes sem movimentação dentro do prazo de retenção (LGPD Art. 16).
///
/// Nunca apaga registros fiscais (`vendas`, `livro_caixa`, `auditoria`): apenas
/// os dados pessoais em `pessoas` são substituídos, preservando o `id` para as
/// chaves estrangeiras. A aplicação e a auditoria entram na mesma transação.
pub async fn expurgo_lgpd_handler(
    State(state): State<AppState>,
    ator: Ator,
    payload: Option<Json<ExpurgoLgpdRequest>>,
) -> impl IntoResponse {
    exigir!(ator, "sistema.configurar");

    let pedido = payload.map(|Json(p)| p).unwrap_or_default();
    let anos = pedido.anos.unwrap_or(5);
    if anos < 5 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "erro": "a retenção mínima é de 5 anos; prazos menores exigem revisão jurídica"
            })),
        );
    }

    let mut db = state.db.lock().await;
    let fuso = DiagnosticoRepo::novo(db.conn()).fuso_horario();

    let resultado = if pedido.aplicar {
        let tx = match db.transacao() {
            Ok(tx) => tx,
            Err(e) => {
                // SEC-017: detalhe do SQLite só no log.
                tracing::error!(erro = %e, "falha ao abrir transação do expurgo LGPD");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha ao iniciar o expurgo" })),
                );
            }
        };

        let relatorio = match expurgar_clientes_por_retencao(&tx, &fuso, anos, true) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(erro = %e, "falha ao aplicar expurgo LGPD");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha ao aplicar o expurgo" })),
                );
            }
        };

        let detalhe = format!(
            "anonimizados={}; prazo_anos={anos}; corte={}",
            relatorio.clientes_anonimizados, relatorio.data_corte
        );
        let auditoria = mikella_infra_sqlite::AuditoriaRepo::novo(&tx);
        if let Err(e) = auditoria.registrar(
            "lgpd",
            "pessoas",
            "EXPURGO_LGPD",
            Some(&ator.id().to_string()),
            None,
            None,
            Some(&detalhe),
        ) {
            tracing::error!(erro = %e, "falha ao auditar expurgo LGPD");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha ao registrar auditoria do expurgo" })),
            );
        }

        if let Err(e) = tx.commit() {
            tracing::error!(erro = %e, "falha ao confirmar expurgo LGPD");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha ao confirmar o expurgo" })),
            );
        }

        relatorio
    } else {
        match expurgar_clientes_por_retencao(db.conn(), &fuso, anos, false) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(erro = %e, "falha ao simular expurgo LGPD");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha ao simular o expurgo" })),
                );
            }
        }
    };

    (StatusCode::OK, Json(serde_json::json!(resultado)))
}
