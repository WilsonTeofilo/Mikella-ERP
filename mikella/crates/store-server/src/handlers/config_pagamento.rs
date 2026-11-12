//! Limites de pagamento configuráveis por loja.
//!
//! O máximo de parcelas no cartão e o prazo do crediário deixaram de ser
//! constantes do código: vivem em `loja_config` e são lidos por este handler.
//! A venda sempre revalida no servidor (`sales_write`), então uma UI adulterada
//! não consegue furar o limite.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use mikella_infra_sqlite::{
    AuditoriaRepo, ConfigLojaRepo, ConfigPagamento, MAX_PARCELAS_CARTAO_LIMITE,
    PRAZO_CREDIARIO_DIAS_LIMITE,
};

#[derive(Debug, Serialize)]
pub struct ConfigPagamentoResponse {
    pub max_parcelas_cartao: u32,
    pub prazo_crediario_dias: u32,
    pub limites: LimitesConfigPagamento,
}

#[derive(Debug, Serialize)]
pub struct LimitesConfigPagamento {
    pub max_parcelas_cartao: u32,
    pub prazo_crediario_dias: u32,
}

impl From<ConfigPagamento> for ConfigPagamentoResponse {
    fn from(config: ConfigPagamento) -> Self {
        Self {
            max_parcelas_cartao: config.max_parcelas_cartao,
            prazo_crediario_dias: config.prazo_crediario_dias,
            limites: LimitesConfigPagamento {
                max_parcelas_cartao: MAX_PARCELAS_CARTAO_LIMITE,
                prazo_crediario_dias: PRAZO_CREDIARIO_DIAS_LIMITE,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtualizarConfigPagamentoRequest {
    pub max_parcelas_cartao: u32,
    pub prazo_crediario_dias: u32,
}

/// `GET /v1/config/pagamento` — limites ativos para o PDV renderizar.
///
/// Qualquer operador autenticado pode ler: a tela precisa do valor para não
/// oferecer uma opção que o servidor vai recusar.
pub async fn obter_config_pagamento(
    State(state): State<AppState>,
    _ator: Ator,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let config = ConfigLojaRepo::novo(db.conn()).config_pagamento();
    (StatusCode::OK, Json(ConfigPagamentoResponse::from(config)))
}

/// `PUT /v1/sistema/config-pagamento` — altera os limites da loja.
pub async fn atualizar_config_pagamento(
    State(state): State<AppState>,
    ator: Ator,
    Json(pedido): Json<AtualizarConfigPagamentoRequest>,
) -> impl IntoResponse {
    exigir!(ator, "sistema.configurar");

    let config =
        match ConfigPagamento::validar(pedido.max_parcelas_cartao, pedido.prazo_crediario_dias) {
            Ok(config) => config,
            Err(mensagem) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "erro": mensagem })),
                );
            }
        };

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao abrir transação da configuração de pagamento");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha ao salvar a configuração" })),
            );
        }
    };

    if let Err(e) = ConfigLojaRepo::novo(&tx).definir_config_pagamento(config) {
        tracing::error!(erro = %e, "falha ao gravar configuração de pagamento");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": "falha ao salvar a configuração" })),
        );
    }

    let detalhe = format!(
        "max_parcelas_cartao={}; prazo_crediario_dias={}",
        config.max_parcelas_cartao, config.prazo_crediario_dias
    );
    if let Err(e) = AuditoriaRepo::novo(&tx).registrar(
        "loja_config",
        "pagamento",
        "CONFIG_PAGAMENTO",
        Some(&ator.id().to_string()),
        None,
        None,
        Some(&detalhe),
    ) {
        tracing::error!(erro = %e, "falha ao auditar configuração de pagamento");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": "falha ao registrar a alteração" })),
        );
    }

    if let Err(e) = tx.commit() {
        tracing::error!(erro = %e, "falha ao confirmar configuração de pagamento");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": "falha ao salvar a configuração" })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!(ConfigPagamentoResponse::from(config))),
    )
}
