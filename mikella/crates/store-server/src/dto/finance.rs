//! DTOs de contas a pagar / receber (RF-030).
//!
//! SEG-060: `#[serde(deny_unknown_fields)]` em todos os DTOs.

use serde::Deserialize;

use mikella_domain_finance::TipoConta;

/// Formulário para registrar uma conta a pagar (produtor ou despesa).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrarContaPagarForm {
    pub descricao: String,
    pub plano_codigo: String,
    pub plano_nome: String,
    /// Parcelas como pares (centavos, RFC3339).
    #[serde(default)]
    pub parcelas: Vec<ParcelaForm>,
}

/// Formulário para registrar uma conta a receber (cliente ou receita).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrarContaReceberForm {
    pub descricao: String,
    pub plano_codigo: String,
    pub plano_nome: String,
    #[serde(default)]
    pub parcelas: Vec<ParcelaForm>,
}

/// Uma parcela dentro do formulário (valor em centavos + vencimento RFC3339).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParcelaForm {
    pub valor_centavos: i64,
    pub vencimento: String,
}

/// Query para listagem de contas abertas.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListarContasQuery {
    #[serde(default)]
    pub tipo: Option<TipoConta>,
}

/// Query para resumo executivo do dashboard.
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ResumoDashboardQuery {
    #[serde(default)]
    pub periodo: Option<String>,
    #[serde(default)]
    pub data_inicio: Option<String>,
    #[serde(default)]
    pub data_fim: Option<String>,
}

/// Query para fluxo de caixa por período.
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct FluxoCaixaQuery {
    #[serde(default)]
    pub inicio: Option<String>,
    #[serde(default)]
    pub fim: Option<String>,
}

/// Query para DRE simplificada.
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ResultadoDREQuery {
    #[serde(default)]
    pub mes: Option<String>,
}
