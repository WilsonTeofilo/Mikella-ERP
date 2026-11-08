//! DTOs de vendas e produtos (request/response).

use serde::Deserialize;

/// Formulário para finalização de venda (JSON).
/// `operador_id` é derivado do usuário autenticado, nunca do cliente.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizarVendaForm {
    pub venda_id: String,
    pub sessao_id: String,
    pub dispositivo_id: String,
    pub itens: Vec<ItemForm>,
    pub pagamentos: Vec<PagamentoForm>,
    #[serde(default)]
    pub desconto_centavos: i64,
    #[serde(default = "default_politica_estoque")]
    pub politica_estoque: String,
    #[serde(default)]
    pub cliente_id: Option<String>,
    #[serde(default)]
    pub comanda_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemForm {
    pub produto_id: String,
    #[serde(default)]
    pub lote_id: Option<String>,
    pub quantidade: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PagamentoForm {
    pub forma: String,
    pub valor_centavos: i64,
    #[serde(default)]
    pub recebido_centavos: Option<i64>,
    #[serde(default)]
    pub parcelas: Option<u32>,
    #[serde(default)]
    pub maquininha: Option<String>,
    #[serde(default)]
    pub modalidade: Option<String>,
    #[serde(default)]
    pub taxa_centavos: Option<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimularParcelasQuery {
    pub total_centavos: i64,
    #[serde(default = "default_max_parcelas")]
    pub max_parcelas: u32,
}

fn default_max_parcelas() -> u32 {
    12
}

#[derive(serde::Serialize)]
pub struct PlanoParcelaDto {
    pub parcelas: u32,
    pub valor_parcela_centavos: i64,
    pub ultima_parcela_centavos: i64,
    pub total_centavos: i64,
    pub texto: String,
}

#[derive(serde::Serialize)]
pub struct SimularParcelasResponse {
    pub total_centavos: i64,
    pub planos: Vec<PlanoParcelaDto>,
}

fn default_politica_estoque() -> String {
    "Bloquear".into()
}

/// Query params para `GET /v1/livro-caixa?sessao_id=<uuid>`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LivroCaixaQuery {
    pub sessao_id: String,
}

/// Query params para `GET /v1/vendas`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VendasQuery {
    /// Limite de resultados (clamp 1..200, default 50).
    #[serde(default = "default_limite_vendas")]
    pub limite: u32,
    #[serde(default)]
    pub data_inicio: Option<String>,
    #[serde(default)]
    pub data_fim: Option<String>,
    #[serde(default)]
    pub codigo: Option<String>,
}

pub fn default_limite_vendas() -> u32 {
    50
}

/// Payload para autorizar cancelamento de compra com login e senha
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutorizarCancelamentoPayload {
    pub username: String,
    pub senha: String,
    pub sessao_id: String,
}

/// Payload para efetuar o cancelamento/estorno de uma venda.
///
/// `aprovador_username`/`aprovador_senha` são obrigatórios quando quem opera
/// não é gerente/dono (SEC-004). O servidor nunca aceita autoaprovação.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelarVendaPayload {
    #[serde(default)]
    pub motivo: Option<String>,
    #[serde(default)]
    pub observacao: Option<String>,
    #[serde(default)]
    pub aprovador_username: Option<String>,
    #[serde(default)]
    pub aprovador_senha: Option<String>,
}

impl CancelarVendaPayload {
    pub fn credenciais_aprovador(&self) -> Option<crate::util::CredenciaisAprovador> {
        credenciais(
            self.aprovador_username.as_deref(),
            self.aprovador_senha.as_deref(),
        )
    }
}

/// Monta credenciais apenas quando usuário e senha vieram preenchidos.
pub(crate) fn credenciais(
    username: Option<&str>,
    senha: Option<&str>,
) -> Option<crate::util::CredenciaisAprovador> {
    match (username, senha) {
        (Some(username), Some(senha)) if !username.trim().is_empty() && !senha.is_empty() => {
            Some(crate::util::CredenciaisAprovador {
                username: username.to_string(),
                senha: senha.to_string(),
            })
        }
        _ => None,
    }
}
