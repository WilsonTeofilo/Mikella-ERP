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

use crate::dto::VendaSummary;
use crate::dto::{LivroCaixaQuery, VendasQuery};
use crate::exigir;
use crate::handlers::products::{produto_para_json, row_para_produto_response, SQL_PRODUTO_POR_ID};
use crate::middleware::Ator;
use crate::state::AppState;
use mikella_shared_kernel::ids::SessaoId;
use mikella_shared_kernel::money::Money;

/// `GET /v1/produtos` — lista produtos ativos (não arquivados).
pub async fn listar_produtos(State(state): State<AppState>, ator: Ator) -> impl IntoResponse {
    exigir!(ator, "produto.ver");
    tracing::debug!(usuario = %ator.0.username, "listando produtos");

    let db = state.db.lock().await;

    let sql_listar = format!(
        "{} WHERE p.arquivado = 0 ORDER BY p.nome",
        SQL_PRODUTO_POR_ID
    );

    let produtos = (|| -> rusqlite::Result<Vec<_>> {
        let mut stmt = db.conn().prepare(&sql_listar)?;
        let rows = stmt.query_map([], row_para_produto_response)?;
        rows.collect()
    })();

    match produtos {
        Ok(produtos) => {
            let lista: Vec<_> = produtos.iter().map(produto_para_json).collect();
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
    tracing::debug!(usuario = %ator.0.username, limite = params.limite, "listando vendas do dia");

    let db = state.db.lock().await;
    let limite = params.limite.clamp(1, 200);

    let vendas = (|| -> rusqlite::Result<Vec<VendaSummary>> {
        let mut stmt = db.conn().prepare(
            "SELECT id, sessao_id, operador_id, estado,
                    total_centavos, recebido_centavos, created_at
             FROM vendas
             WHERE date(created_at) = date('now')
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([&limite], |row| {
            let total_centavos: i64 = row.get(4)?;
            let recebido_centavos: i64 = row.get(5)?;
            Ok(VendaSummary {
                id: row.get(0)?,
                sessao_id: row.get(1)?,
                operador_id: row.get(2)?,
                estado: row.get(3)?,
                total: Money::from_centavos(total_centavos).to_string(),
                total_centavos,
                recebido: Money::from_centavos(recebido_centavos).to_string(),
                created_at: row.get(6)?,
            })
        })?;
        rows.collect()
    })();

    match vendas {
        Ok(vendas) => (StatusCode::OK, Json(serde_json::json!(vendas))),
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

    let entradas = (|| -> rusqlite::Result<Vec<crate::dto::response::LivroEntradaResponse>> {
        let mut stmt = db.conn().prepare(
            "SELECT sequencia, tipo, valor_centavos, troco_centavos,
                    usuario_id, aprovador_id, motivo,
                    referencia_id, referencia_tipo,
                    prev_hash, hash, created_at
             FROM livro_caixa
             WHERE sessao_id = ?1
             ORDER BY sequencia ASC",
        )?;
        let rows = stmt.query_map([&sessao_id], |row| {
            let valor_centavos: i64 = row.get(2)?;
            let troco_centavos: i64 = row.get(3)?;
            Ok(crate::dto::response::LivroEntradaResponse {
                sequencia: row.get(0)?,
                tipo: row.get(1)?,
                valor: Money::from_centavos(valor_centavos).to_string(),
                troco: Money::from_centavos(troco_centavos).to_string(),
                usuario_id: row.get(4)?,
                aprovador_id: row.get(5)?,
                motivo: row.get(6)?,
                referencia_id: row.get(7)?,
                referencia_tipo: row.get(8)?,
                prev_hash: row.get(9)?,
                hash: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?;
        rows.collect()
    })();

    let entradas = match entradas {
        Ok(e) => e,
        Err(e) => {
            tracing::error!(erro = %e, sessao_id = %sessao_id, "erro ao ler livro-caixa");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha de banco" })),
            );
        }
    };

    let saldo_total: i64 = match db.conn().query_row(
        "SELECT COALESCE(SUM(valor_centavos - troco_centavos), 0) FROM livro_caixa WHERE sessao_id = ?1",
        [&sessao_id],
        |r| r.get(0),
    ) {
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
