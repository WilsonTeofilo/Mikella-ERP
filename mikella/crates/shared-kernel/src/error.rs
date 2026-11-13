use std::fmt;

/// Erros de domínio base — compartilhados por todos os crates.
#[derive(Debug)]
pub enum DomainError {
    NaoEncontrado { entidade: &'static str, id: String },
    Invalido(String),
    PermissaoNegada(String),
    CotaEsgotada(String),
    IdempotenciaConflito(String),
    EstadoInvalido(String),
    InvarianteViolada(String),
    Infraestrutura(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainError::NaoEncontrado { entidade, id } => {
                write!(f, "Entidade {} não encontrada com id {}", entidade, id)
            }
            DomainError::Invalido(msg) | DomainError::EstadoInvalido(msg) => {
                write!(f, "{}", msg)
            }
            DomainError::PermissaoNegada(_) => {
                write!(f, "Permission denied")
            }
            DomainError::CotaEsgotada(_) => {
                write!(f, "Quota exhausted")
            }
            DomainError::IdempotenciaConflito(_) => {
                write!(f, "Idempotency conflict")
            }
            DomainError::InvarianteViolada(_) => {
                write!(f, "Invariante violated")
            }
            DomainError::Infraestrutura(_) => {
                write!(f, "Internal server error")
            }
        }
    }
}

impl std::error::Error for DomainError {}

/// Resultado padrão do domínio.
pub type DomainResult<T> = Result<T, DomainError>;

/// Logging helper for domain errors
#[macro_export]
macro_rules! log_domain_error {
    ($error:expr, $context:expr) => {
        tracing::error!(
            error = %$error,
            context = %$context,
            "Domain operation failed"
        );
    };
}
