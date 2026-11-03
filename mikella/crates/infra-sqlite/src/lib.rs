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

pub mod db;
pub mod error;
pub mod hash;
pub mod migrate;
pub mod repos;

mod tempo;

pub use db::Database;
pub use error::{InfraError, InfraResult};
pub use hash::Blake3HashChain;
pub use repos::caixa::{LivroRepo, SessaoRepo};
pub use repos::lotes::LoteRepo;
pub use repos::movimentacoes::MovimentacaoRepo;
pub use repos::pessoas::UsuarioRepo;
pub use repos::produtos::ProdutoRepo;
pub use repos::vendas::VendaRepo;
pub use repos::financeiro::{ContaPagarRepo, ContaReceberRepo, ParcelaRepo};

/// Reexporta os traços de domínio para o chamador não precisar de dois `use`.
pub mod ports {
    pub use mikella_domain_cash::ports::{
        HashChain, LivroCaixaRepository, PosicaoLivro, SessaoRepository,
    };
    pub use mikella_domain_inventory::ports::{
        LoteRepository, MovimentacaoEstoqueRepository, ProdutoRepository,
    };
    pub use mikella_domain_people::ports::UsuarioRepository;
    pub use mikella_domain_sales::ports::VendaRepository;
    pub use mikella_domain_finance::ports::{
        ContaPagarRepository, ContaReceberRepository, ParcelaRepository,
    };
}
