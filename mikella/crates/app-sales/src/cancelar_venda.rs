//! Caso de uso: Cancelar venda finalizada (RF-013, Clean Architecture).

use chrono::Utc;
use mikella_domain_cash::livro::{EntradaLivro, Referencia, TipoMovimentacao};
use mikella_domain_cash::ports::{HashChain, LivroCaixaRepository};
use mikella_domain_sales::ports::VendaRepository;
use mikella_domain_sales::venda::EstadoVenda;
use mikella_shared_kernel::error::{DomainError, DomainResult};
use mikella_shared_kernel::ids::{LivroCaixaId, UsuarioId, VendaId};
use mikella_shared_kernel::money::Money;

pub struct CancelarVendaRepos<'a> {
    pub vendas: &'a dyn VendaRepository,
    pub livro: &'a dyn LivroCaixaRepository,
    pub hash: &'a dyn HashChain,
}

pub struct CancelarVendaCmd {
    pub venda_id: VendaId,
    pub usuario_id: UsuarioId,
    pub motivo: Option<String>,
}

pub fn cancelar_venda(repos: &CancelarVendaRepos<'_>, cmd: CancelarVendaCmd) -> DomainResult<()> {
    let venda = repos
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

    // 1. Atualiza estado da venda no repositório de vendas
    repos.vendas.atualizar_estado(cmd.venda_id, EstadoVenda::Cancelada)?;

    // 2. Lança a movimentação de estorno/cancelamento no livro-caixa
    let posicao = repos.livro.posicao_atual(venda.sessao_id)?;
    let agora = Utc::now();
    let mut entrada = EntradaLivro {
        id: LivroCaixaId::new(),
        sessao_id: venda.sessao_id,
        sequencia: posicao.proxima_sequencia,
        tipo: TipoMovimentacao::Cancelamento,
        valor_centavos: -venda.total,
        troco_centavos: Money::ZERO,
        referencia: Some(Referencia {
            id: cmd.venda_id.to_string(),
            tipo: "VENDA".to_string(),
        }),
        usuario_id: cmd.usuario_id,
        aprovador_id: None,
        motivo: cmd.motivo.or_else(|| Some("Cancelamento de venda".to_string())),
        prev_hash: posicao.ultimo_hash,
        hash: String::new(),
        created_at: agora,
    };
    entrada.hash = repos.hash.hash_entrada(&entrada);
    repos.livro.inserir_entrada(&entrada)?;

    Ok(())
}
