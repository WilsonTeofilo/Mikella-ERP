//! Handler de analytics de vendas para os gráficos do dashboard (DSH-001).
//!
//! - `GET /v1/analytics/vendas?periodo=hoje|semana|mes`
//! - `GET /v1/analytics/vendas?inicio=AAAA-MM-DD&fim=AAAA-MM-DD`
//!
//! A janela é resolvida no fuso da loja (mesma regra do resumo) e a agregação
//! fica toda no SQLite; o frontend só desenha. Nenhum preço é recalculado aqui.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::NaiveDate;
use serde::Deserialize;

use mikella_infra_sqlite::{AnalyticsVendasRepo, DiagnosticoRepo};

use crate::exigir;
use crate::handlers::finance::resolver_janela_resumo;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::util::bad_request;

/// Janela máxima aceita para não varrer o banco inteiro num PC fraco.
const DIAS_MAXIMOS: i64 = 366;

#[derive(Deserialize)]
pub struct AnalyticsVendasQuery {
    /// `hoje` (padrão), `semana` (7 dias) ou `mes` (mês corrente).
    pub periodo: Option<String>,
    /// Data local inicial (`AAAA-MM-DD`); sobrepõe `periodo`.
    pub inicio: Option<String>,
    /// Data local final (`AAAA-MM-DD`); sobrepõe `periodo`.
    pub fim: Option<String>,
}

/// `GET /v1/analytics/vendas` - séries e rankings para os gráficos do dono.
pub async fn obter_analytics_vendas_handler(
    State(state): State<AppState>,
    ator: Ator,
    Query(query): Query<AnalyticsVendasQuery>,
) -> impl IntoResponse {
    exigir!(ator, "financeiro.ver");

    let periodo = query.periodo.as_deref().unwrap_or("hoje");
    tracing::debug!(operador = %ator.0.username, periodo, "obtendo analytics de vendas");

    let db = state.db.lock().await;
    let fuso = DiagnosticoRepo::novo(db.conn()).fuso_horario();

    let (inicio, fim) = match resolver_janela_resumo(
        &fuso,
        periodo,
        query.inicio.as_deref(),
        query.fim.as_deref(),
    ) {
        Ok(janela) => janela,
        Err(resposta) => return resposta,
    };

    // Defesa contra janelas absurdas: o dashboard é uma tela, não um BI.
    let intervalo = NaiveDate::parse_from_str(&inicio, "%Y-%m-%d")
        .ok()
        .zip(NaiveDate::parse_from_str(&fim, "%Y-%m-%d").ok());
    match intervalo {
        Some((ini, f)) if (f - ini).num_days() <= DIAS_MAXIMOS => {}
        _ => return bad_request("janela de datas inválida"),
    }

    let repo = AnalyticsVendasRepo::novo(db.conn());
    match repo.obter(&fuso, &inicio, &fim) {
        Ok(dados) => (
            StatusCode::OK,
            Json(serde_json::to_value(dados).unwrap_or_default()),
        ),
        Err(e) => {
            tracing::error!(erro = %e, "erro ao obter analytics de vendas");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha de banco" })),
            )
        }
    }
}
