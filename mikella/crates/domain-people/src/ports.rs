//! Ports (traits) do contexto de pessoas.
//! Implementações vivem em infra-sqlite — NUNCA aqui.
//!
//! O domínio NUNCA conhece SQLite, axum ou HTTP; apenas estas traits.

use mikella_shared_kernel::{error::DomainResult, ids::UsuarioId};

use crate::Usuario;

/// Repositório de usuários.
pub trait UsuarioRepository {
    /// Persiste um novo usuário.
    fn salvar(&self, usuario: &Usuario) -> DomainResult<()>;

    /// Atualiza usuário existente.
    fn atualizar(&self, usuario: &Usuario) -> DomainResult<()>;

    /// Busca usuário por ID.
    fn buscar_por_id(&self, id: UsuarioId) -> DomainResult<Option<Usuario>>;

    /// Busca usuário por username (case-insensitive).
    fn buscar_por_username(&self, username: &str) -> DomainResult<Option<Usuario>>;

    /// Lista todos os usuários ativos da loja.
    fn listar_ativos(&self) -> DomainResult<Vec<Usuario>>;

    /// Lista todos os usuários (ativos e inativos).
    fn listar_todos(&self) -> DomainResult<Vec<Usuario>>;
}
