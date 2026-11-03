//! mikella-app-cash
//!
//! Casos de uso do caixa. Nenhuma sessão fecha sozinha: o estado só muda por
//! comando explícito do operador.

pub mod caixa;

pub use caixa::{
    abrir_caixa, fechar_caixa, registrar_sangria, AbrirCaixaCmd, CaixaRepos, FecharCaixaCmd,
};
