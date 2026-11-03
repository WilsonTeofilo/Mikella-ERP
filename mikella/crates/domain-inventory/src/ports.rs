//! Ports (interfaces) do contexto de inventário.
//! Implementações vivem em infra-sqlite — NUNCA aqui.

use crate::lote::Lote;
use crate::movimentacao::MovimentacaoEstoque;
use crate::produto::Produto;
use mikella_shared_kernel::{
    error::DomainResult,
    ids::{LoteId, MovimentacaoEstoqueId, ProdutoId},
    pagination::{Page, PageParams},
};

/// Repositório de produtos.
pub trait ProdutoRepository {
    /// Persiste um produto novo.
    fn salvar(&self, produto: &Produto) -> DomainResult<()>;

    /// Atualiza produto existente (todos os campos mutáveis).
    fn atualizar(&self, produto: &Produto) -> DomainResult<()>;

    /// Busca produto por ID.
    fn buscar_por_id(&self, id: ProdutoId) -> DomainResult<Option<Produto>>;

    /// Busca produto por código de barras.
    fn buscar_por_codigo_barras(&self, codigo: &str) -> DomainResult<Option<Produto>>;

    /// Busca produto por SKU (opcional único).
    fn buscar_por_sku(&self, sku: &str) -> DomainResult<Option<Produto>>;

    /// Pesquisa full-text (FTS5) por nome/SKU. Retorna paginado.
    fn pesquisar(&self, query: &str, params: PageParams) -> DomainResult<Page<Produto>>;

    /// Lista todos os produtos não-arquivados, paginado.
    fn listar_ativos(&self, params: PageParams) -> DomainResult<Page<Produto>>;

    /// Retorna o estoque atual calculado (SUM de movimentações).
    /// Nunca lê de coluna armazenada — usa a view `vw_estoque_atual`.
    fn estoque_atual(&self, produto_id: ProdutoId) -> DomainResult<i32>;
}

/// Repositório de lotes.
pub trait LoteRepository {
    /// Persiste um lote novo.
    fn salvar(&self, lote: &Lote) -> DomainResult<()>;

    /// Atualiza lote (ex: marcar inativo).
    fn atualizar(&self, lote: &Lote) -> DomainResult<()>;

    /// Busca lote por ID.
    fn buscar_por_id(&self, id: LoteId) -> DomainResult<Option<Lote>>;

    /// Lista lotes ativos de um produto, ordenados por validade ASC (FEFO).
    fn listar_por_produto_fefo(&self, produto_id: ProdutoId) -> DomainResult<Vec<Lote>>;

    /// Lista lotes próximos do vencimento (próximos N dias).
    fn lotes_proximos_vencimento(&self, dias: u32) -> DomainResult<Vec<Lote>>;

    /// Saldo atual do lote, derivado das movimentações (nunca de coluna solta).
    fn estoque_atual(&self, lote_id: LoteId) -> DomainResult<i32>;
}

/// Repositório de movimentações de estoque — INSERT ONLY.
/// Update e delete são bloqueados por trigger no banco.
pub trait MovimentacaoEstoqueRepository {
    /// Registra uma nova movimentação. Idempotente por `id`.
    fn registrar(&self, mov: &MovimentacaoEstoque) -> DomainResult<()>;

    /// Lista movimentações de um produto, paginado, do mais recente ao mais antigo.
    fn listar_por_produto(
        &self,
        produto_id: ProdutoId,
        params: PageParams,
    ) -> DomainResult<Page<MovimentacaoEstoque>>;

    /// Busca movimentação por ID.
    fn buscar_por_id(&self, id: MovimentacaoEstoqueId)
        -> DomainResult<Option<MovimentacaoEstoque>>;
}
