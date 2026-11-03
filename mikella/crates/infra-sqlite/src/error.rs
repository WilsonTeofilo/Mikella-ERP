//! Erros da camada de infraestrutura e conversão para o erro de domínio.
//!
//! A conversão é deliberadamente genérica: o domínio só precisa saber que
//! "a infraestrutura falhou", nunca que existe SQLite ou um caminho de arquivo.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum InfraError {
    #[error("SQLite: {0}")]
    Sqlite(String),

    #[error("migração {versao} falhou: {detalhe}")]
    Migracao { versao: String, detalhe: String },

    #[error(
        "checksum da migração {versao} divergente (aplicado no banco: {aplicado}; arquivo atual: {atual}). \
         Nunca edite uma migração já aplicada — crie uma nova."
    )]
    ChecksumDivergente {
        versao: String,
        aplicado: String,
        atual: String,
    },

    #[error("banco corrompido ou inconsistente: {0}")]
    Integridade(String),

    #[error("dado inválido no banco ({campo}): {detalhe}")]
    DadoInvalido {
        campo: &'static str,
        detalhe: String,
    },

    #[error("JSON inválido: {0}")]
    Json(String),

    #[error("IO: {0}")]
    Io(String),
}

impl From<rusqlite::Error> for InfraError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Sqlite(err.to_string())
    }
}

impl From<serde_json::Error> for InfraError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err.to_string())
    }
}

impl From<std::io::Error> for InfraError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

use mikella_shared_kernel::error::DomainError;

impl From<InfraError> for DomainError {
    fn from(err: InfraError) -> Self {
        DomainError::Infraestrutura(err.to_string())
    }
}

/// Resultado padrão da infraestrutura.
pub type InfraResult<T> = Result<T, InfraError>;

/// Atalho para tratar coluna `NULL` em `TEXT` como `Option<String>`.
pub(crate) fn erro_campo(campo: &'static str, detalhe: impl ToString) -> InfraError {
    InfraError::DadoInvalido {
        campo,
        detalhe: detalhe.to_string(),
    }
}
