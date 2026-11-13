//! Handlers de operações de sistema, divididos por responsabilidade.
//!
//! Cada submódulo cobre um domínio pequeno o bastante para ser lido de uma vez:
//! backup e restauração assistida, expurgo LGPD, diagnóstico/pacote de suporte,
//! consulta de CEP e cupom de teste da impressora.

mod backup;
mod cep;
mod diagnostico;
mod impressora;
mod lgpd;

pub use backup::{backup_local, listar_backups_handler, verificar_backup_handler};
pub use cep::consultar_cep;
pub use diagnostico::{diagnostico, pacote_suporte_handler};
pub use impressora::teste_impressora_handler;
pub use lgpd::expurgo_lgpd_handler;
