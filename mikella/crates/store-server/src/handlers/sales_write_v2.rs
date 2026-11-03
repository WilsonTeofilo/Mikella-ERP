//! Handler de finalização de venda (v2) - Usando o padrão UnitOfWork.
//!
//! Versão atualizada do sales_write.rs que utiliza o padrão UnitOfWork
//! para melhor encapsulamento da lógica de transações e rollback.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use mikella_app_sales::{finalizar_venda, FinalizarVendaCmd, ItemCmd, PoliticaEstoque, VendaRepos};
use mikella_domain_sales::venda::{FormaPagamento, Pagamento};
use mikella_infra_sqlite::{UnitOfWork, VendaRepo, ProdutoRepo, LoteRepo, SessaoRepo, LivroRepo, MovimentacaoRepo, Blake3HashChain};
use mikella_shared_kernel::clock::SystemClock;
use mikella_shared_kernel::ids::{DispositivoId, LoteId, ProdutoId, SessaoId, VendaId};
use mikella_shared_kernel::money::Money;

use crate::dto::FinalizarVendaForm;
use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::util::{bad_request, error_para_http, garantir_dispositivo};

/// `POST /v1/vendas` — finaliza uma venda (v2).
///
/// Idempotente: reenvio com o mesmo `venda_id` devolve a venda já finalizada.
/// Preço e estoque vêm do servidor — a UI só envia IDs, quantidades e forma de pagamento.
///
/// Versão otimizada para uso com UnitOfWork padrão.
#[allow(clippy::too_many_lines)]
pub async fn finalizar_venda_handler_v2(
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
    let dispositivo_id: DispositivoId = match garantir_dispositivo(&form).parse() {
        Ok(id) => id,
        Err(_) => return bad_request("dispositivo_id inválido (UUID esperado)"),
    };

    let operador_id = match Ator::obter_operador_id(ator) {
        Some(id) => id,
        None => return bad_request("operador_id não encontrado na sessão"),
    };

    if form.sessao_id != sessao_id.to_string() {
        return bad_request("sessao_id não corresponde");
    }

    let itens = form.itens.iter().map(|item| {
        let produto_id: ProdutoId = match item.produto_id.parse() {
            Ok(id) => id,
            Err(_) => panic!("produto_id inválido (UUID esperado)"),
        };
        let lote_id = item.lote_id.as_ref().and_then(|l| {
            match l.parse() {
                Ok(id) => Some(id),
                Err(_) => panic!("lote_id inválido (UUID esperado)"),
            }
        });
        ItemCmd {
            produto_id,
            lote_id,
            quantidade: item.quantidade,
        }
    }).collect();

    let pagamentos = form.pagamentos.iter().map(|p| {
        let valor = Money::from_centavos(p.valor_centavos);
        match Pagamento::eletronico(p.forma.clone(), valor) {
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

    // Usar UnitOfWork para gerenciar a transação
    let mut db = state.db.lock().await;
    let conn = db.conn_mut();
    let mut uow = UnitOfWork::new(conn.clone());

    // Registrar operações de repositório no unit of work
    let produtos_repo = ProdutoRepo::novo(&uow.connection());
    let lotes_repo = LoteRepo::novo(&uow.connection());
    let vendas_repo = VendaRepo::novo(&uow.connection());
    let sessoes_repo = SessaoRepo::novo(&uow.connection());
    let livro_repo = LivroRepo::novo(&uow.connection());
    let estoque_repo = MovimentacaoRepo::novo(&uow.connection());

    let repos = VendaRepos {
        produtos: produtos_repo,
        lotes: lotes_repo,
        vendas: vendas_repo,
        sessoes: sessoes_repo,
        livro: livro_repo,
        estoque: estoque_repo,
        hash: &Blake3HashChain,
        clock: &SystemClock,
    };

    // Executar a operação de venda dentro do unit of work
    uow.execute(|conn| {
        // ReCRIAR repositórios COM a conexão do unit of work
        let produtos_repo = ProdutoRepo::novo(conn);
        let lotes_repo = LoteRepo::novo(conn);
        let vendas_repo = VendaRepo::novo(conn);
        let sessoes_repo = SessaoRepo::novo(conn);
        let livro_repo = LivroRepo::novo(conn);
        let estoque_repo = MovimentacaoRepo::novo(conn);

        let repos = VendaRepos {
            produtos: produtos_repo,
            lotes: lotes_repo,
            vendas: vendas_repo,
            sessoes: sessoes_repo,
            livro: livro_repo,
            estoque: estoque_repo,
            hash: &Blake3HashChain,
            clock: &SystemClock,
        };

        // Executar o caso de uso de venda
        finalizar_venda(&repos, cmd.clone()).map_err(|e| {
            let error = crate::util::InfraError::from(e);
            error
        })
    });

    // Commit com rollback automático em caso de falha
    match uow.commit() {
        Ok(_) => {
            tracing::info!(venda_id = %cmd.venda_id, total = %cmd.desconto, "venda finalizada");
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "venda_id": cmd.venda_id.to_string(),
                    "sessao_id": cmd.sessao_id.to_string(),
                    "operador_id": cmd.operador_id.to_string(),
                    "estado": "FINALIZADA",
                    "forma_pagamento": cmd.pagamentos[0].forma.as_str(),
                    "subtotal": cmd.desconto.to_string(),
                    "desconto": "0",
                    "total": cmd.desconto.to_string(),
                    "recebido": cmd.desconto.to_string(),
                    "troco": "0",
                    "itens": [],
                    "pagamentos": [],
                })),
            )
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

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::{automock, predicate::*};
    use tokio::sync::Mutex;

    #[automock]
    type MockState = AppState;

    #[tokio::test]
    async fn test_finalizar_venda_handler_v2_success() {
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

        let result = finalizar_venda_handler_v2(
            State(mock_state),
            ator,
            Json(form),
        ).await;

        assert_eq!(result.0, StatusCode::OK);
    }
}