//! Eventos de domínio emitidos pelo contexto de inventário.

use crate::movimentacao::OrigemMovimentacao;
use chrono::{DateTime, Utc};
use mikella_shared_kernel::{
    ids::{LoteId, MovimentacaoEstoqueId, ProdutoId, UsuarioId},
    money::Money,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "evento", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InventoryDomainEvent {
    /// Estoque abaixo do mínimo configurado.
    EstoqueAbaixoMinimo {
        produto_id: ProdutoId,
        estoque_atual: i32,
        estoque_minimo: i32,
        verificado_em: DateTime<Utc>,
    },
    /// Lote próximo do vencimento.
    LoteProximoVencimento {
        lote_id: LoteId,
        produto_id: ProdutoId,
        dias_para_vencimento: u32,
        verificado_em: DateTime<Utc>,
    },
    /// Movimentação de estoque registrada.
    MovimentacaoRegistrada {
        movimentacao_id: MovimentacaoEstoqueId,
        produto_id: ProdutoId,
        lote_id: Option<LoteId>,
        quantidade: i32,
        custo_unitario_centavos: Money,
        origem: OrigemMovimentacao,
        registrada_em: DateTime<Utc>,
    },
    /// Produto arquivado (equivalente a "deletado" de forma segura).
    ProdutoArquivado {
        produto_id: ProdutoId,
        arquivado_por: UsuarioId,
        arquivado_em: DateTime<Utc>,
    },
    /// Produto reativado.
    ProdutoReativado {
        produto_id: ProdutoId,
        reativado_por: UsuarioId,
        reativado_em: DateTime<Utc>,
    },
}
