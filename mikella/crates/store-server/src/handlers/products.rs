//! Handlers de cadastro de produtos (MVP-1 — contínuo).
//!
//! Casos de uso de produto via `app-inventory::produto`:
//! - criar_produto (POST /v1/produtos)
//! - atualizar_produto (PUT /v1/produtos/:id)
//! - arquivar_produto (POST /v1/produtos/:id/arquivar)
//! - buscar_produto (GET /v1/produtos/:id)
//!
//! A validação de unicidade (código_barras, SKU) fica no domínio; a regra:
//!   - "operação deve ser impossível de ser desfazível" (dado escrito).
//!
//! Clientes experientes geram `uuid-v4` antes de enviar (idempotência parcial).
//! Arquivamento jamais apaga fisicamente — apenas marca `arquivado = true`.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use rusqlite::Row;

use mikella_app_inventory::produto::{
    arquivar_produto, atualizar_produto, criar_produto, AtualizarProdutoCmd, CriarProdutoCmd,
    ProdutoRepos,
};
use mikella_domain_inventory::produto::Unidade;
use mikella_infra_sqlite::ProdutoRepo;
use mikella_shared_kernel::ids::{CategoriaId, ProdutoId};
use mikella_shared_kernel::money::Money;

use crate::dto::{AtualizarProdutoForm, CriarProdutoForm, ProdutoResponse};
use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::util::{bad_request, error_para_http};

/// SQL de seleção de produto (reusado por handlers).
pub(crate) const SQL_PRODUTO_POR_ID: &str = "\
SELECT p.id, p.codigo_barras, p.sku, p.nome, p.categoria_id, p.unidade,
       p.preco_centavos, p.custo_centavos, p.estoque_minimo,
       p.perecivel, p.alerta_validade_dias, p.arquivado, p.arquivado_em,
       COALESCE(s.saldo_atual, 0), p.created_at, p.updated_at
FROM produtos p
LEFT JOIN vw_estoque_atual s ON s.produto_id = p.id";

/// Handler auxiliar: converte uma row SQL em ProdutoResponse.
///
/// Os índices de coluna são baseados em `SQL_PRODUTO_POR_ID`.
pub(crate) fn row_para_produto_response(row: &Row) -> rusqlite::Result<ProdutoResponse> {
    let id: String = row.get(0)?;
    let codigo_barras: Option<String> = row.get(1)?;
    let sku: Option<String> = row.get(2)?;
    let nome: String = row.get(3)?;
    let categoria_id: Option<String> = row.get(4)?;
    let unidade_str: String = row.get(5)?;
    let preco_centavos: i64 = row.get(6)?;
    let custo_centavos: i64 = row.get(7)?;
    let estoque_minimo: i64 = row.get(8)?;
    let perecivel: i32 = row.get(9)?;
    let alerta_validade_dias: i64 = row.get(10)?;
    let arquivado: i32 = row.get(11)?;
    let arquivado_em: Option<String> = row.get(12)?;
    let saldo_atual: i64 = row.get(13)?;
    let created_at: String = row.get(14)?;
    let updated_at: String = row.get(15)?;

    let unidade = match unidade_str.as_str() {
        "UN" => Unidade::Un,
        "KG" => Unidade::Kg,
        "L" => Unidade::L,
        "CX" => Unidade::Cx,
        "PCT" => Unidade::Pct,
        outro => Unidade::Outro(outro.to_string()),
    };

    Ok(ProdutoResponse {
        id,
        codigo_barras,
        sku,
        nome,
        categoria_id,
        unidade: unidade.as_str().to_string(),
        preco_centavos,
        preco_display: Money::from_centavos(preco_centavos).to_string(),
        custo_centavos,
        estoque_minimo: i32::try_from(estoque_minimo)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(8, estoque_minimo))?,
        perecivel: perecivel != 0,
        alerta_validade_dias: u32::try_from(alerta_validade_dias)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(10, alerta_validade_dias))?,
        arquivado: arquivado != 0,
        arquivado_em,
        estoque_atual: i32::try_from(saldo_atual)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(13, saldo_atual))?,
        created_at,
        updated_at,
    })
}

/// Converte ProdutoResponse em JSON `Value` para arms de match mistos.
pub(crate) fn produto_para_json(produto: &ProdutoResponse) -> serde_json::Value {
    serde_json::to_value(produto).unwrap_or_else(|_| serde_json::json!({}))
}

/// Busca um produto pelo ID, retornando a response completa.
fn buscar_produto_response(
    conn: &rusqlite::Connection,
    id: &str,
) -> rusqlite::Result<ProdutoResponse> {
    conn.query_row(
        &format!("{} WHERE p.id = ?1", SQL_PRODUTO_POR_ID),
        [id],
        row_para_produto_response,
    )
}

/// Busca produto pelo ID em transação (para usar dentro de tx).
fn buscar_produto_response_tx(
    tx: &rusqlite::Transaction,
    id: &str,
) -> rusqlite::Result<ProdutoResponse> {
    tx.query_row(
        &format!("{} WHERE p.id = ?1", SQL_PRODUTO_POR_ID),
        [id],
        row_para_produto_response,
    )
}

// ─── DTO → Cmd mapping helpers ────────────────────────────────────────────

/// Mapeia a `unidade` string do formulário para o enum do domínio.
fn parse_unidade(valor: &str) -> Unidade {
    match valor.to_uppercase().as_str() {
        "UN" => Unidade::Un,
        "KG" => Unidade::Kg,
        "L" => Unidade::L,
        "CX" => Unidade::Cx,
        "PCT" => Unidade::Pct,
        outro => Unidade::Outro(outro.to_string()),
    }
}

/// Converte um UUID string do formulário para `CategoriaId`.
/// Retorna `Ok(Some(id))` se parseável, `Ok(None)` se ausente,
/// `Err(msg)` se UUID inválido (o handler converte para HTTP 400).
fn parse_categoria_id(id_str: &Option<String>) -> Result<Option<CategoriaId>, String> {
    match id_str {
        Some(s) => s
            .parse()
            .map(Some)
            .map_err(|_| "categoria_id inválido (UUID esperado)".into()),
        None => Ok(None),
    }
}

/// Handler auxiliar: converte erro de parse para HTTP 400.
fn categoria_id_err_para_http(e: String) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"erro": e})),
    )
}

/// `POST /v1/produtos` — cria um produto no catálogo.
///
/// O `operador_id` é derivado do usuário autenticado — nunca do cliente.
/// Idempotência parcial: reenvio do mesmo `produto_id` pode falhar com 409
/// se outro processo já cadastrou o mesmo ID simultaneamente.
pub async fn criar_produto_handler(
    State(state): State<AppState>,
    ator: Ator,
    Json(form): Json<CriarProdutoForm>,
) -> impl IntoResponse {
    exigir!(ator, "produto.criar");
    tracing::debug!(usuario = %ator.0.username, produto_id = %form.produto_id, "criando produto");

    let produto_id: ProdutoId = match form.produto_id.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("produto_id inválido (UUID esperado)"),
    };

    let categoria_id = match parse_categoria_id(&form.categoria_id) {
        Ok(opt) => opt,
        Err(e) => return categoria_id_err_para_http(e),
    };

    let cmd = CriarProdutoCmd {
        produto_id,
        nome: form.nome,
        preco: Money::from_centavos(form.preco_centavos),
        codigo_barras: form.codigo_barras,
        sku: form.sku,
        categoria_id,
        unidade: parse_unidade(&form.unidade),
        custo: Money::from_centavos(form.custo_centavos),
        estoque_minimo: form.estoque_minimo,
        perecivel: form.perecivel,
        alerta_validade_dias: form.alerta_validade_dias,
    };

    let mut db = state.db.lock().await;

    let tx = match db.transacao() {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao iniciar transação");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"erro": "falha de banco"})),
            );
        }
    };

    let produtos_repo = ProdutoRepo::novo(&tx);
    let repos = ProdutoRepos {
        produtos: &produtos_repo,
    };

    match criar_produto(&repos, cmd) {
        Ok(produto) => {
            let produto_response = match buscar_produto_response_tx(&tx, &produto.id.to_string()) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(erro = %e, produto_id = %produto.id, "falha ao reler produto");
                    let _ = tx.rollback();
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"erro": "falha de banco"})),
                    );
                }
            };

            if tx.commit().is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"erro": "falha ao confirmar produto"})),
                );
            }

            tracing::info!(produto_id = %produto.id, "produto criado com sucesso");
            (
                StatusCode::CREATED,
                Json(produto_para_json(&produto_response)),
            )
        }
        Err(e) => {
            let _ = tx.rollback();
            error_para_http(e)
        }
    }
}

/// `GET /v1/produtos/:id` — busca produto por ID (ativa ou arquivada).
pub async fn buscar_produto_handler(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
    ator: Ator,
) -> impl IntoResponse {
    exigir!(ator, "produto.ver");
    tracing::debug!(usuario = %ator.0.username, id = %id_str, "buscando produto");

    if id_str.parse::<ProdutoId>().is_err() {
        return bad_request("produto_id inválido (UUID esperado)");
    }

    let db = state.db.lock().await;

    let result = buscar_produto_response(db.conn(), &id_str);

    match result {
        Ok(produto) => (StatusCode::OK, Json(produto_para_json(&produto))),
        Err(rusqlite::Error::QueryReturnedNoRows) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"erro": "produto não encontrado"})),
        ),
        Err(e) => {
            tracing::error!(erro = %e, "erro ao buscar produto");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"erro": "falha de banco"})),
            )
        }
    }
}

/// `PUT /v1/produtos/:id` — atualização completa (substitui todos os campos mutáveis).
pub async fn atualizar_produto_handler(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
    ator: Ator,
    Json(form): Json<AtualizarProdutoForm>,
) -> impl IntoResponse {
    exigir!(ator, "produto.atualizar");
    tracing::debug!(usuario = %ator.0.username, id = %id_str, "atualizando produto");

    let produto_id: ProdutoId = match id_str.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("produto_id inválido na URL"),
    };

    if form.nome.trim().is_empty() {
        return bad_request("nome não pode ser vazio");
    }

    let categoria_id = match parse_categoria_id(&form.categoria_id) {
        Ok(opt) => opt,
        Err(e) => return categoria_id_err_para_http(e),
    };

    let cmd = AtualizarProdutoCmd {
        produto_id,
        codigo_barras: form.codigo_barras,
        sku: form.sku,
        nome: form.nome,
        categoria_id,
        unidade: parse_unidade(&form.unidade),
        preco: Money::from_centavos(form.preco_centavos),
        custo: Money::from_centavos(form.custo_centavos),
        estoque_minimo: form.estoque_minimo,
        perecivel: form.perecivel,
        alerta_validade_dias: form.alerta_validade_dias,
    };

    let mut db = state.db.lock().await;

    let tx = match db.transacao() {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao iniciar transação");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"erro": "falha de banco"})),
            );
        }
    };

    let produtos_repo = ProdutoRepo::novo(&tx);
    let repos = ProdutoRepos {
        produtos: &produtos_repo,
    };

    match atualizar_produto(&repos, cmd) {
        Ok(produto) => {
            let produto_response = match buscar_produto_response_tx(&tx, &produto.id.to_string()) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(erro = %e, produto_id = %produto.id, "falha ao reler produto");
                    let _ = tx.rollback();
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"erro": "falha de banco"})),
                    );
                }
            };

            if tx.commit().is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"erro": "falha ao confirmar atualização"})),
                );
            }

            tracing::info!(produto_id = %produto.id, "produto atualizado");
            (StatusCode::OK, Json(produto_para_json(&produto_response)))
        }
        Err(e) => {
            let _ = tx.rollback();
            error_para_http(e)
        }
    }
}

/// `POST /v1/produtos/:id/arquivar` — marca o produto como arquivado (soft delete).
///
/// Nunca deleta fisicamente — apenas marca `arquivado = true` + `arquivado_em`.
/// Produtos arquivados saem da listagem `GET /v1/produtos` automaticamente.
pub async fn arquivar_produto_handler(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
    ator: Ator,
) -> impl IntoResponse {
    exigir!(ator, "produto.arquivar");
    tracing::debug!(usuario = %ator.0.username, id = %id_str, "arquivando produto");

    let produto_id: ProdutoId = match id_str.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("produto_id inválido na URL"),
    };

    let mut db = state.db.lock().await;

    let tx = match db.transacao() {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao iniciar transação");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"erro": "falha de banco"})),
            );
        }
    };

    let produtos_repo = ProdutoRepo::novo(&tx);
    let repos = ProdutoRepos {
        produtos: &produtos_repo,
    };

    match arquivar_produto(&repos, produto_id) {
        Ok(_) => {
            if tx.commit().is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"erro": "falha ao confirmar arquivamento"})),
                );
            }

            tracing::info!(produto_id = %produto_id, "produto arquivado");
            (
                StatusCode::OK,
                Json(serde_json::json!({"arquivado": true, "id": produto_id.to_string()})),
            )
        }
        Err(e) => {
            let _ = tx.rollback();
            error_para_http(e)
        }
    }
}
