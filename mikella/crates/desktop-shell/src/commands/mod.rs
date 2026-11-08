//! Comandos IPC do shell desktop.
//!
//! Regras:
//! - comandos são adaptadores finos; validam entrada e delegam;
//! - não expor comando que o frontend não usa;
//! - operações de janela usam a API oficial (`window.__TAURI__.window`) com
//!   permissões declaradas em `capabilities/main.json`, não comandos próprios.

pub mod sistema;
