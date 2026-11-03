//! Caso de uso: finalizar venda.
//!
//! Este é o caminho crítico do sistema. Tudo acontece dentro da transação
//! aberta pelo chamador (servidor/Tauri):
//!
//!   1. idempotência pelo `venda_id` (UUID gerado antes do envio);
//!   2. validação da sessão de caixa e do dispositivo;
//!   3. preço vem do banco — nunca da UI;
//!   4. baixa de estoque por movimentação imutável;
//!   5. lançamentos no livro do caixa (dinheiro e eletrônico);
//!   6. projeções (saldo do lote e do caixa) via trigger na mesma transação.
//!
//! Se qualquer passo falhar, o chamador dá rollback e nada fica gravado.

use std::collections::HashMap;

use chrono::DateTime;
use chrono::Utc;

use mikella_domain_cash::livro::{EntradaLivro, Referencia, TipoMovimentacao};
use mikella_domain_cash::ports::{HashChain, LivroCaixaRepository, SessaoRepository};
use mikella_domain_inventory::lote::Lote;
use mikella_domain_inventory::movimentacao::MovimentacaoEstoque;
use mikella_domain_inventory::ports::{
    LoteRepository, MovimentacaoEstoqueRepository, ProdutoRepository,
};
use mikella_domain_inventory::produto::Produto;
use mikella_domain_sales::item::ItemVenda;
use mikella_domain_sales::ports::VendaRepository;
use mikella_domain_sales::venda::{FormaPagamento, Pagamento, Venda, VendaBuilder};
use mikella_shared_kernel::clock::Clock;
use mikella_shared_kernel::error::{DomainError, DomainResult};
use mikella_shared_kernel::ids::{
    DispositivoId, LivroCaixaId, LoteId, MovimentacaoEstoqueId, ProdutoId, SessaoId, UsuarioId,
    VendaId,
};
use mikella_shared_kernel::money::Money;

/// Política de estoque quando a quantidade vendida passa do saldo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PoliticaEstoque {
    /// Recusa a venda (recomendado para quem controla estoque de verdade).
    #[default]
    Bloquear,
    /// Permite vender e deixa o saldo negativo (loja que ainda não confia no estoque).
    PermitirNegativo,
}

/// Um item pedido pela UI. Preço e desconto NÃO vêm daqui.
#[derive(Debug, Clone)]
pub struct ItemCmd {
    pub produto_id: ProdutoId,
    pub lote_id: Option<LoteId>,
    pub quantidade: u32,
}

/// Comando de finalização de venda.
#[derive(Debug)]
pub struct FinalizarVendaCmd {
    /// UUID gerado no cliente antes do envio — é a chave de idempotência.
    pub venda_id: VendaId,
    pub sessao_id: SessaoId,
    pub dispositivo_id: DispositivoId,
    pub operador_id: UsuarioId,
    pub vendedor_id: Option<UsuarioId>,
    pub comanda_id: Option<String>,
    pub itens: Vec<ItemCmd>,
    pub desconto: Money,
    /// Pagamentos já montados. A soma tem de fechar com o total calculado
    /// pelo servidor; `VendaBuilder` valida.
    pub pagamentos: Vec<Pagamento>,
    pub politica_estoque: PoliticaEstoque,
}

/// Dependências do caso de uso. O chamador monta com os repositórios SQLite
/// dentro da mesma transação.
pub struct VendaRepos<'a> {
    pub produtos: &'a dyn ProdutoRepository,
    pub lotes: &'a dyn LoteRepository,
    pub vendas: &'a dyn VendaRepository,
    pub sessoes: &'a dyn SessaoRepository,
    pub livro: &'a dyn LivroCaixaRepository,
    pub estoque: &'a dyn MovimentacaoEstoqueRepository,
    pub hash: &'a dyn HashChain,
    pub clock: &'a dyn Clock,
}

pub fn finalizar_venda(repos: &VendaRepos<'_>, cmd: FinalizarVendaCmd) -> DomainResult<Venda> {
    // 1. Idempotência: reenvio do mesmo UUID devolve a venda já gravada,
    //    mas só se o resto da requisição bate — evita que um caller
    //    reenvie `venda_id` de outra sessão e "apague" uma venda alheia.
    if let Some(existente) = repos.vendas.buscar_por_id(cmd.venda_id)? {
        if existente.sessao_id != cmd.sessao_id || existente.operador_id != cmd.operador_id {
            return Err(DomainError::EstadoInvalido(
                "Reenvio de venda_id com sessão/operador incompatível".into(),
            ));
        }
        return Ok(existente);
    }

    // 2. Sessão de caixa.
    let sessao =
        repos
            .sessoes
            .buscar_por_id(cmd.sessao_id)?
            .ok_or_else(|| DomainError::NaoEncontrado {
                entidade: "SessaoCaixa",
                id: cmd.sessao_id.to_string(),
            })?;

    if !sessao.is_aberta() {
        return Err(DomainError::EstadoInvalido(
            "Não é possível vender em uma sessão que não está aberta".into(),
        ));
    }
    if sessao.dispositivo_id != cmd.dispositivo_id {
        return Err(DomainError::PermissaoNegada(
            "A sessão pertence a outro dispositivo".into(),
        ));
    }

    // RF-012/RF-014: apenas o operador da sessão vende nela.
    if sessao.operador_id != cmd.operador_id {
        return Err(DomainError::PermissaoNegada(
            "A venda só pode ser feita na sessão do operador autenticado".into(),
        ));
    }

    let agora = repos.clock.now();

    // 3. Itens com preço do servidor.
    let mut builder = VendaBuilder::novo(
        cmd.venda_id,
        cmd.sessao_id,
        cmd.dispositivo_id,
        cmd.operador_id,
    );
    if let Some(vendedor) = cmd.vendedor_id {
        builder = builder.vendedor(vendedor);
    }
    if let Some(comanda) = cmd.comanda_id {
        builder = builder.comanda(comanda);
    }

    let mut custos: HashMap<ProdutoId, Money> = HashMap::new();

    for pedido in &cmd.itens {
        let produto = carregar_produto(repos, pedido.produto_id)?;
        validar_lote(
            repos,
            &produto,
            pedido.lote_id,
            pedido.quantidade,
            cmd.politica_estoque,
            agora,
        )?;
        validar_estoque(repos, &produto, pedido.quantidade, cmd.politica_estoque)?;

        custos.insert(produto.id, produto.custo_centavos);

        let item = ItemVenda::novo(
            produto.id,
            pedido.lote_id,
            pedido.quantidade,
            produto.preco_centavos,
            Money::ZERO,
        )?;
        builder = builder.adicionar_item(item);
    }

    for pagamento in cmd.pagamentos {
        builder = builder.pagamento(pagamento);
    }
    builder = builder.desconto_venda(cmd.desconto);

    // Recalcula subtotal/total e valida soma dos pagamentos.
    let venda = builder.build(agora)?;

    // 4. Persiste cabeçalho + itens + pagamentos.
    repos.vendas.salvar(&venda)?;

    // 5. Baixa de estoque (imutável).
    for item in &venda.itens {
        let custo = custos.get(&item.produto_id).copied().unwrap_or(Money::ZERO);
        let movimentacao = MovimentacaoEstoque::saida_venda(
            MovimentacaoEstoqueId::new(),
            item.produto_id,
            item.lote_id,
            item.quantidade,
            custo,
            venda.id,
            venda.operador_id,
            agora,
        )?;
        repos.estoque.registrar(&movimentacao)?;
    }

    // 6. Livro do caixa.
    registrar_livro_da_venda(repos, &venda, agora)?;

    tracing::info!(
        venda_id = %venda.id,
        sessao_id = %venda.sessao_id,
        total_centavos = venda.total.centavos(),
        "venda finalizada"
    );

    Ok(venda)
}

fn carregar_produto(repos: &VendaRepos<'_>, id: ProdutoId) -> DomainResult<Produto> {
    let produto = repos
        .produtos
        .buscar_por_id(id)?
        .ok_or_else(|| DomainError::NaoEncontrado {
            entidade: "Produto",
            id: id.to_string(),
        })?;

    if !produto.esta_ativo() {
        return Err(DomainError::EstadoInvalido(format!(
            "Produto {} está arquivado e não pode ser vendido",
            produto.nome
        )));
    }
    Ok(produto)
}

fn validar_lote(
    repos: &VendaRepos<'_>,
    produto: &Produto,
    lote_id: Option<LoteId>,
    quantidade: u32,
    politica: PoliticaEstoque,
    agora: DateTime<Utc>,
) -> DomainResult<()> {
    let Some(lote_id) = lote_id else {
        if produto.perecivel {
            return Err(DomainError::Invalido(format!(
                "Produto perecível exige lote: {}",
                produto.nome
            )));
        }
        return Ok(());
    };

    let lote = repos
        .lotes
        .buscar_por_id(lote_id)?
        .ok_or_else(|| DomainError::NaoEncontrado {
            entidade: "Lote",
            id: lote_id.to_string(),
        })?;

    validar_lote_do_produto(&lote, produto, agora)?;

    if politica == PoliticaEstoque::Bloquear {
        let disponivel = repos.lotes.estoque_atual(lote_id)?;
        if i64::from(disponivel) < i64::from(quantidade) {
            return Err(DomainError::Invalido(format!(
                "Estoque insuficiente no lote de {}: disponível {}, pedido {}",
                produto.nome, disponivel, quantidade
            )));
        }
    }

    Ok(())
}

fn validar_lote_do_produto(
    lote: &Lote,
    produto: &Produto,
    agora: DateTime<Utc>,
) -> DomainResult<()> {
    if lote.produto_id != produto.id {
        return Err(DomainError::Invalido(
            "O lote informado pertence a outro produto".into(),
        ));
    }
    if !lote.ativo {
        return Err(DomainError::EstadoInvalido("Lote inativo".into()));
    }
    if let Some(validade) = lote.validade {
        if validade < agora.date_naive() {
            return Err(DomainError::EstadoInvalido(format!(
                "Lote vencido em {} — não pode ser vendido",
                validade.format("%d/%m/%Y")
            )));
        }
    }
    Ok(())
}

fn validar_estoque(
    repos: &VendaRepos<'_>,
    produto: &Produto,
    quantidade: u32,
    politica: PoliticaEstoque,
) -> DomainResult<()> {
    if politica == PoliticaEstoque::PermitirNegativo {
        return Ok(());
    }
    let disponivel = repos.produtos.estoque_atual(produto.id)?;
    if i64::from(disponivel) < i64::from(quantidade) {
        return Err(DomainError::Invalido(format!(
            "Estoque insuficiente para {}: disponível {}, pedido {}",
            produto.nome, disponivel, quantidade
        )));
    }
    Ok(())
}

fn registrar_livro_da_venda(
    repos: &VendaRepos<'_>,
    venda: &Venda,
    agora: DateTime<Utc>,
) -> DomainResult<()> {
    let posicao = repos.livro.posicao_atual(venda.sessao_id)?;
    let mut proxima_sequencia = posicao.proxima_sequencia;
    let mut ultimo_hash = posicao.ultimo_hash;

    let mut dinheiro_recebido = Money::ZERO;
    let mut dinheiro_troco = Money::ZERO;

    for pagamento in &venda.pagamentos {
        if pagamento.forma.movimenta_dinheiro_fisico() {
            dinheiro_recebido = dinheiro_recebido + pagamento.recebido.unwrap_or(pagamento.valor);
            dinheiro_troco = dinheiro_troco + pagamento.troco();
        }
    }

    if !dinheiro_recebido.is_zero() {
        inserir_entrada(
            repos,
            venda,
            TipoMovimentacao::VendaDinheiro,
            dinheiro_recebido,
            dinheiro_troco,
            &mut proxima_sequencia,
            &mut ultimo_hash,
            agora,
        )?;
    }

    for pagamento in &venda.pagamentos {
        let tipo = match pagamento.forma {
            FormaPagamento::Dinheiro => continue,
            FormaPagamento::Debito => TipoMovimentacao::VendaDebito,
            FormaPagamento::Credito => TipoMovimentacao::VendaCredito,
            FormaPagamento::Pix => TipoMovimentacao::VendaPix,
            FormaPagamento::Outro | FormaPagamento::Misto => TipoMovimentacao::VendaOutro,
        };
        inserir_entrada(
            repos,
            venda,
            tipo,
            pagamento.valor,
            Money::ZERO,
            &mut proxima_sequencia,
            &mut ultimo_hash,
            agora,
        )?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn inserir_entrada(
    repos: &VendaRepos<'_>,
    venda: &Venda,
    tipo: TipoMovimentacao,
    valor: Money,
    troco: Money,
    proxima_sequencia: &mut i64,
    ultimo_hash: &mut String,
    agora: DateTime<Utc>,
) -> DomainResult<()> {
    let mut entrada = EntradaLivro {
        id: LivroCaixaId::new(),
        sessao_id: venda.sessao_id,
        sequencia: *proxima_sequencia,
        tipo,
        valor_centavos: valor,
        troco_centavos: troco,
        referencia: Some(Referencia {
            id: venda.id.to_string(),
            tipo: "VENDA".to_string(),
        }),
        usuario_id: venda.operador_id,
        aprovador_id: None,
        motivo: None,
        prev_hash: ultimo_hash.clone(),
        hash: String::new(),
        created_at: agora,
    };

    entrada.hash = repos.hash.hash_entrada(&entrada);
    repos.livro.inserir_entrada(&entrada)?;

    *ultimo_hash = entrada.hash;
    *proxima_sequencia += 1;
    Ok(())
}
