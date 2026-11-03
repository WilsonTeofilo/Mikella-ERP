//! Ports (interfaces) do contexto de vendas.
//! Implementações vivem em infra-sqlite — NUNCA aqui.

use crate::venda::Venda;
use mikella_shared_kernel::{
    error::DomainResult,
    ids::{SessaoId, VendaId},
    pagination::{Page, PageParams},
};

/// Repositório de vendas — contrato que a infra deve satisfazer.
pub trait VendaRepository {
    /// Persiste uma venda nova (INSERT). Retorna erro se já existir.
    fn salvar(&self, venda: &Venda) -> DomainResult<()>;

    /// Busca venda por ID. Retorna `None` se não existir.
    fn buscar_por_id(&self, id: VendaId) -> DomainResult<Option<Venda>>;

    /// Lista vendas de uma sessão, paginado, do mais recente ao mais antigo.
    fn listar_por_sessao(
        &self,
        sessao_id: SessaoId,
        params: PageParams,
    ) -> DomainResult<Page<Venda>>;

    /// Total de vendas finalizadas em uma sessão (em centavos).
    fn total_por_sessao(&self, sessao_id: SessaoId) -> DomainResult<i64>;
}
