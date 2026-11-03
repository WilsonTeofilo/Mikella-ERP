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
}
