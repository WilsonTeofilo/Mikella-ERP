//! Handler de finalização de venda (v3) - Usando o padrão UnitOfWork.
//!
//! Versão atualizada que utiliza o padrão UnitOfWork para melhor encapsulamento da lógica de transações.
//!
//! O UnitOfWork pattern permite registrar múltiplas operações que devem ser executadas
//! dentro de uma única transação atômica: commit se todas tiverem sucesso, rollback em caso de falha.
//!
//! Esta versão mantém a mesma interface pública mas implementa o transaction management
//! usando o UnitOfWork pattern de forma mais limpa.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use mikella_app_sales::{finalizar_venda, FinalizarVendaCmd, ItemCmd, PoliticaEstoque, VendaRepos};
use mikella_domain_sales::venda::{FormaPagamento, Pagamento};
use mikella_infra_sqlite::{UnitOfWork, ProdutoRepo, LoteRepo, SessaoRepo, VendaRepo, LivroRepo, MovimentacaoRepo, Blake3HashChain};
use mikella_shared_kernel::clock::SystemClock;
use mikella_shared_kernel::ids::{DispositivoId, LoteId, ProdutoId, SessaoId, VendaId};
use mikella_shared_kernel::money::Money;

use crate::dto::FinalizarVendaForm;
use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::util::{bad_request, error_para_http, garantir_dispositivo};

/// `POST /v1/vendas` — finaliza uma venda (v3).
///
/// Idempotente: reenvio com o mesmo `venda_id` devolve a venda já finalizada.
/// Preço e estoque vêm do servidor — a UI só envia IDs, quantidades e forma de pagamento.
///
/// Versão otimizada para uso com UnitOfWork padrão.
#[allow(clippy::too_many_lines)]
pub async fn finalizar_venda_handler_v3(
    State(state): State<AppState>,
    ator: Ator,
    Json(form): Json<FinalizarVendaForm>,
) -> impl IntoResponse {
    exigir!(ator, "venda.criar");

    if form.desconto_centavos < 0 {
        return bad_request("desconto não pode ser negativo");
    }
    if form.desconto_centavos > 0 {
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
    let dispositivo_id: DispositivoId = match garantir_dispositivo(&form).parse() {
        Ok(id) => id,
        Err(_) => return bad_request("dispositivo_id inválido (UUID esperado)"),
    };

    let operador_id = match Ator::obter_operador_id(ator) {
        Some(id) => id,
        None => return bad_request("operador_id não encontrado na sessão"),
    };

    // Preparar o comando para finalização da venda
    let itens: Vec<ItemCmd> = form.itens.iter().map(|item| {
        let produto_id: ProdutoId = item.produto_id.parse().unwrap();
        let lote_id = item.lote_id.as_ref().map(|l| l.parse().unwrap());
        ItemCmd {
            produto_id,
            lote_id,
            quantidade: item.quantidade,
        }
    }).collect();

    let pagamentos: Vec<Pagamento> = form.pagamentos.iter().map(|p| {
        let forma = match p.forma.to_uppercase().as_str() {
            "DINHEIRO" => FormaPagamento::Dinheiro,
            "DEBITO" => FormaPagamento::Debito,
            "CREDITO" => FormaPagamento::Credito,
            "PIX" => FormaPagamento::Pix,
            "OUTRO" => FormaPagamento::Outro,
            _ => panic!("forma desconhecida: {}", p.forma),
        };
        let valor = Money::from_centavos(p.valor_centavos);
        match Pagamento::eletronico(forma, valor) {
            Ok(pag) => pag,
            Err(e) => panic!("pagamento inválido: {}", e),
        }
    }).collect();

    let politica = match form.politica_estoque.to_uppercase().as_str() {
        "BLOQUEAR" => PoliticaEstoque::Bloquear,
        "PERMITIR_NEGATIVO" | "PERMITIRNEGATIVO" => PoliticaEstoque::PermitirNegativo,
        _ => panic!("politica_estoque inválida (Bloquear|PermitirNegativo)"),
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

    // Usar UnitOfWork para gerenciar transações e operações
    let mut db = state.db.lock().await;
    let conn = db.conn_mut();
    let mut uow = UnitOfWork::new(conn.clone());

    // Usar a trait Extension para Type to UnitOfWork para facilitar o uso
    let mut uow = uow;

    // Executar a operação de venda dentro do unit of work
    let venda_result = {
        let mut uow_guard = uow;
        let mut repos = create_venda_repos(&mut uow_guard);
        finalizar_venda(&repos, cmd)
    };

    match venda_result {
        Ok(venda) => {
            // Commit com rollback automático
            if uow.commit().is_ok() {
                tracing::info!(venda_id = %cmd.venda_id, total = %cmd.desconto, "venda finalizada");
                (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "venda_id": cmd.venda_id.to_string(),
                        "sessao_id": cmd.sessao_id.to_string(),
                        "operador_id": cmd.operador_id.to_string(),
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
            } else {
                tracing::error!(venda_id = %cmd.venda_id, "falha ao confirmar transação");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"erro": "falha ao confirmar transação"})),
                )
            }
        }
        Err(e) => {
            tracing::error!(erro = %e, "falha ao finalizar venda");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"erro": "falha ao finalizar venda"})),
            )
        }
    }
}

fn create_venda_repos(uow: &mut UnitOfWork) -> VendaRepos {
    let produtos_repo = ProdutoRepo::novo(uow.connection());
    let lotes_repo = LoteRepo::novo(uow.connection());
    let vendas_repo = VendaRepo::novo(uow.connection());
    let sessoes_repo = SessaoRepo::novo(uow.connection());
    let livro_repo = LivroRepo::novo(uow.connection());
    let estoque_repo = MovimentacaoRepo::novo(uow.connection());
    let hash = Blake3HashChain;
    let clock = SystemClock;

    VendaRepos {
        produtos: produtos_repo,
        lotes: lotes_repo,
        vendas: vendas_repo,
        sessoes: sessoes_repo,
        livro: livro_repo,
        estoque: estoque_repo,
        hash: &hash,
        clock: &clock,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::{automock, predicate::*};
    use tokio::sync::Mutex;

    #[automock]
    type MockState = AppState;

    #[tokio::test]
    async fn test_finalizar_venda_handler_v3_success() {
        let mut mock_state = MockState::new();
        let ator = Ator {
            username: "operador".to_string(),
            token: "token".to_string(),
            operador_id: Some("operador-uuid".to_string()),
            vendedor_id: None,
            permissoes: vec!["venda.criar".to_string()],
        };

        let form = FinalizarVendaForm {
            venda_id: "venda-uuid".to_string(),
            sessao_id: "sessao-uuid".to_string(),
            dispositivo_id: "dispositivo-uuid".to_string(),
            itens: vec![],
            pagamentos: vec![],
            desconto_centavos: 0,
            politica_estoque: "Bloquear".to_string(),
        };

        let result = finalizar_venda_handler_v3(
            State(mock_state),
            ator,
            Json(form),
        ).await;

        assert_eq!(result.0, StatusCode::OK);
    }
}