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

/// Payload para efetuar o cancelamento de uma venda
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelarVendaPayload {
    #[serde(default)]
    pub motivo: Option<String>,
}
