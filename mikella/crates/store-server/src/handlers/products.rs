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

use mikella_app_inventory::produto::{
    arquivar_produto, atualizar_produto, criar_produto, AtualizarProdutoCmd, CriarProdutoCmd,
    ProdutoRepos,
};
use mikella_domain_inventory::produto::Unidade;
use mikella_infra_sqlite::{ProdutoRepo, ProdutoView};
use mikella_shared_kernel::ids::{CategoriaId, ProdutoId};
use mikella_shared_kernel::money::Money;

use crate::dto::{AtualizarProdutoForm, CriarProdutoForm, ProdutoResponse};
use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::util::{bad_request, error_para_http};

/// Converte a linha crua do read model em DTO de apresentação.
pub(crate) fn view_para_response(view: ProdutoView) -> Result<ProdutoResponse, String> {
    let estoque_minimo = i32::try_from(view.estoque_minimo)
        .map_err(|_| "estoque_minimo fora do intervalo suportado".to_string())?;
    let alerta_validade_dias = u32::try_from(view.alerta_validade_dias)
        .map_err(|_| "alerta_validade_dias fora do intervalo suportado".to_string())?;
    let estoque_atual = i32::try_from(view.saldo_atual)
        .map_err(|_| "saldo de estoque fora do intervalo suportado".to_string())?;

    let unidade = match view.unidade.as_str() {
        "UN" => Unidade::Un,
        "KG" => Unidade::Kg,
        "L" => Unidade::L,
        "CX" => Unidade::Cx,
        "PCT" => Unidade::Pct,
        outro => Unidade::Outro(outro.to_string()),
    };

    Ok(ProdutoResponse {
        id: view.id,
        codigo_barras: view.codigo_barras,
        sku: view.sku,
        nome: view.nome,
        categoria_id: view.categoria_id,
        unidade: unidade.as_str().to_string(),
        preco_centavos: view.preco_centavos,
        preco_display: Money::from_centavos(view.preco_centavos).to_string(),
        custo_centavos: view.custo_centavos,
        estoque_minimo,
        perecivel: view.perecivel,
        alerta_validade_dias,
        arquivado: view.arquivado,
        arquivado_em: view.arquivado_em,
        estoque_atual,
        created_at: view.created_at,
        updated_at: view.updated_at,
    })
}

/// Converte ProdutoResponse em JSON `Value` para arms de match mistos.
pub(crate) fn produto_para_json(produto: &ProdutoResponse) -> serde_json::Value {
    serde_json::to_value(produto).unwrap_or_else(|_| serde_json::json!({}))
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
    tracing::debug!(usuario = %ator.0.username, produto_id = ?form.produto_id, "criando produto");

    let produto_id: ProdutoId = match form.produto_id.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(s) => match s.parse() {
            Ok(id) => id,
            Err(_) => return bad_request("produto_id inválido (UUID esperado)"),
        },
        None => ProdutoId::new(),
    };

    let categoria_id = match parse_categoria_id(&form.categoria_id) {
        Ok(opt) => opt,
        Err(e) => return categoria_id_err_para_http(e),
    };

    let custo_centavos = form.custo_centavos;
    let qtd_inicial = form.quantidade_inicial.unwrap_or(0);

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
            if qtd_inicial > 0 {
                let mov_id = uuid::Uuid::new_v4().to_string();
                if let Err(e) = tx.execute(
                    "INSERT INTO movimentacoes_estoque
                     (id, produto_id, lote_id, tipo, quantidade, custo_unitario_centavos, referencia_id, referencia_tipo, usuario_id, motivo)
                     VALUES (?1, ?2, NULL, 'ENTRADA_COMPRA', ?3, ?4, NULL, 'CADASTRO_INICIAL', ?5, 'Estoque inicial no cadastro')",
                    rusqlite::params![
                        mov_id,
                        produto.id.to_string(),
                        qtd_inicial,
                        custo_centavos,
                        ator.id().to_string(),
                    ],
                ) {
                    tracing::error!(erro = %e, "falha ao lançar movimentação de estoque inicial");
                    let _ = tx.rollback();
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"erro": "falha ao registrar estoque inicial"})),
                    );
                }
            }

            let produto_response = match produtos_repo
                .buscar_view(&produto.id.to_string())
                .map_err(|e| e.to_string())
                .and_then(|view| {
                    view.ok_or_else(|| "produto recém-gravado não encontrado".to_string())
                })
                .and_then(view_para_response)
            {
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

            let audit = mikella_infra_sqlite::AuditoriaRepo::novo(&tx);
            let _ = audit.registrar(
                "produto",
                &produto.id.to_string(),
                "CRIACAO_PRODUTO",
                Some(&ator.id().to_string()),
                None,
                None,
                Some(&format!("nome: {}, preco_centavos: {}", produto.nome, produto.preco_centavos.centavos())),
            );

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

    let result = ProdutoRepo::novo(db.conn()).buscar_view(&id_str);

    match result {
        Ok(Some(view)) => match view_para_response(view) {
            Ok(produto) => (StatusCode::OK, Json(produto_para_json(&produto))),
            Err(e) => {
                tracing::error!(erro = %e, "produto com valores fora do intervalo");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"erro": "falha de banco"})),
                )
            }
        },
        Ok(None) => (
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
            let produto_response = match produtos_repo
                .buscar_view(&produto.id.to_string())
                .map_err(|e| e.to_string())
                .and_then(|view| {
                    view.ok_or_else(|| "produto recém-gravado não encontrado".to_string())
                })
                .and_then(view_para_response)
            {
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

            let audit = mikella_infra_sqlite::AuditoriaRepo::novo(&tx);
            let _ = audit.registrar(
                "produto",
                &produto.id.to_string(),
                "ALTERACAO_PRODUTO",
                Some(&ator.id().to_string()),
                None,
                None,
                Some(&format!("nome: {}, preco_centavos: {}", produto.nome, produto.preco_centavos.centavos())),
            );

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
            let audit = mikella_infra_sqlite::AuditoriaRepo::novo(&tx);
            let _ = audit.registrar(
                "produto",
                &produto_id.to_string(),
                "ARQUIVAMENTO_PRODUTO",
                Some(&ator.id().to_string()),
                None,
                None,
                None,
            );

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

#[cfg(test)]
mod tests {
    use super::*;
    use mikella_infra_sqlite::Database;
    use uuid::Uuid;

    #[test]
    fn produto_com_estoque_inicial_gera_saldo_na_view() {
        let db = Database::em_memoria().unwrap();
        let prod_id = Uuid::new_v4().to_string();
        let user_id = Uuid::new_v4().to_string();

        db.conn()
            .execute(
                "INSERT INTO usuarios (id, username, nome, senha_hash, papel, ativo)
                 VALUES (?1, 'operador_teste', 'Operador Teste', 'hash', 'OPERADOR_CAIXA', 1)",
                rusqlite::params![user_id],
            )
            .unwrap();

        db.conn()
            .execute(
                "INSERT INTO produtos (id, nome, preco_centavos, unidade, arquivado)
                 VALUES (?1, 'Feijão Carioca 1kg', 850, 'PCT', 0)",
                rusqlite::params![prod_id],
            )
            .unwrap();

        let mov_id = Uuid::new_v4().to_string();
        db.conn()
            .execute(
                "INSERT INTO movimentacoes_estoque
                 (id, produto_id, lote_id, tipo, quantidade, custo_unitario_centavos, referencia_id, referencia_tipo, usuario_id, motivo)
                 VALUES (?1, ?2, NULL, 'ENTRADA_COMPRA', 25, 450, NULL, 'CADASTRO_INICIAL', ?3, 'Estoque inicial')",
                rusqlite::params![mov_id, prod_id, user_id],
            )
            .unwrap();

        let repo = ProdutoRepo::novo(db.conn());
        let view = repo.buscar_view(&prod_id).unwrap().unwrap();
        assert_eq!(view.saldo_atual, 25);
    }
}
