//! DTOs de caixa: abertura e fechamento de sessão.

use serde::Deserialize;

/// Formulário para abertura de caixa.
///
/// `operador_id` **não** vem do cliente — é derivado do usuário autenticado
/// via token Bearer. Isso impede impersonation: o operador é sempre quem
/// abriu a sessão, não quem o formulário diz que é.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbrirCaixaForm {
    pub sessao_id: String,
    pub numero_caixa: u8,
    pub dispositivo_id: String,
    pub saldo_inicial_centavos: i64,
    #[serde(default)]
    pub fechamento_cego: bool,
}

/// Formulário para fechamento de caixa.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FecharCaixaForm {
    pub sessao_id: String,
    pub saldo_contado_centavos: i64,
    #[serde(default)]
    pub observacao: Option<String>,
}

/// Formulário para sangria (retirada de dinheiro do caixa).
///
/// O aprovador é obrigatório quando quem opera não é gerente/dono (SEC-006).
/// `None` é aceito para gerente/dono, que aprovam a própria operação.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SangriaForm {
    pub sessao_id: String,
    pub valor_centavos: i64,
    #[serde(default)]
    pub motivo: Option<String>,
    #[serde(default)]
    pub aprovador_username: Option<String>,
    #[serde(default)]
    pub aprovador_senha: Option<String>,
}

impl SangriaForm {
    /// Credenciais de aprovação, quando informadas.
    pub fn credenciais_aprovador(&self) -> Option<crate::util::CredenciaisAprovador> {
        match (
            self.aprovador_username.as_deref(),
            self.aprovador_senha.as_deref(),
        ) {
            (Some(username), Some(senha)) if !username.trim().is_empty() && !senha.is_empty() => {
                Some(crate::util::CredenciaisAprovador {
                    username: username.to_string(),
                    senha: senha.to_string(),
                })
            }
            _ => None,
        }
    }
}
