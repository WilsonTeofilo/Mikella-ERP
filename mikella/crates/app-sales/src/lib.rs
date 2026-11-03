//! mikella-app-sales
//!
//! Casos de uso do contexto de vendas. Orquestram domínio + portas, sem
//! conhecer SQLite, HTTP ou Tauri.

pub mod finalizar_venda;

pub use finalizar_venda::{
    finalizar_venda, FinalizarVendaCmd, ItemCmd, PoliticaEstoque, VendaRepos,
};
