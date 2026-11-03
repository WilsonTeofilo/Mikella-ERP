//! DTOs de autenticação: onboarding e login.

use serde::{Deserialize, Serialize};

/// Formulário de criação do usuário dono durante o onboarding.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnboardingForm {
    pub nome: String,
    pub username: String,
    pub senha: String,
    /// Obrigatório: o 2FA é entregue por e-mail (SEG-011).
    pub email: String,
}

/// Formulário de login (form-urlencoded).
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LoginForm {
    pub username: String,
    pub senha: String,
}

/// Status de onboarding (consultado pela UI antes de decidir qual tela mostrar).
#[derive(Serialize)]
pub struct OnboardingStatus {
    pub onboarded: bool,
}
