//! MovimentacaoEstoque — registro imutável de entrada ou saída de estoque.
//! Estoque atual = SUM(quantidade) de todas as movimentações de um produto/lote.
//! NUNCA update ou delete — apenas inserts. Triggers no SQLite bloqueiam qualquer alteração.

use chrono::{DateTime, Utc};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{LoteId, MovimentacaoEstoqueId, ProdutoId, UsuarioId, VendaId},
    money::Money,
};
use serde::{Deserialize, Serialize};

/// Origem da movimentação — quem causou a mudança no estoque.
///
/// Os nomes são os mesmos gravados no banco (SCREAMING_SNAKE_CASE).
/// Variantes `Entrada*` exigem quantidade positiva; `Saida*`, negativa.
/// `Inventario` é o único caso que pode ser positivo ou negativo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrigemMovimentacao {
    /// Entrada de compra / recebimento de fornecedor.
    EntradaCompra,
    /// Devolução de venda — retorno ao estoque.
    EntradaDevolucao,
    /// Ajuste manual positivo de inventário.
    EntradaAjuste,
    /// Venda finalizada no PDV.
    SaidaVenda,
    /// Perda declarada (vencimento, dano, furto).
    SaidaPerda,
    /// Ajuste manual negativo de inventário.
    SaidaAjuste,
    /// Contagem de inventário (pode ser positiva ou negativa).
    Inventario,
}

impl OrigemMovimentacao {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EntradaCompra => "ENTRADA_COMPRA",
            Self::EntradaDevolucao => "ENTRADA_DEVOLUCAO",
            Self::EntradaAjuste => "ENTRADA_AJUSTE",
            Self::SaidaVenda => "SAIDA_VENDA",
            Self::SaidaPerda => "SAIDA_PERDA",
            Self::SaidaAjuste => "SAIDA_AJUSTE",
            Self::Inventario => "INVENTARIO",
        }
    }

    pub fn from_db(valor: &str) -> DomainResult<Self> {
        match valor {
            "ENTRADA_COMPRA" => Ok(Self::EntradaCompra),
            "ENTRADA_DEVOLUCAO" => Ok(Self::EntradaDevolucao),
            "ENTRADA_AJUSTE" => Ok(Self::EntradaAjuste),
            "SAIDA_VENDA" => Ok(Self::SaidaVenda),
            "SAIDA_PERDA" => Ok(Self::SaidaPerda),
            "SAIDA_AJUSTE" => Ok(Self::SaidaAjuste),
            "INVENTARIO" => Ok(Self::Inventario),
            outro => Err(DomainError::Invalido(format!(
                "Origem de movimentação desconhecida no banco: {outro}"
            ))),
        }
    }

    /// `true` se a origem exige quantidade positiva.
    pub fn exige_quantidade_positiva(&self) -> bool {
        matches!(
            self,
            Self::EntradaCompra | Self::EntradaDevolucao | Self::EntradaAjuste
        )
    }

    /// `true` se a origem exige quantidade negativa.
    pub fn exige_quantidade_negativa(&self) -> bool {
        matches!(
            self,
            Self::SaidaVenda | Self::SaidaPerda | Self::SaidaAjuste
        )
    }
}

/// Uma movimentação de estoque — imutável após gravada.
///
/// quantidade > 0 = entrada no estoque  
/// quantidade < 0 = saída do estoque  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovimentacaoEstoque {
    pub id: MovimentacaoEstoqueId,
    pub produto_id: ProdutoId,
    pub lote_id: Option<LoteId>,
    /// Quantidade movimentada. Positivo = entrada, Negativo = saída.
    /// Para produtos vendidos: -1 por unidade vendida.
    pub quantidade: i32,
    /// Custo unitário no momento da movimentação (para CMV / custo médio).
    pub custo_unitario_centavos: Money,
    pub origem: OrigemMovimentacao,
    /// Referência ao documento de origem (VendaId, etc.).
    pub referencia_id: Option<String>,
    /// ID da venda, se a origem for Venda ou EstornoVenda.
    pub venda_id: Option<VendaId>,
    pub usuario_id: UsuarioId,
    pub observacao: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl MovimentacaoEstoque {
    /// Cria nova movimentação de saída por venda.
    /// `quantidade` deve ser POSITIVO — este método a torna negativa.
    #[allow(clippy::too_many_arguments)] // espelha as colunas do registro imutável
    pub fn saida_venda(
        id: MovimentacaoEstoqueId,
        produto_id: ProdutoId,
        lote_id: Option<LoteId>,
        quantidade: u32,
        custo_unitario_centavos: Money,
        venda_id: VendaId,
        usuario_id: UsuarioId,
        agora: DateTime<Utc>,
    ) -> DomainResult<Self> {
        if quantidade == 0 {
            return Err(DomainError::InvarianteViolada(
                "Quantidade de saída não pode ser zero".into(),
            ));
        }
        Ok(Self {
            id,
            produto_id,
            lote_id,
            quantidade: -(quantidade as i32),
            custo_unitario_centavos,
            origem: OrigemMovimentacao::SaidaVenda,
            referencia_id: Some(venda_id.to_string()),
            venda_id: Some(venda_id),
            usuario_id,
            observacao: None,
            created_at: agora,
        })
    }

    /// Cria nova movimentação de entrada (compra, ajuste positivo).
    #[allow(clippy::too_many_arguments)] // espelha as colunas do registro imutável
    pub fn entrada(
        id: MovimentacaoEstoqueId,
        produto_id: ProdutoId,
        lote_id: Option<LoteId>,
        quantidade: u32,
        custo_unitario_centavos: Money,
        origem: OrigemMovimentacao,
        usuario_id: UsuarioId,
        observacao: Option<String>,
        agora: DateTime<Utc>,
    ) -> DomainResult<Self> {
        if quantidade == 0 {
            return Err(DomainError::InvarianteViolada(
                "Quantidade de entrada não pode ser zero".into(),
            ));
        }
        if origem.exige_quantidade_negativa() {
            return Err(DomainError::Invalido(format!(
                "Origem {} exige quantidade negativa; use o construtor de saída",
                origem.as_str()
            )));
        }
        Ok(Self {
            id,
            produto_id,
            lote_id,
            quantidade: quantidade as i32,
            custo_unitario_centavos,
            origem,
            referencia_id: None,
            venda_id: None,
            usuario_id,
            observacao,
            created_at: agora,
        })
    }

    /// Retorna o valor total desta movimentação (quantidade × custo unitário).
    /// Pode ser negativo (saída).
    pub fn valor_total_centavos(&self) -> i64 {
        self.quantidade as i64 * self.custo_unitario_centavos.centavos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saida_venda_quantidade_negativa() {
        let mov = MovimentacaoEstoque::saida_venda(
            MovimentacaoEstoqueId::new(),
            ProdutoId::new(),
            None,
            3,
            Money::from_centavos(500),
            VendaId::new(),
            UsuarioId::new(),
            Utc::now(),
        )
        .unwrap();
        assert_eq!(mov.quantidade, -3);
        assert_eq!(mov.valor_total_centavos(), -1500);
    }

    #[test]
    fn saida_venda_quantidade_zero_falha() {
        let result = MovimentacaoEstoque::saida_venda(
            MovimentacaoEstoqueId::new(),
            ProdutoId::new(),
            None,
            0,
            Money::from_centavos(500),
            VendaId::new(),
            UsuarioId::new(),
            Utc::now(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn entrada_compra_quantidade_positiva() {
        let mov = MovimentacaoEstoque::entrada(
            MovimentacaoEstoqueId::new(),
            ProdutoId::new(),
            None,
            10,
            Money::from_centavos(200),
            OrigemMovimentacao::EntradaCompra,
            UsuarioId::new(),
            None,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(mov.quantidade, 10);
        assert_eq!(mov.valor_total_centavos(), 2000);
    }
}
