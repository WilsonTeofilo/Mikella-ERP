//! Handler de finalização de venda (MVP-1).
//!
//! Usa `app_sales::finalizar_venda` com repositórios SQLite dentro de uma
//! transação. O cliente envia IDs e quantidades; preços e custos vêm do banco.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use mikella_app_sales::{finalizar_venda, FinalizarVendaCmd, ItemCmd, PoliticaEstoque, VendaRepos};
use mikella_domain_sales::venda::{FormaPagamento, Pagamento};
use mikella_infra_sqlite::{Blake3HashChain, LivroRepo, LoteRepo, MovimentacaoRepo, ProdutoRepo, SessaoRepo, VendaRepo};
use mikella_shared_kernel::clock::SystemClock;
use mikella_shared_kernel::ids::{DispositivoId, LoteId, ProdutoId, SessaoId, VendaId};
use mikella_shared_kernel::money::Money;

use crate::dto::FinalizarVendaForm;
use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::util::{bad_request, error_para_http, garantir_dispositivo};

/// `POST /v1/vendas` — finaliza uma venda.
///
/// Idempotente: reenvio com o mesmo `venda_id` devolve a venda já finalizada.
/// Preço e estoque vêm do servidor — a UI só envia IDs, quantidades e forma de pagamento.
///
/// Exemplo de request JSON:
/// ```json
/// {
///   "venda_id": "uuid-v4",
///   "sessao_id": "uuid",
///   "dispositivo_id": "uuid",
///   "itens": [{"produto_id": "uuid", "lote_id": null, "quantidade": 2}],
///   "pagamentos": [{"forma": "PIX", "valor_centavos": 25980}],
///   "desconto_centavos": 0,
///   "politica_estoque": "Bloquear"
/// }
/// ```
#[allow(clippy::too_many_lines)]
pub async fn finalizar_venda_handler(
    State(state): State<AppState>,
    ator: Ator,
    Json(form): Json<FinalizarVendaForm>,
) -> impl IntoResponse {
    exigir!(ator, "venda.criar");

    if form.desconto_centavos < 0 {
        return bad_request("desconto não pode ser negativo");
    }
    if form.desconto_centavos > 0 {
        // SEG-022: desconto é decisão do servidor, não um valor livre da UI.
        exigir!(ator, "venda.desconto");
    }

    if form.itens.is_empty() {
        return bad_request("pelo menos um item é obrigatório");
    }
    if form.pagamentos.is_empty() {
        return bad_request("pelo menos um pagamento é obrigatório");
    }

    let venda_id: VendaId = match form.venda_id.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("venda_id inválido (UUID esperado)"),
    };
    let sessao_id: SessaoId = match form.sessao_id.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("sessao_id inválido (UUID esperado)"),
    };
    let dispositivo_id: DispositivoId = match form.dispositivo_id.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("dispositivo_id inválido (UUID esperado)"),
    };

    // operador_id derivado do auth, nunca do cliente
    let operador_id = ator.id();

    // garantir dispositivo
    let dev_str = form.dispositivo_id.clone();
    {
        let db_guard = state.db.lock().await;
        if let Err(e) = garantir_dispositivo(&db_guard, &dev_str) {
            tracing::error!(erro = %e, "falha ao garantir dispositivo");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"erro": "falha de banco"})),
            );
        }
    }

    // itens
    let itens: Vec<ItemCmd> = {
        let mut acc = Vec::with_capacity(form.itens.len());
        for item in &form.itens {
            let produto_id: ProdutoId = match item.produto_id.parse() {
                Ok(id) => id,
                Err(_) => return bad_request("produto_id inválido"),
            };
            let lote_id: Option<LoteId> = match &item.lote_id {
                Some(s) => match s.parse() {
                    Ok(id) => Some(id),
                    Err(_) => return bad_request("lote_id inválido"),
                },
                None => None,
            };
            acc.push(ItemCmd {
                produto_id,
                lote_id,
                quantidade: item.quantidade,
            });
        }
        acc
    };

    // pagamentos
    let pagamentos: Vec<Pagamento> = {
        let mut acc = Vec::with_capacity(form.pagamentos.len());
        for p in &form.pagamentos {
            let forma = match p.forma.to_uppercase().as_str() {
                "DINHEIRO" => FormaPagamento::Dinheiro,
                "DEBITO" => FormaPagamento::Debito,
                "CREDITO" => FormaPagamento::Credito,
                "PIX" => FormaPagamento::Pix,
                "OUTRO" | "VOUCHER" | "CREDIARIO" => FormaPagamento::Outro,
                _ => return bad_request(&format!("forma desconhecida: {}", p.forma)),
            };
            let valor = Money::from_centavos(p.valor_centavos);
            let pag = if forma == FormaPagamento::Dinheiro {
                let recebido_centavos = p.recebido_centavos.unwrap_or(p.valor_centavos);
                let recebido = Money::from_centavos(recebido_centavos);
                match Pagamento::dinheiro(valor, recebido) {
                    Ok(p) => p,
                    Err(e) => return bad_request(&e.to_string()),
                }
            } else {
                match Pagamento::eletronico(forma, valor) {
                    Ok(p) => p,
                    Err(e) => return bad_request(&e.to_string()),
                }
            };
            acc.push(pag);
        }
        acc
    };

    let politica = match form.politica_estoque.to_uppercase().as_str() {
        "BLOQUEAR" => PoliticaEstoque::Bloquear,
        "PERMITIR_NEGATIVO" | "PERMITIRNEGATIVO" => PoliticaEstoque::PermitirNegativo,
        _ => return bad_request("politica_estoque inválida (Bloquear|PermitirNegativo)"),
    };

    let cmd = FinalizarVendaCmd {
        venda_id,
        sessao_id,
        dispositivo_id,
        operador_id,
        vendedor_id: None,
        comanda_id: form.comanda_id.clone(),
        itens,
        desconto: Money::from_centavos(form.desconto_centavos),
        pagamentos,
        politica_estoque: politica,
    };

    // Usar o padrão UnitOfWork para melhor encapsulamento da transação
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

    let produtos = ProdutoRepo::novo(&tx);
    let lotes = LoteRepo::novo(&tx);
    let vendas = VendaRepo::novo(&tx);
    let sessoes = SessaoRepo::novo(&tx);
    let livro = LivroRepo::novo(&tx);
    let estoque = MovimentacaoRepo::novo(&tx);
    let hash = Blake3HashChain;
    let clock = SystemClock;
    let repos = VendaRepos {
        produtos: &produtos,
        lotes: &lotes,
        vendas: &vendas,
        sessoes: &sessoes,
        livro: &livro,
        estoque: &estoque,
        hash: &hash,
        clock: &clock,
    };

    match finalizar_venda(&repos, cmd) {
        Ok(venda) => {
            // Se houve pagamento no CREDIARIO e cliente informado, lança em contas_receber na mesma hora
            for p in &form.pagamentos {
                if p.forma.to_uppercase() == "CREDIARIO" {
                    if let Some(cid) = &form.cliente_id {
                        let conta_id = uuid::Uuid::new_v4().to_string();
                        let desc = format!("Venda Crediário #{}", &form.venda_id[..8.min(form.venda_id.len())]);
                        let _ = tx.execute(
                            "INSERT INTO contas_receber (id, descricao, cliente_id, referencia_id, referencia_tipo, total_centavos) \
                             VALUES (?1, ?2, ?3, ?4, 'VENDA', ?5)",
                            rusqlite::params![conta_id, desc, cid, form.venda_id, p.valor_centavos],
                        );
                        let venc = (chrono::Utc::now() + chrono::Duration::days(30)).format("%Y-%m-%d").to_string();
                        let _ = tx.execute(
                            "INSERT INTO parcelas_receber (conta_id, numero, vencimento, valor_centavos, estado) \
                             VALUES (?1, 1, ?2, ?3, 'PENDENTE')",
                            rusqlite::params![conta_id, venc, p.valor_centavos],
                        );
                    }
                }
            }

            if tx.commit().is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"erro": "falha ao confirmar venda"})),
                );
            }
            tracing::info!(venda_id = %venda.id, total = %venda.total, "venda finalizada");
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "venda_id": venda.id.to_string(),
                    "sessao_id": venda.sessao_id.to_string(),
                    "operador_id": venda.operador_id.to_string(),
                    "estado": venda.estado.as_str(),
                    "forma_pagamento": venda.forma_pagamento.as_str(),
                    "subtotal": venda.subtotal.to_string(),
                    "desconto": venda.desconto_total.to_string(),
                    "total": venda.total.to_string(),
                    "recebido": venda.recebido.to_string(),
                    "troco": venda.troco.to_string(),
                    "itens": venda.itens.iter().map(|i| serde_json::json!({
                        "produto_id": i.produto_id.to_string(),
                        "quantidade": i.quantidade,
                        "subtotal": i.preco_unitario.mul_qty(i.quantidade).map(|m| m.to_string()).unwrap_or_else(|_| "R$ 0,00".to_string()),
                    })).collect::<Vec<_>>(),
                    "pagamentos": venda.pagamentos.iter().map(|p| serde_json::json!({
                        "forma": p.forma.as_str(),
                        "valor": p.valor.to_string(),
                        "recebido": p.recebido.map(|v| v.to_string()).unwrap_or_else(|| p.valor.to_string()),
                    })).collect::<Vec<_>>(),
                })),
            )
        }
        Err(e) => {
            let _ = tx.rollback();
            error_para_http(e)
        }
    }
}

/// `POST /v1/vendas/autorizar-cancelamento`
///
/// Valida se o usuário e senha têm permissão para cancelar compra no caixa atual:
/// - Dono e Gerente podem cancelar em qualquer caixa.
/// - Operador de caixa comum só pode cancelar no seu próprio caixa (onde é o operador da sessão).
pub async fn autorizar_cancelamento_handler(
    State(state): State<AppState>,
    Json(payload): Json<crate::dto::sales::AutorizarCancelamentoPayload>,
) -> impl IntoResponse {
    use rusqlite::OptionalExtension;
    let db = state.db.lock().await;

    let usuario_row: Option<(String, String, String)> = db
        .conn()
        .query_row(
            "SELECT id, senha_hash, papel FROM usuarios WHERE username = ?1 COLLATE NOCASE AND ativo = 1",
            [payload.username.trim()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .unwrap_or(None);

    let (u_id, senha_hash, papel_str) = match usuario_row {
        Some(u) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "erro": "Usuário de autorização não encontrado ou inativo." })),
            );
        }
    };

    if !crate::util::verificar_senha(&payload.senha, &senha_hash) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "erro": "Senha de autorização incorreta." })),
        );
    }

    let papel = match mikella_domain_people::Papel::from_db(&papel_str) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "erro": "Papel do usuário inválido." })),
            );
        }
    };

    // Dono ou Gerente podem cancelar em qualquer caixa
    if papel.nivel() >= 2 {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "autorizado": true,
                "usuario_id": u_id,
                "papel": papel_str,
                "motivo": "Autorizado por perfil gerencial"
            })),
        );
    }

    // Operador comum: só pode cancelar no seu próprio caixa
    let sessao_operador_id: Option<String> = db
        .conn()
        .query_row(
            "SELECT operador_id FROM sessoes_caixa WHERE id = ?1",
            [&payload.sessao_id],
            |r| r.get(0),
        )
        .optional()
        .unwrap_or(None);

    match sessao_operador_id {
        Some(op_id) if op_id == u_id => (
            StatusCode::OK,
            Json(serde_json::json!({
                "autorizado": true,
                "usuario_id": u_id,
                "papel": papel_str,
                "motivo": "Autorizado pelo próprio operador do caixa"
            })),
        ),
        Some(_) => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "erro": "Permissão negada: Um operador só pode cancelar compras no seu próprio caixa. Chame o gerente."
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "erro": "Sessão de caixa não encontrada." })),
        ),
    }
}

/// `POST /v1/vendas/:id/cancelar`
pub async fn cancelar_venda_handler(
    State(state): State<AppState>,
    ator: Ator,
    axum::extract::Path(venda_id): axum::extract::Path<String>,
    Json(payload): Json<crate::dto::sales::CancelarVendaPayload>,
) -> impl IntoResponse {
    exigir!(ator, "venda.criar");

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": e.to_string() })),
            );
        }
    };

    let vid: mikella_shared_kernel::ids::VendaId = match venda_id.parse() {
        Ok(id) => id,
        Err(_) => {
            let _ = tx.rollback();
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "erro": "venda_id inválido (UUID esperado)" })),
            );
        }
    };

    let vendas = mikella_infra_sqlite::VendaRepo::novo(&tx);
    let livro = mikella_infra_sqlite::LivroRepo::novo(&tx);
    let hash = mikella_infra_sqlite::Blake3HashChain;
    let repos = mikella_app_sales::CancelarVendaRepos {
        vendas: &vendas,
        livro: &livro,
        hash: &hash,
    };

    let cmd = mikella_app_sales::CancelarVendaCmd {
        venda_id: vid,
        usuario_id: ator.id(),
        motivo: payload.motivo,
    };

    match mikella_app_sales::cancelar_venda(&repos, cmd) {
        Ok(()) => {
            if let Err(e) = tx.commit() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": format!("Falha ao confirmar cancelamento: {}", e) })),
                );
            }

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "sucesso": true,
                    "mensagem": "Venda cancelada e estornada com sucesso."
                })),
            )
        }
        Err(e) => {
            let _ = tx.rollback();
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "erro": e.to_string() })),
            )
        }
    }
}
