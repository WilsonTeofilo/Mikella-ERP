//! Portas (traits) do domínio financeiro — implementadas pela camada de infra.
//!
//! O domínio NÃO conhece SQLite, axum ou Tauri; apenas estas traits.
//!
//! As traits são síncronas: o banco é local e o servidor usa um único escritor.

use chrono::{DateTime, Utc};

use mikella_shared_kernel::{
    error::DomainResult,
    ids::{ContaPagarId, ContaReceberId, ParcelaId, UsuarioId},
};

use crate::conta::{ContaPagar, ContaReceber, Parcela, PlanoConta};

/// Repositório de contas a pagar (RF-030).
/// Implementação: `infra-sqlite`.
pub trait ContaPagarRepository {
    /// Salva uma nova conta a pagar (insert).
    fn salvar(&self, conta: &ContaPagar) -> DomainResult<()>;

    /// Atualiza parcelas de uma conta existente (update).
    fn atualizar(&self, conta: &ContaPagar) -> DomainResult<()>;

    /// Busca uma conta a pagar pelo ID.
    fn buscar_por_id(&self, id: ContaPagarId) -> DomainResult<Option<ContaPagar>>;

    /// Lista todas as contas a pagar abertas (não totalmente pagas).
    fn buscar_abertas(&self) -> DomainResult<Vec<ContaPagar>>;

    /// Lista planos de contas distintos.
    fn listar_planos(&self) -> DomainResult<Vec<PlanoConta>>;

    /// Cancela parcelas ainda abertas de contas originadas por um documento
    /// (por exemplo, o cancelamento de uma venda a crediário).
    /// Para contas a pagar, geralmente não usado mas mantido para consistência.
    fn cancelar_abertas_por_referencia(&self, referencia_id: &str) -> DomainResult<u32>;
}

/// Repositório de contas a receber (RF-030).
pub trait ContaReceberRepository {
    fn salvar(&self, conta: &ContaReceber) -> DomainResult<()>;
    fn atualizar(&self, conta: &ContaReceber) -> DomainResult<()>;
    fn buscar_por_id(&self, id: ContaReceberId) -> DomainResult<Option<ContaReceber>>;
    fn buscar_abertas(&self) -> DomainResult<Vec<ContaReceber>>;
    fn listar_planos(&self) -> DomainResult<Vec<PlanoConta>>;

    /// Cancela parcelas ainda abertas de contas originadas por um documento
    /// (por exemplo, o cancelamento de uma venda a crediário).
    fn cancelar_abertas_por_referencia(&self, referencia_id: &str) -> DomainResult<u32>;
}

/// Repositório de parcelas individuais (para buscas rápidas por situação).
pub trait ParcelaRepository {
    /// Busca uma parcela por ID (necessário para idempotência de pagamento).
    fn buscar_por_id(&self, id: ParcelaId) -> DomainResult<Option<(ContaPagarId, Parcela)>>;

    /// Lista parcelas vencidas e abertas até `data_limite`.
    fn buscar_vencidas_ate(
        &self,
        data_limite: DateTime<Utc>,
        usuario_id: UsuarioId,
    ) -> DomainResult<Vec<(ContaPagarId, Parcela)>>;

    /// Lista parcelas abertas pendentes.
    fn listar_abertas(
        &self,
        usuario_id: UsuarioId,
    ) -> DomainResult<Vec<(ContaPagarId, Parcela)>>;
}
