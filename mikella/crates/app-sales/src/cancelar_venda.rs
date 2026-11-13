//! Caso de uso: cancelar venda finalizada (RF-013).
//!
//! O cancelamento é atômico quando o chamador usa a mesma transação:
//! - a venda vira `CANCELADA`;
//! - o estoque volta (entrada por devolução);
//! - o livro-caixa recebe o estorno de cada forma de pagamento;
//! - o crediário em aberto da venda é cancelado.
//!
//! Vendas de cartão/Pix não devolvem dinheiro à gaveta; o estorno é registrado
//! na mesma forma para o fechamento continuar batendo.

use chrono::{DateTime, Utc};

use mikella_domain_cash::livro::{EntradaLivro, Referencia, TipoMovimentacao};
use mikella_domain_cash::ports::{HashChain, LivroCaixaRepository};
use mikella_domain_finance::ports::ContaReceberRepository;
use mikella_domain_inventory::movimentacao::{MovimentacaoEstoque, OrigemMovimentacao};
use mikella_domain_inventory::ports::{MovimentacaoEstoqueRepository, ProdutoRepository};
use mikella_domain_sales::ports::VendaRepository;
use mikella_domain_sales::venda::{EstadoVenda, FormaPagamento};
use mikella_shared_kernel::error::{DomainError, DomainResult};
use mikella_shared_kernel::ids::{LivroCaixaId, MovimentacaoEstoqueId, UsuarioId, VendaId};
use mikella_shared_kernel::money::Money;

pub struct CancelarVendaRepos<'a> {
    pub vendas: &'a dyn VendaRepository,
    pub produtos: &'a dyn ProdutoRepository,
    pub estoque: &'a dyn MovimentacaoEstoqueRepository,
    pub livro: &'a dyn LivroCaixaRepository,
    pub hash: &'a dyn HashChain,
    pub contas_receber: Option<&'a dyn ContaReceberRepository>,
}

pub struct CancelarVendaCmd {
    pub venda_id: VendaId,
    pub usuario_id: UsuarioId,
    /// Quem autorizou o estorno (SEC-004). Nunca `None` no fluxo HTTP novo.
    pub aprovador_id: Option<UsuarioId>,
    pub motivo: Option<String>,
    /// Texto livre do operador, impresso no comprovante.
    pub observacao: Option<String>,
}

pub fn cancelar_venda(repos: &CancelarVendaRepos<'_>, cmd: CancelarVendaCmd) -> DomainResult<()> {
    let venda =
        repos
            .vendas
            .buscar_por_id(cmd.venda_id)?
            .ok_or_else(|| DomainError::NaoEncontrado {
                entidade: "Venda",
                id: cmd.venda_id.to_string(),
            })?;

    if venda.estado != EstadoVenda::Finalizada {
        return Err(DomainError::EstadoInvalido(format!(
            "Venda não pode ser cancelada pois seu estado atual é {}",
            venda.estado.as_str()
        )));
    }

    repos
        .vendas
        .atualizar_estado(cmd.venda_id, EstadoVenda::Cancelada)?;

    let agora = Utc::now();

    // 1. Devolve o estoque de cada item.
    for item in &venda.itens {
        let custo = repos
            .produtos
            .buscar_por_id(item.produto_id)?
            .map(|p| p.custo_centavos)
            .unwrap_or(Money::ZERO);
        let mov = MovimentacaoEstoque::entrada(
            MovimentacaoEstoqueId::new(),
            item.produto_id,
            item.lote_id,
            item.quantidade,
            custo,
            OrigemMovimentacao::EntradaDevolucao,
            cmd.usuario_id,
            Some(format!("Cancelamento da venda {}", cmd.venda_id)),
            agora,
        )?;
        repos.estoque.registrar(&mov)?;
    }

    // 2. Crediário: cancela parcelas ainda em aberto.
    if let Some(receber) = repos.contas_receber {
        let canceladas = receber.cancelar_abertas_por_referencia(&cmd.venda_id.to_string())?;
        if canceladas > 0 {
            tracing::info!(
                venda_id = %cmd.venda_id,
                parcelas = canceladas,
                "crediário da venda cancelado"
            );
        }
    }

    // 3. Livro-caixa: marcador + estorno por forma de pagamento.
    let posicao = repos.livro.posicao_atual(venda.sessao_id)?;
    let mut proxima_sequencia = posicao.proxima_sequencia;
    let mut ultimo_hash = posicao.ultimo_hash;
    let motivo = cmd
        .motivo
        .clone()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| "Cancelamento de venda".to_string());
    let motivo: String = match cmd
        .observacao
        .as_deref()
        .map(str::trim)
        .filter(|o| !o.is_empty())
    {
        Some(obs) => format!("{} — {}", motivo.trim(), obs),
        None => motivo.trim().to_string(),
    };
    // Limite defensivo: o motivo (120) + observação (240) cabem no cupom e no
    // livro sem truncar a observação quando o motivo é "Outro".
    let motivo: String = motivo.chars().take(360).collect();

    inserir_estorno(
        repos,
        &venda,
        TipoMovimentacao::Cancelamento,
        Money::ZERO,
        &motivo,
        cmd.usuario_id,
        cmd.aprovador_id,
        &mut proxima_sequencia,
        &mut ultimo_hash,
        agora,
    )?;

    let mut dinheiro_estornado: Option<(Money, Money)> = None;
    for pagamento in &venda.pagamentos {
        match pagamento.forma {
            FormaPagamento::Crediario => continue,
            FormaPagamento::Dinheiro => {
                let recebido = pagamento.recebido.unwrap_or(pagamento.valor);
                let troco = pagamento.troco();
                let acumulado = dinheiro_estornado.unwrap_or((Money::ZERO, Money::ZERO));
                dinheiro_estornado = Some((
                    acumulado.0.somar(recebido).map_err(|_| {
                        DomainError::InvarianteViolada("overflow no estorno".into())
                    })?,
                    acumulado.1.somar(troco).map_err(|_| {
                        DomainError::InvarianteViolada("overflow no estorno".into())
                    })?,
                ));
            }
            FormaPagamento::Debito => {
                inserir_estorno(
                    repos,
                    &venda,
                    TipoMovimentacao::VendaDebito,
                    negativo(pagamento.valor)?,
                    &motivo,
                    cmd.usuario_id,
                    cmd.aprovador_id,
                    &mut proxima_sequencia,
                    &mut ultimo_hash,
                    agora,
                )?;
            }
            FormaPagamento::Credito => {
                inserir_estorno(
                    repos,
                    &venda,
                    TipoMovimentacao::VendaCredito,
                    negativo(pagamento.valor)?,
                    &motivo,
                    cmd.usuario_id,
                    cmd.aprovador_id,
                    &mut proxima_sequencia,
                    &mut ultimo_hash,
                    agora,
                )?;
            }
            FormaPagamento::Pix => {
                inserir_estorno(
                    repos,
                    &venda,
                    TipoMovimentacao::VendaPix,
                    negativo(pagamento.valor)?,
                    &motivo,
                    cmd.usuario_id,
                    cmd.aprovador_id,
                    &mut proxima_sequencia,
                    &mut ultimo_hash,
                    agora,
                )?;
            }
            FormaPagamento::Voucher | FormaPagamento::Outro | FormaPagamento::Misto => {
                inserir_estorno(
                    repos,
                    &venda,
                    TipoMovimentacao::VendaOutro,
                    negativo(pagamento.valor)?,
                    &motivo,
                    cmd.usuario_id,
                    cmd.aprovador_id,
                    &mut proxima_sequencia,
                    &mut ultimo_hash,
                    agora,
                )?;
            }
        }
    }

    if let Some((recebido, troco)) = dinheiro_estornado {
        let liquido = recebido.subtrair(troco).map_err(|_| {
            DomainError::InvarianteViolada("overflow no estorno em dinheiro".into())
        })?;
        if !liquido.is_zero() {
            inserir_estorno(
                repos,
                &venda,
                TipoMovimentacao::VendaDinheiro,
                negativo(liquido)?,
                &motivo,
                cmd.usuario_id,
                cmd.aprovador_id,
                &mut proxima_sequencia,
                &mut ultimo_hash,
                agora,
            )?;
        }
    }

    Ok(())
}

fn negativo(valor: Money) -> DomainResult<Money> {
    Money::ZERO
        .subtrair(valor)
        .map_err(|_| DomainError::InvarianteViolada("overflow ao negar valor".into()))
}

#[allow(clippy::too_many_arguments)]
fn inserir_estorno(
    repos: &CancelarVendaRepos<'_>,
    venda: &mikella_domain_sales::venda::Venda,
    tipo: TipoMovimentacao,
    valor_centavos: Money,
    motivo: &str,
    usuario_id: UsuarioId,
    aprovador_id: Option<UsuarioId>,
    proxima_sequencia: &mut i64,
    ultimo_hash: &mut String,
    agora: DateTime<Utc>,
) -> DomainResult<()> {
    let mut entrada = EntradaLivro {
        id: LivroCaixaId::new(),
        sessao_id: venda.sessao_id,
        sequencia: *proxima_sequencia,
        tipo,
        valor_centavos,
        troco_centavos: Money::ZERO,
        referencia: Some(Referencia {
            id: venda.id.to_string(),
            tipo: "VENDA".to_string(),
        }),
        usuario_id,
        aprovador_id,
        motivo: Some(motivo.to_string()),
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
