//! mikella-domain-inventory
//! Produto, Lote, Validade, movimentação de estoque.
//! Estoque é calculado por movimentações — nunca um número solto.

pub mod events;
pub mod lote;
pub mod movimentacao;
pub mod ports;
pub mod produto;
