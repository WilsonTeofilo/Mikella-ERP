//! mikella-app-finance
//! Casos de uso financeiros: contas a pagar/receber (RF-030).
//!
//! Regras de negócio:
//! - Criar conta (pagar/receber) com parcelas
//! - Liquidar parcela
//! - Listar contas abertas por tipo

pub mod contas;

pub use contas::{
    liquidar_parcela_pagar, liquidar_parcela_receber, listar_contas, registrar_conta_pagar,
    registrar_conta_receber, ContaFinanceira, ContasRepos, LiquidarParcelaPagarCmd,
    LiquidarParcelaReceberCmd, ListaContasQuery, RegistrarContaPagarCmd, RegistrarContaReceberCmd,
};
