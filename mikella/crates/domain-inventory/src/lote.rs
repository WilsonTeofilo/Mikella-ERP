use chrono::{DateTime, NaiveDate, Utc};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{LoteId, ProdutoId},
    money::Money,
};
use serde::{Deserialize, Serialize};

/// Um lote de produto com data de validade.
/// Cada lote tem sua própria quantidade (derivada de movimentações).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lote {
    pub id: LoteId,
    pub produto_id: ProdutoId,
    pub numero_lote: Option<String>,
    pub validade: Option<NaiveDate>, // None = sem validade
    pub custo_centavos: Money,
    pub ativo: bool,
    pub created_at: DateTime<Utc>,
}

impl Lote {
    pub fn novo(
        id: LoteId,
        produto_id: ProdutoId,
        numero_lote: Option<String>,
        validade: Option<NaiveDate>,
        custo_centavos: Money,
    ) -> DomainResult<Self> {
        custo_centavos
            .assert_nao_negativo()
            .map_err(|_| DomainError::Invalido("Custo do lote não pode ser negativo".into()))?;

        Ok(Self {
            id,
            produto_id,
            numero_lote,
            validade,
            custo_centavos,
            ativo: true,
            created_at: Utc::now(),
        })
    }

    /// Verifica se o lote vence dentro de N dias a partir de `hoje`.
    /// Usado para gerar alertas de validade (RF-023).
    pub fn vence_em_dias(&self, hoje: NaiveDate, dias: u32) -> bool {
        match self.validade {
            None => false,
            Some(val) => {
                let dias_restantes = (val - hoje).num_days();
                dias_restantes >= 0 && dias_restantes <= dias as i64
            }
        }
    }

    pub fn esta_vencido(&self, hoje: NaiveDate) -> bool {
        match self.validade {
            None => false,
            Some(val) => val < hoje,
        }
    }

    /// Marcos de alerta (em dias antes do vencimento) que geram notificações.
    pub const MARCOS_ALERTA: [u32; 4] = [60, 30, 7, 0]; // 0 = vencido
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn lote_vence_em_dias() {
        let hoje = NaiveDate::from_ymd_opt(2026, 10, 29).unwrap();
        let validade = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap(); // 7 dias
        let lote = Lote::novo(
            LoteId::new(),
            ProdutoId::new(),
            None,
            Some(validade),
            Money::ZERO,
        )
        .unwrap();
        assert!(lote.vence_em_dias(hoje, 7));
        assert!(lote.vence_em_dias(hoje, 30));
        assert!(!lote.esta_vencido(hoje));
    }

    #[test]
    fn lote_vencido() {
        let hoje = NaiveDate::from_ymd_opt(2026, 10, 29).unwrap();
        let validade = NaiveDate::from_ymd_opt(2026, 10, 28).unwrap(); // ontem
        let lote = Lote::novo(
            LoteId::new(),
            ProdutoId::new(),
            None,
            Some(validade),
            Money::ZERO,
        )
        .unwrap();
        assert!(lote.esta_vencido(hoje));
    }
}
