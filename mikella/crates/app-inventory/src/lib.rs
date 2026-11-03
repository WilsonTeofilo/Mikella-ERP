//! mikella-app-inventory
//! Casos de uso de estoque.
//!
//! Regras de negocio:
//! - Criar produto, atualizar, arquivar.
//! - Importacao CSV (PROD-010)

pub mod import_csv;
pub mod produto;

pub use produto::{
    arquivar_produto, atualizar_produto, criar_produto, CriarProdutoCmd, ProdutoRepos,
    AtualizarProdutoCmd,
};
pub use import_csv::importar_produtos_csv;
pub use import_csv::{ResultadoImportacao, ResultadoLinha};
