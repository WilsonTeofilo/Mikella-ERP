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
                "OUTRO" => FormaPagamento::Outro,
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
        comanda_id: None,
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
