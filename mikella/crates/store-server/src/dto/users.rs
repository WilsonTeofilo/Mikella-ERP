//! DTOs de gerenciamento de usuários.
//!
//! Regras de segurança (SEG-010, FRD-055):
//! - `papel` vem do cliente, mas quem cria (via token Bearer) determina
//!   o limite de poder via `Papel::pode_criar()`.
//! - `senha_hash` NÃO é aceito do cliente — o handler faz hashing local
//!   via `hash_senha()` (Argon2id). A senha em texto chega apenas neste DTO.
//! - `capacidades_extras` e `pin_hash` são null por padrão; administradores
//!   podem gerenciar via endpoint separado (POST /v1/usuarios/:id/capacidades).

use serde::Deserialize;

/// Formulário de criação de usuário (JSON).
///
/// A senha é recebida em texto claro APENAS para hashing local — nunca
/// é armazenada ou logada. O hash Argon2id é gerado server-side.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriarUsuarioForm {
    /// UUID v4 gerado pelo cliente antes do envio (idempotência parcial).
    pub usuario_id: String,
    pub nome: String,
    pub username: String,
    pub senha: String,
    /// Papel: "DONO", "GERENTE", "OPERADOR_CAIXA", "ESTOQUISTA", "VENDEDOR".
    pub papel: String,
    /// Obrigatório: sem e-mail não há entrega do código 2FA.
    pub email: String,
    /// Capacidades extras além do papel (apenas DONO/GERENTE pode usar).
    #[serde(default)]
    pub capacidades_extras: Vec<String>,
}

/// Formulário de atualização de usuário (JSON).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtualizarUsuarioForm {
    pub nome: String,
    #[serde(default)]
    pub email: Option<String>,
}

/// Formulário para definir/atualizar senha de usuário.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtualizarSenhaForm {
    /// Senha nova em texto claro — será hasheada antes de persistir.
    pub senha: String,
}

/// Formulário para gerenciar capacidades extras de um usuário.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapacidadesForm {
    pub capacidades: Vec<String>,
}
