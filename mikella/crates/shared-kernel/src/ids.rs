/// IDs fortemente tipados para cada entidade — evita trocar IDs de entidades diferentes.
///
/// Usando newtype pattern: `VendaId(Uuid)` != `SessaoId(Uuid)` em tempo de compilação.
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! typed_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            pub fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            pub fn as_str(&self) -> String {
                self.0.to_string()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(s.parse()?))
            }
        }
    };
}

typed_id!(VendaId);
typed_id!(SessaoId);
typed_id!(ProdutoId);
typed_id!(LoteId);
typed_id!(UsuarioId);
typed_id!(DispositivoId);
typed_id!(CategoriaId);
typed_id!(MovimentacaoEstoqueId);
typed_id!(LivroCaixaId);
typed_id!(ContaPagarId);
typed_id!(ContaReceberId);
typed_id!(ParcelaId);
typed_id!(PessoaId);
typed_id!(AuditoriaId);
typed_id!(ComandaId);
typed_id!(ComissaoRegraId);
typed_id!(QuotaEventId);
