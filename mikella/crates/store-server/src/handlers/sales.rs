//! Handlers de produtos e vendas (leitura).
//!
//! Estes handlers fazem leitura direta via SQL (CQRS read model).
//! O domínio mantém a regra de negócio; aqui apenas serializamos o resultado.
//!
//! Nenhum erro de banco é silenciado: `unwrap`/`filter_map(Result::ok)` foram
//! removidos para que uma linha corrompida vire 500 e fique no log, em vez de
//! desaparecer da resposta como se a lista estivesse vazia.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::dto::VendaSummary;
use crate::dto::{
    LivroCaixaQuery, PlanoParcelaDto, SimularParcelasQuery, SimularParcelasResponse, VendasQuery,
};
use crate::exigir;
use crate::handlers::products::{produto_para_json, view_para_response};
use crate::middleware::Ator;
use crate::state::AppState;
use mikella_infra_sqlite::{
    ConfigLojaRepo, DiagnosticoRepo, FiltroVendas, LivroRepo, ProdutoRepo, VendaRepo,
};
use mikella_shared_kernel::ids::SessaoId;
use mikella_shared_kernel::money::Money;

/// Converte um horário local da loja para UTC.
///
/// Quando o horário não existe no fuso (salto de horário de verão), interpreta
/// o mesmo valor como UTC. O fallback anterior (`Utc::now`) encolhia a janela
/// para o instante atual e escondia as vendas do dia.
fn resolver_local(tz: &Tz, naive: NaiveDateTime) -> DateTime<Utc> {
    tz.from_local_datetime(&naive)
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| {
            tracing::warn!(
                horario = %naive,
                "horário local inexistente no fuso da loja; usando o mesmo valor em UTC"
            );
            Utc.from_utc_datetime(&naive)
        })
}

/// 23:59:59 — validado em tempo de compilação, sem `unwrap` em runtime.
const FIM_DO_DIA: NaiveTime = match NaiveTime::from_hms_opt(23, 59, 59) {
    Some(hora) => hora,
    None => NaiveTime::MIN,
};

/// Calcula o início e o fim do "hoje" no fuso IANA da loja como timestamps UTC RFC-3339.
///
/// Exemplo: se a loja está em `America/Sao_Paulo` (UTC-3) e agora são 23h30 UTC
/// (20h30 local), retorna a janela 03:00:00Z–26:59:59Z (ou seja, 00:00:00 BRT–23:59:59 BRT).
/// Isso resolve o bug em que `date('now')` em UTC retornava o dia seguinte entre 21h e 0h BRT.
fn hoje_utc_intervalo(fuso_iana: &str) -> (String, String) {
    let tz: Tz = fuso_iana.parse().unwrap_or(chrono_tz::America::Sao_Paulo);
    let agora_local = Utc::now().with_timezone(&tz);
    let data_local = agora_local.date_naive();

    let inicio_local = resolver_local(&tz, data_local.and_time(NaiveTime::MIN));
    let fim_local = resolver_local(&tz, data_local.and_time(FIM_DO_DIA));

    (
        inicio_local.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        fim_local.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
    )
}

/// `GET /v1/produtos` — lista produtos ativos (não arquivados).
pub async fn listar_produtos(State(state): State<AppState>, ator: Ator) -> impl IntoResponse {
    exigir!(ator, "produto.ver");
    tracing::debug!(usuario = %ator.0.username, "listando produtos");

    let db = state.db.lock().await;

    let produtos = ProdutoRepo::novo(db.conn()).listar_ativos_view();

    match produtos {
        Ok(views) => {
            let mut lista = Vec::with_capacity(views.len());
            for view in views {
                match view_para_response(view) {
                    Ok(produto) => lista.push(produto_para_json(&produto)),
                    Err(e) => {
                        tracing::error!(erro = %e, "produto com valores fora do intervalo");
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({ "erro": "falha de banco" })),
                        );
                    }
                }
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({ "produtos": lista })),
            )
        }
        Err(e) => {
            tracing::error!(erro = %e, "erro ao listar produtos");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha de banco" })),
            )
        }
    }
}

/// `GET /v1/vendas?limite=50` — lista vendas do dia.
///
/// Read model via SQL (CQRS) — ordenado por `created_at DESC`.
pub async fn vendas_listar(
    State(state): State<AppState>,
    ator: Ator,
    Query(params): Query<VendasQuery>,
) -> impl IntoResponse {
    exigir!(ator, "venda.ver");

    let db = state.db.lock().await;
    let limite = params.limite.clamp(1, 200);

    // Calcula o "hoje" correto no fuso da loja — resolve o bug 21h-0h BRT / UTC.
    let (data_inicio, data_fim) =
        if params.data_inicio.is_none() && params.data_fim.is_none() && params.codigo.is_none() {
            let fuso = DiagnosticoRepo::novo(db.conn()).fuso_horario();
            let (ini, fim) = hoje_utc_intervalo(&fuso);
            tracing::debug!(fuso = %fuso, inicio = %ini, fim = %fim, "calculando janela 'hoje'");
            (Some(ini), Some(fim))
        } else {
            (params.data_inicio, params.data_fim)
        };

    let filtro = FiltroVendas {
        codigo: params.codigo,
        data_inicio,
        data_fim,
        sem_filtro_data: false,
        limite: i64::from(limite),
    };

    match VendaRepo::novo(db.conn()).listar_resumo(&filtro) {
        Ok(vendas) => {
            let lista: Vec<VendaSummary> = vendas
                .into_iter()
                .map(|v| VendaSummary {
                    id: v.id,
                    coo: v.coo,
                    sessao_id: v.sessao_id,
                    operador_id: v.operador_id,
                    operador: v.operador,
                    estado: v.estado,
                    total: Money::from_centavos(v.total_centavos).to_string(),
                    total_centavos: v.total_centavos,
                    recebido: Money::from_centavos(v.recebido_centavos).to_string(),
                    created_at: v.created_at,
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!(lista)))
        }
        Err(e) => {
            tracing::error!(erro = %e, "erro ao listar vendas");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha de banco" })),
            )
        }
    }
}

/// `GET /v1/livro-caixa?sessao_id=<uuid>` — lista entradas do livro de uma sessão.
///
/// Read model via SQL direto (CQRS): o `LivroCaixa` do domínio tem `entradas`
/// privado e não serializa; aqui fazemos um SELECT otimizado para a UI.
pub async fn livro_caixa(
    State(state): State<AppState>,
    ator: Ator,
    Query(params): Query<LivroCaixaQuery>,
) -> impl IntoResponse {
    exigir!(ator, "caixa.ver");
    tracing::debug!(usuario = %ator.0.username, sessao_id = %params.sessao_id, "consultando livro-caixa");

    let sessao_id = match params.sessao_id.parse::<SessaoId>() {
        Ok(id) => id.to_string(),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "erro": "sessao_id inválido (UUID esperado)" })),
            )
        }
    };

    let db = state.db.lock().await;
    let livro = LivroRepo::novo(db.conn());

    let entradas: Vec<crate::dto::response::LivroEntradaResponse> =
        match livro.listar_entradas(&sessao_id) {
            Ok(entradas) => entradas
                .into_iter()
                .map(|e| crate::dto::response::LivroEntradaResponse {
                    sequencia: e.sequencia,
                    tipo: e.tipo,
                    valor: Money::from_centavos(e.valor_centavos).to_string(),
                    troco: Money::from_centavos(e.troco_centavos).to_string(),
                    usuario_id: e.usuario_id,
                    aprovador_id: e.aprovador_id,
                    motivo: e.motivo,
                    referencia_id: e.referencia_id,
                    referencia_tipo: e.referencia_tipo,
                    prev_hash: e.prev_hash,
                    hash: e.hash,
                    created_at: e.created_at,
                })
                .collect(),
            Err(e) => {
                tracing::error!(erro = %e, sessao_id = %sessao_id, "erro ao ler livro-caixa");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha de banco" })),
                );
            }
        };

    let saldo_total = match livro.saldo_total(&sessao_id) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(erro = %e, sessao_id = %sessao_id, "erro ao somar livro-caixa");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha de banco" })),
            );
        }
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "sessao_id": sessao_id,
            "entradas": entradas,
            "saldo_esperado": Money::from_centavos(saldo_total).to_string(),
            "saldo_esperado_centavos": saldo_total,
        })),
    )
}

/// `GET /v1/vendas/simular-parcelas` — calcula planos de parcelamento de 1 a N vezes no servidor.
/// Regra: o servidor decide e a tela só renderiza o que veio (resto na última parcela).
pub async fn simular_parcelas(
    State(state): State<AppState>,
    Query(q): Query<SimularParcelasQuery>,
) -> impl IntoResponse {
    // Rust é a autoridade: o teto vem da configuração da loja, não do clique.
    let max_loja = {
        let db = state.db.lock().await;
        ConfigLojaRepo::novo(db.conn())
            .config_pagamento()
            .max_parcelas_cartao
    };
    let max = q.max_parcelas.clamp(1, max_loja);
    let total = Money::from_centavos(q.total_centavos);

    let mut planos = Vec::with_capacity(max as usize);
    for n in 1..=max {
        match total.dividir_em_parcelas(n) {
            Ok(parcelas) => {
                let base = parcelas.first().map(|p| p.centavos()).unwrap_or(0);
                let ultima = parcelas.last().map(|p| p.centavos()).unwrap_or(0);
                let texto = if n == 1 {
                    format!("1x de {} (à vista)", Money::from_centavos(base))
                } else if base == ultima {
                    format!("{}x de {} sem juros", n, Money::from_centavos(base))
                } else {
                    format!(
                        "{}x ({}x de {} + 1x de {}) • Total: {}",
                        n,
                        n - 1,
                        Money::from_centavos(base),
                        Money::from_centavos(ultima),
                        total
                    )
                };
                planos.push(PlanoParcelaDto {
                    parcelas: n,
                    valor_parcela_centavos: base,
                    ultima_parcela_centavos: ultima,
                    total_centavos: q.total_centavos,
                    texto,
                });
            }
            Err(_) => break,
        }
    }

    (
        StatusCode::OK,
        Json(SimularParcelasResponse {
            total_centavos: q.total_centavos,
            planos,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_model_do_historico_prepara_no_schema_real() {
        // Impede regressão do read model que alimenta F3/estorno: o COO vem do
        // rowid e o nome do operador do JOIN com usuarios.
        let db = mikella_infra_sqlite::Database::em_memoria().unwrap();
        let filtro = FiltroVendas {
            limite: 100,
            ..FiltroVendas::default()
        };
        let vendas = VendaRepo::novo(db.conn())
            .listar_resumo(&filtro)
            .expect("SQL do histórico de vendas inválido");
        assert!(vendas.is_empty());
    }
}
