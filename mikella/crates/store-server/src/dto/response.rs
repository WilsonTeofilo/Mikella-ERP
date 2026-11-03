//! DTOs de resposta HTTP.

use serde::Serialize;

/// Resposta do health check.
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub versao: &'static str,
    pub schema: String,
    pub integridade: &'static str,
    pub hora: String,
}

/// Produto para listagem.
#[derive(Serialize)]
pub struct ProdutoResponse {
    pub id: String,
    pub codigo_barras: Option<String>,
    pub sku: Option<String>,
    pub nome: String,
    pub categoria_id: Option<String>,
    pub unidade: String,
    pub preco_centavos: i64,
    pub preco_display: String,
    pub custo_centavos: i64,
    pub estoque_minimo: i32,
    pub perecivel: bool,
    pub alerta_validade_dias: u32,
    pub arquivado: bool,
    pub arquivado_em: Option<String>,
    pub estoque_atual: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// Entrada do livro do caixa para resposta HTTP.
/// DTO de leitura (read model) — não expõe campos internos do domínio.
#[derive(Serialize)]
pub struct LivroEntradaResponse {
    pub sequencia: i64,
    pub tipo: String,
    pub valor: String,
    pub troco: String,
    pub usuario_id: String,
    pub aprovador_id: Option<String>,
    pub motivo: Option<String>,
    pub referencia_id: Option<String>,
    pub referencia_tipo: Option<String>,
    pub prev_hash: String,
    pub hash: String,
    pub created_at: String,
}

/// Resumo de uma venda para listagem.
#[derive(Serialize)]
pub struct VendaSummary {
    pub id: String,
    /// COO (número sequencial da venda na base local) usado no cupom.
    pub coo: i64,
    pub sessao_id: String,
    pub operador_id: String,
    pub estado: String,
    pub total: String,
    pub total_centavos: i64,
    pub recebido: String,
    pub created_at: String,
}

// Usuário para listagem e detalhe é montado diretamente em
// `handlers::users::usuario_para_response` (JSON estável), sem struct
// intermediária duplicada.
