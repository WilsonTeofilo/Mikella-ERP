use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{LoteId, ProdutoId},
    money::Money,
};
use serde::{Deserialize, Serialize};

/// Um item dentro de uma venda em construção.
/// O preço oficial vem do servidor (nunca da UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemVenda {
    pub produto_id: ProdutoId,
    pub lote_id: Option<LoteId>,
    pub quantidade: u32,
    pub preco_unitario: Money, // snapshot do preço no momento da venda
    pub desconto_item: Money,  // desconto específico deste item
    pub comissao_regra_id: Option<String>,
    pub comissao_valor: Money,
}

impl ItemVenda {
    /// Cria um item validado.
    pub fn novo(
        produto_id: ProdutoId,
        lote_id: Option<LoteId>,
        quantidade: u32,
        preco_unitario: Money,
        desconto_item: Money,
    ) -> DomainResult<Self> {
        if quantidade == 0 {
            return Err(DomainError::Invalido("Quantidade não pode ser zero".into()));
        }
        preco_unitario
            .assert_nao_negativo()
            .map_err(|_| DomainError::Invalido("Preço unitário não pode ser negativo".into()))?;
        desconto_item
            .assert_nao_negativo()
            .map_err(|_| DomainError::Invalido("Desconto não pode ser negativo".into()))?;
        if desconto_item > preco_unitario {
            return Err(DomainError::Invalido(
                "Desconto não pode ser maior que o preço unitário".into(),
            ));
        }
        Ok(Self {
            produto_id,
            lote_id,
            quantidade,
            preco_unitario,
            desconto_item,
            comissao_regra_id: None,
            comissao_valor: Money::ZERO,
        })
    }

    /// Subtotal do item (após desconto).
    pub fn subtotal(&self) -> DomainResult<Money> {
        let bruto = self
            .preco_unitario
            .mul_qty(self.quantidade)
            .map_err(|_| DomainError::InvarianteViolada("Overflow no subtotal do item".into()))?;
        let desconto_total = self
            .desconto_item
            .mul_qty(self.quantidade)
            .map_err(|_| DomainError::InvarianteViolada("Overflow no desconto do item".into()))?;
        Ok(bruto - desconto_total)
    }
}
