//! DTOs de produtos: criação e atualização.
//!
//! `operador_id` é derivado do usuário autenticado via token Bearer — nunca
//! confiamos em dados do cliente para auditoria (SEG-010).

use serde::Deserialize;

/// Formulário para criação de produto (JSON).
///
/// O `preco_centavos` e `custo_centavos` vêm em centavos (INTEGER) —
/// nunca float. Use R$ 129,90 → 12990.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriarProdutoForm {
    /// UUID v4 gerado pelo cliente ANTES do envio (idempotência parcial).
    pub produto_id: String,
    pub nome: String,
    /// Preço de venda em centavos.
    pub preco_centavos: i64,
    /// Código de barras (opcional, único por loja).
    #[serde(default)]
    pub codigo_barras: Option<String>,
    /// SKU (opcional, único por loja).
    #[serde(default)]
    pub sku: Option<String>,
    /// UUID da categoria (opcional, FK para categorias).
    #[serde(default)]
    pub categoria_id: Option<String>,
    /// Unidade: "UN", "KG", "L", "CX", "PCT" ou valor customizado.
    #[serde(default = "default_unidade")]
    pub unidade: String,
    /// Custo médio em centavos (default: 0).
    #[serde(default)]
    pub custo_centavos: i64,
    /// Estoque mínimo em unidades (default: 0).
    #[serde(default)]
    pub estoque_minimo: i32,
    /// Produto perecível?
    #[serde(default)]
    pub perecivel: bool,
    /// Dias de antecedência para alerta de validade (default: 60).
    #[serde(default = "default_alerta_dias")]
    pub alerta_validade_dias: u32,
}

fn default_unidade() -> String {
    "UN".into()
}

fn default_alerta_dias() -> u32 {
    60
}

/// Formulário para atualização completa de produto (JSON).
///
/// O `produto_id` vem da URL — todos os campos aqui são reescritos.
/// Campos `None` para `codigo_barras` e `sku` os limpam (removem o valor).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtualizarProdutoForm {
    pub nome: String,
    pub preco_centavos: i64,
    #[serde(default)]
    pub codigo_barras: Option<String>,
    #[serde(default)]
    pub sku: Option<String>,
    #[serde(default)]
    pub categoria_id: Option<String>,
    #[serde(default = "default_unidade")]
    pub unidade: String,
    /// Custo médio em centavos.
    #[serde(default)]
    pub custo_centavos: i64,
    #[serde(default)]
    pub estoque_minimo: i32,
    #[serde(default)]
    pub perecivel: bool,
    #[serde(default = "default_alerta_dias")]
    pub alerta_validade_dias: u32,
}
