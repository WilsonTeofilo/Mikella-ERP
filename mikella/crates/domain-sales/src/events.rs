//! Eventos de domínio emitidos pelo contexto de vendas.

use crate::venda::FormaPagamento;
use chrono::{DateTime, Utc};
use mikella_shared_kernel::{
    ids::{SessaoId, UsuarioId, VendaId},
    money::Money,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "evento", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SalesDomainEvent {
    /// Venda processada com sucesso. Dispara: baixa no estoque, entrada no livro.
    VendaFinalizada {
        venda_id: VendaId,
        sessao_id: SessaoId,
        operador_id: UsuarioId,
        total_centavos: Money,
        forma_pagamento: FormaPagamento,
        finalizada_em: DateTime<Utc>,
    },
    /// Venda cancelada antes de fechar o caixa (sem troca de dinheiro ainda).
    VendaCancelada {
        venda_id: VendaId,
        sessao_id: SessaoId,
        motivo: Option<String>,
        cancelada_em: DateTime<Utc>,
    },
    /// Venda estornada após fechamento (dinheiro já tinha saído/entrado).
    /// Um estorno gera uma movimentação compensatória no livro do caixa.
    VendaEstornada {
        venda_id: VendaId,
        sessao_id: SessaoId,
        aprovador_id: UsuarioId,
        valor_devolvido: Money,
        motivo: String,
        estornada_em: DateTime<Utc>,
    },
}
