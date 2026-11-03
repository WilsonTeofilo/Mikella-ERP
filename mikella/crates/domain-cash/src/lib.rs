//! mikella-domain-cash
//!
//! Domínio do caixa: SessaoCaixa, livro imutável de movimentações.
//! REGRA: saldo nunca é um número solto — é sempre Σ(movimentações).
//! Nenhuma movimentação é editada ou removida; correções são compensações.

pub mod events;
pub mod livro;
pub mod ports;
pub mod sessao;
