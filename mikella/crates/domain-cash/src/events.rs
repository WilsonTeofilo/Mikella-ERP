use crate::livro::TipoMovimentacao;
use chrono::{DateTime, Utc};
use mikella_shared_kernel::{
    ids::{DispositivoId, LivroCaixaId, SessaoId, UsuarioId},
    money::Money,
};
/// Eventos de domínio emitidos pelo contexto do caixa.
/// Outros contextos (financeiro, auditoria) reagem a estes eventos
/// sem acoplamento direto com o contexto do caixa.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
// ATENÇÃO: o campo discriminante se chama "evento", não "tipo",
// para não conflitar com o campo `tipo: TipoMovimentacao` em
// MovimentacaoRegistrada (serde internal-tag não permite sobreposição).
#[serde(tag = "evento", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CashDomainEvent {
    SessaoAberta {
        sessao_id: SessaoId,
        numero_caixa: u8,
        operador_id: UsuarioId,
        dispositivo_id: DispositivoId,
        saldo_inicial: Money,
        aberta_em: DateTime<Utc>,
    },
    MovimentacaoRegistrada {
        entrada_id: LivroCaixaId,
        sessao_id: SessaoId,
        /// Renomeado de `tipo` para evitar conflito com a tag interna "tipo".
        movimento_tipo: TipoMovimentacao,
        valor: Money,
        created_at: DateTime<Utc>,
    },
    SessaoFechada {
        sessao_id: SessaoId,
        saldo_esperado: Money,
        saldo_contado: Money,
        diferenca: Money,
        fechada_em: DateTime<Utc>,
    },
}
