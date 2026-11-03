//! mikella-app-people
//! Casos de uso de pessoas.
//!
//! Regras de negócio:
//! - Criar usuário: valida unicidade de username, checa permissão do criador.
//! - Apenas DONO cria GERENTE; GERENTE cria OPERADOR_CAIXA ou VENDEDOR.
//! - Atualizar: nome, email, PIN, capacidades extras, senha.
//! - Desativar: soft-delete (preserve histórico); não remove capacidades.
//! - Lista: ativos para seleção; todos para administração.

pub mod usuario;

pub use usuario::{
    atualizar_senha, gerenciar_capacidades, AtualizarUsuarioCmd, CriarUsuarioCmd,
    DesativarUsuarioCmd, ListaUsuarioQuery, UsuarioRepos,
};
