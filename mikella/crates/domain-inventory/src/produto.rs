use chrono::{DateTime, Utc};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{CategoriaId, ProdutoId},
    money::Money,
};
use serde::{Deserialize, Serialize};

/// Unidade de medida do produto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unidade {
    Un,  // unidade
    Kg,  // quilograma
    L,   // litro
    Cx,  // caixa
    Pct, // pacote
    Outro(String),
}

impl Unidade {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Un => "UN",
            Self::Kg => "KG",
            Self::L => "L",
            Self::Cx => "CX",
            Self::Pct => "PCT",
            Self::Outro(valor) => valor,
        }
    }

    pub fn from_db(valor: &str) -> DomainResult<Self> {
        match valor {
            "UN" => Ok(Self::Un),
            "KG" => Ok(Self::Kg),
            "L" => Ok(Self::L),
            "CX" => Ok(Self::Cx),
            "PCT" => Ok(Self::Pct),
            "" => Err(DomainError::Invalido("Unidade vazia".into())),
            outro => Ok(Self::Outro(outro.to_string())),
        }
    }
}

/// Um produto no catálogo da loja.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Produto {
    pub id: ProdutoId,
    pub codigo_barras: Option<String>,
    pub sku: Option<String>,
    pub nome: String,
    pub categoria_id: Option<CategoriaId>,
    pub unidade: Unidade,
    pub preco_centavos: Money, // preço de venda atual
    pub custo_centavos: Money, // custo médio ponderado
    pub estoque_minimo: i32,
    pub perecivel: bool,
    pub alerta_validade_dias: u32, // default 60
    pub arquivado: bool,
    pub arquivado_em: Option<DateTime<Utc>>,
}

impl Produto {
    pub fn novo(id: ProdutoId, nome: String, preco_centavos: Money) -> DomainResult<Self> {
        if nome.trim().is_empty() {
            return Err(DomainError::Invalido(
                "Nome do produto não pode ser vazio".into(),
            ));
        }
        preco_centavos
            .assert_nao_negativo()
            .map_err(|_| DomainError::Invalido("Preço não pode ser negativo".into()))?;

        Ok(Self {
            id,
            codigo_barras: None,
            sku: None,
            nome: nome.trim().to_string(),
            categoria_id: None,
            unidade: Unidade::Un,
            preco_centavos,
            custo_centavos: Money::ZERO,
            estoque_minimo: 0,
            perecivel: false,
            alerta_validade_dias: 60,
            arquivado: false,
            arquivado_em: None,
        })
    }

    /// Arquiva o produto — NUNCA deleta fisicamente (D8).
    /// O arquivamento NÃO devolve cota mensal.
    pub fn arquivar(&mut self, agora: DateTime<Utc>) -> DomainResult<()> {
        if self.arquivado {
            return Err(DomainError::EstadoInvalido(format!(
                "Produto {:?} já está arquivado",
                self.id
            )));
        }
        self.arquivado = true;
        self.arquivado_em = Some(agora);
        Ok(())
    }

    /// Atualiza o preço. Retorna o preço anterior para auditoria.
    pub fn atualizar_preco(&mut self, novo_preco: Money) -> DomainResult<Money> {
        novo_preco
            .assert_nao_negativo()
            .map_err(|_| DomainError::Invalido("Novo preço não pode ser negativo".into()))?;
        let anterior = self.preco_centavos;
        self.preco_centavos = novo_preco;
        Ok(anterior)
    }

    pub fn esta_ativo(&self) -> bool {
        !self.arquivado
    }

    // ─── Builder methods for mutable fields ──────────────────────

    /// Define código de barras (opcional). Deve ser único por loja.
    pub fn com_codigo_barras(mut self, codigo: Option<String>) -> Self {
        self.codigo_barras = codigo;
        self
    }

    /// Define SKU (opcional). Deve ser único por loja.
    pub fn com_sku(mut self, sku: Option<String>) -> Self {
        self.sku = sku;
        self
    }

    /// Define categoria (opcional). FK para categorias.id.
    pub fn com_categoria(mut self, categoria: Option<CategoriaId>) -> Self {
        self.categoria_id = categoria;
        self
    }

    /// Define unidade de medida.
    pub fn com_unidade(mut self, unidade: Unidade) -> Self {
        self.unidade = unidade;
        self
    }

    /// Define custo médio (em centavos). Não pode ser negativo.
    pub fn com_custo(mut self, custo: Money) -> Self {
        self.custo_centavos = custo;
        self
    }

    /// Define estoque mínimo (unidades). Não pode ser negativo.
    pub fn com_estoque_minimo(mut self, minimo: i32) -> Self {
        self.estoque_minimo = minimo;
        self
    }

    /// Marca como perecível e define dias de antecedência para alerta.
    pub fn com_perecivel(mut self, perecivel: bool, alerta_dias: u32) -> Self {
        self.perecivel = perecivel;
        self.alerta_validade_dias = alerta_dias;
        self
    }

    /// Atualiza o preço de venda, retornando o anterior para auditoria.
    /// Reutiliza `atualizar_preco` — mantém invariante de não-negativo.
    pub fn atualizar_preco_retorna_anterior(&mut self, novo: Money) -> DomainResult<Money> {
        self.atualizar_preco(novo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produto_criado_corretamente() {
        let p = Produto::novo(
            ProdutoId::new(),
            "Whey 900g Baunilha".into(),
            Money::from_centavos(12990),
        )
        .unwrap();
        assert_eq!(p.nome, "Whey 900g Baunilha");
        assert!(!p.arquivado);
        assert!(p.esta_ativo());
    }

    #[test]
    fn nome_vazio_falha() {
        let r = Produto::novo(ProdutoId::new(), "   ".into(), Money::ZERO);
        assert!(r.is_err());
    }

    #[test]
    fn arquivar_produto() {
        let mut p = Produto::novo(ProdutoId::new(), "Teste".into(), Money::ZERO).unwrap();
        p.arquivar(Utc::now()).unwrap();
        assert!(p.arquivado);
        assert!(!p.esta_ativo());
    }

    #[test]
    fn arquivar_ja_arquivado_falha() {
        let mut p = Produto::novo(ProdutoId::new(), "Teste".into(), Money::ZERO).unwrap();
        p.arquivar(Utc::now()).unwrap();
        let r = p.arquivar(Utc::now());
        assert!(r.is_err());
    }
}
