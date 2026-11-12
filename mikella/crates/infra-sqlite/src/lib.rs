//! mikella-infra-sqlite
//!
//! Implementação SQLite das portas do domínio. Nenhum tipo deste crate vaza
//! para o domínio: os casos de uso recebem `&dyn Trait` e os testes podem
//! trocar por dublês em memória.
//!
//! ## Como usar
//!
//! ```no_run
//! use mikella_infra_sqlite::{Database, SessaoRepo, LivroRepo};
//! use mikella_domain_cash::ports::SessaoRepository;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut db = Database::abrir("dados/loja.db")?;
//! let tx = db.transacao()?;
//! let sessoes = SessaoRepo::novo(&tx);
//! let abertas = sessoes.buscar_sessoes_abertas()?;
//! tx.commit()?;
//! # Ok(())
//! # }
//! ```

pub mod backup;
pub mod db;
pub mod error;
pub mod hash;
pub mod lgpd;
pub mod migrate;
pub mod repos;

mod tempo;

pub use backup::{
    blake3_arquivo, restaurar_backup, verificar_backup, RestauracaoBackup, VerificacaoBackup,
};
pub use db::Database;
pub use error::{InfraError, InfraResult};
pub use hash::{blind_index_documento, Blake3HashChain};
pub use lgpd::{expurgar_clientes_por_retencao, RelatorioExpurgoLgpd};
pub use repos::auditoria::AuditoriaRepo;
pub use repos::caixa::{LivroEntradaView, LivroRepo, SessaoRepo};
pub use repos::config_loja::{
    ConfigLojaRepo, ConfigPagamento, MAX_PARCELAS_CARTAO_LIMITE, MAX_PARCELAS_CARTAO_PADRAO,
    PRAZO_CREDIARIO_DIAS_LIMITE, PRAZO_CREDIARIO_DIAS_PADRAO,
};
pub use repos::doacao::DoacaoRepo;
pub use repos::financeiro::{ContaPagarRepo, ContaReceberRepo, ParcelaRepo};
pub use repos::financeiro_dashboard::{
    DiaFluxoCaixaView, EntradaDetalhadaView, FinanceiroDashboardRepo, FluxoCaixaView,
    ItemAtencaoView, MovimentoFitaView, ResultadoDREView, ResumoDashboardView,
};
pub use repos::instance::InstanceRepo;
pub use repos::lotes::LoteRepo;
pub use repos::movimentacoes::MovimentacaoRepo;
pub use repos::pessoas::{backfill_blind_index, PessoaRepo, UsuarioRepo};
pub use repos::produtos::{ProdutoRepo, ProdutoView};
pub use repos::recibos::{
    carregar_estorno, carregar_fechamento, carregar_venda, EstornoReciboView, FechamentoReciboView,
    ItemReciboView, MovimentoReciboView, PagamentoReciboView, ReciboEstorno, VendaReciboView,
};
pub use repos::sistema::{ContagensDiagnostico, DiagnosticoRepo, MigracaoAplicada};
pub use repos::vendas::{FiltroVendas, VendaRepo, VendaResumo};

/// Reexporta os traços de domínio para o chamador não precisar de dois `use`.
pub mod ports {
    pub use mikella_domain_cash::ports::{
        HashChain, LivroCaixaRepository, PosicaoLivro, SessaoRepository,
    };
    pub use mikella_domain_finance::ports::{
        ContaPagarRepository, ContaReceberRepository, ParcelaRepository,
    };
    pub use mikella_domain_inventory::ports::{
        LoteRepository, MovimentacaoEstoqueRepository, ProdutoRepository,
    };
    pub use mikella_domain_people::ports::{PessoaRepository, UsuarioRepository};
    pub use mikella_domain_sales::ports::VendaRepository;
}
