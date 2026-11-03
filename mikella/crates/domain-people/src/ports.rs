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

/// DTO de cliente com resumo financeiro para consultas operacionais.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClienteResumo {
    pub pessoa: crate::Pessoa,
    pub saldo_devedor_centavos: i64,
    pub tem_vencido: bool,
}

/// DTO de parcela pendente no crediário.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParcelaDevedor {
    pub parcela_id: String,
    pub numero: u32,
    pub valor_centavos: i64,
    pub vencimento: String,
    pub dias_atraso: i64,
}

/// DTO de devedor listado em ordem alfabética.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DevedorItem {
    pub cliente_id: String,
    pub nome: String,
    pub documento: Option<String>,
    pub telefone: Option<String>,
    pub total_devido_centavos: i64,
    pub parcelas: Vec<ParcelaDevedor>,
}

/// Repositório de pessoas (clientes, fornecedores, transportadoras).
pub trait PessoaRepository {
    /// Salva uma nova pessoa no banco.
    fn salvar(&self, pessoa: &crate::Pessoa) -> DomainResult<()>;

    /// Atualiza dados de uma pessoa existente.
    fn atualizar(&self, pessoa: &crate::Pessoa) -> DomainResult<()>;

    /// Busca pessoa por ID.
    fn buscar_por_id(&self, id: &str) -> DomainResult<Option<crate::Pessoa>>;

    /// Busca clientes por termo (nome, telefone ou documento) com saldo devedor calculado.
    fn buscar_clientes_com_saldo(&self, busca: Option<&str>) -> DomainResult<Vec<ClienteResumo>>;

    /// Lista todos os devedores em ordem alfabética com parcelas pendentes.
    fn listar_devedores(&self) -> DomainResult<Vec<DevedorItem>>;

    /// Liquida uma parcela de crediário.
    fn liquidar_parcela(&self, parcela_id: &str, meio_pagamento: &str) -> DomainResult<i64>;
}
