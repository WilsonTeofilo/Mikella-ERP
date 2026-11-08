//! Módulo utilitário — funções auxiliares compartilhadas entre handlers.

pub mod autorizacao;
pub mod http;
pub mod senha;
pub mod usuario;

pub use autorizacao::{exigir_aprovador, CredenciaisAprovador};
pub use http::{bad_request, error_para_http};
pub use senha::{
    gerar_codigo_2fa, hash_codigo_2fa, hash_senha, hash_senha_ficticio, senha_aceitavel,
    verificar_codigo_2fa, verificar_senha, DOISFA_TTL, TOKEN_PREFIXO,
};
pub use usuario::garantir_dispositivo;
