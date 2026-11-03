//! mikella-shared-kernel
//!
//! Tipos base compartilhados por todos os crates do domínio.
//! REGRA: este crate NÃO pode importar Tauri, axum, rusqlite nem HTTP.
//! Só biblioteca padrão + crates utilitárias puras.

pub mod clock;
pub mod documento;
pub mod error;
pub mod ids;
pub mod money;
pub mod pagination;
