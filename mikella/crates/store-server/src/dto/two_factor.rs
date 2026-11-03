//! DTOs de 2FA: solicitação e verificação de código.

use serde::Deserialize;

/// Formulário para solicitação de código 2FA.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoisFARequestForm {
    pub username: String,
    pub senha: String,
}

/// Formulário para verificação de código 2FA.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoisFAVerifyForm {
    pub username: String,
    pub codigo: String,
}
