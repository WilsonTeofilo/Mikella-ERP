//! mikella-domain-finance
//! Domínio financeiro: ContaPagar, ContaReceber, Parcela — contas a pagar e receber.
//!
//! REGRA: todo dinheiro é `Money` (centavos i64). Nunca f32/f64.
//! REGRA: o domínio NÃO conhece SQLite — apenas traits (ports).

pub mod conta;
pub mod ports;

pub use conta::{ContaPagar, ContaReceber, Parcela, PlanoConta, TipoConta};
pub use ports::{ContaPagarRepository, ContaReceberRepository, ParcelaRepository};
