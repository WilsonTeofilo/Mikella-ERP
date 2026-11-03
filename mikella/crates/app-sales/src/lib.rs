//! mikella-app-sales
//!
//! Casos de uso do contexto de vendas. Orquestram domínio + portas, sem
//! conhecer SQLite, HTTP ou Tauri.

pub mod cancelar_venda;
pub mod finalizar_venda;

pub use cancelar_venda::{cancelar_venda, CancelarVendaCmd, CancelarVendaRepos};
pub use finalizar_venda::{
    finalizar_venda, CrediarioCmd, FinalizarVendaCmd, ItemCmd, PoliticaEstoque, VendaRepos,
};
