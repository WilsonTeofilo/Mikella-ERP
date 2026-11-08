//! Domínio de contas a pagar e receber (RF-030).
//!
//! ## Modelo
//!
//! - **ContaPagar** / **ContaReceber** — lançamentos individuais.
//! - **Parcela** — fração do total, com vencimento e status.
//! - **TipoConta** — enum: `Pagamento`, `Recebimento`.
//! - **PlanoConta** — categoria contábil (ex: "Fornecedores", "Vendas").
//!
//! ## Regras (RF-030 §3)
//!
//! - Valor total = Σ valores das parcelas.
//! - Parcelamento: 1..=12 parcelas.
//! - Vencimento em dia útil (ajuste automático).
//! - Uma parcela só pode ser paga uma vez (idempotência).

use chrono::{DateTime, Datelike, Utc, Weekday};
use serde::{Deserialize, Serialize};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{ContaPagarId, ContaReceberId, ParcelaId, UsuarioId},
    money::Money,
};

/// Soma valores com aritmética verificada — overflow vira erro de domínio.
fn total(mut parcelas: impl Iterator<Item = Money>) -> DomainResult<Money> {
    parcelas
        .try_fold(Money::ZERO, |acc, valor| acc.somar(valor))
        .map_err(|e| DomainError::InvarianteViolada(e.to_string()))
}

/// Natureza da conta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoConta {
    #[serde(alias = "PAGAMENTO")]
    Pagamento,
    #[serde(alias = "RECEBIMENTO")]
    Recebimento,
}

impl TipoConta {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pagamento => "PAGAMENTO",
            Self::Recebimento => "RECEBIMENTO",
        }
    }

    pub fn from_db(valor: &str) -> DomainResult<Self> {
        match valor {
            "PAGAMENTO" => Ok(Self::Pagamento),
            "RECEBIMENTO" => Ok(Self::Recebimento),
            outro => Err(DomainError::Invalido(
                format!("TipoConta desconhecido: {}", outro),
            )),
        }
    }
}

/// Categoria contábil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanoConta {
    pub codigo: String,
    pub nome: String,
}

impl PlanoConta {
    pub fn novo(codigo: String, nome: String) -> DomainResult<Self> {
        let c = codigo.trim();
        let n = nome.trim();
        if c.is_empty() {
            return Err(DomainError::Invalido(
                "código do plano de contas não pode ser vazio".into(),
            ));
        }
        if n.is_empty() {
            return Err(DomainError::Invalido(
                "nome do plano de contas não pode ser vazio".into(),
            ));
        }
        Ok(Self {
            codigo: c.to_string(),
            nome: n.to_string(),
        })
    }
}

/// Status de uma parcela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusParcela {
    Aberta,
    Paga,
    Cancelada,
}

impl StatusParcela {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Aberta => "ABERTA",
            Self::Paga => "PAGA",
            Self::Cancelada => "CANCELADA",
        }
    }

    pub fn from_db(valor: &str) -> DomainResult<Self> {
        match valor {
            "ABERTA" => Ok(Self::Aberta),
            "PAGA" => Ok(Self::Paga),
            "CANCELADA" => Ok(Self::Cancelada),
            outro => Err(DomainError::Invalido(
                format!("Status de parcela desconhecido: {}", outro),
            )),
        }
    }
}

/// Uma parcela de uma conta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parcela {
    pub id: ParcelaId,
    pub valor: Money,
    pub vencimento: DateTime<Utc>,
    pub pago_em: Option<DateTime<Utc>>,
    pub status: StatusParcela,
}

impl Parcela {
    /// Cria uma nova parcela.
    ///
    /// Valor zero ou negativo é erro de domínio — não é corrigido em silêncio
    /// para `ZERO`, o que mascararia um bug de cobrança.
    pub fn nova(valor: Money, vencimento: DateTime<Utc>) -> DomainResult<Self> {
        if valor.centavos() <= 0 {
            return Err(DomainError::Invalido(
                "valor da parcela deve ser maior que zero".into(),
            ));
        }
        Ok(Self {
            id: ParcelaId::new(),
            valor,
            vencimento,
            pago_em: None,
            status: StatusParcela::Aberta,
        })
    }

    /// Marca a parcela como paga.
    pub fn marcar_como_paga(&mut self, quando: DateTime<Utc>) -> DomainResult<()> {
        if self.status == StatusParcela::Paga {
            return Err(DomainError::EstadoInvalido("parcela já paga".into()));
        }
        if self.status == StatusParcela::Cancelada {
            return Err(DomainError::EstadoInvalido("parcela cancelada".into()));
        }
        self.status = StatusParcela::Paga;
        self.pago_em = Some(quando);
        Ok(())
    }

    /// Cancela a parcela (apenas se aberta).
    pub fn cancelar(&mut self) -> DomainResult<()> {
        if self.status != StatusParcela::Aberta {
            return Err(DomainError::EstadoInvalido(format!(
                "parcela no status {} — não pode cancelar", self.status.as_str()
            )));
        }
        self.status = StatusParcela::Cancelada;
        Ok(())
    }

    /// Valor efetivamente pago.
    pub fn valor_pago(&self) -> Money {
        if self.status == StatusParcela::Paga {
            self.valor
        } else {
            Money::ZERO
        }
    }

    /// Valor em aberto.
    pub fn valor_em_aberto(&self) -> Money {
        if self.status == StatusParcela::Aberta {
            self.valor
        } else {
            Money::ZERO
        }
    }
}

/// Conta a pagar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContaPagar {
    pub id: ContaPagarId,
    pub descricao: String,
    pub plano_conta: PlanoConta,
    pub parcelas: Vec<Parcela>,
    pub criada_em: DateTime<Utc>,
    pub criada_por: UsuarioId,
}

impl ContaPagar {
    pub fn nova(
        descricao: String,
        plano_conta: PlanoConta,
        usuario_id: UsuarioId,
        agora: DateTime<Utc>,
    ) -> DomainResult<Self> {
        let d = descricao.trim();
        if d.is_empty() {
            return Err(DomainError::Invalido(
                "descrição da conta não pode ser vazia".into(),
            ));
        }
        Ok(Self {
            id: ContaPagarId::new(),
            descricao: d.to_string(),
            plano_conta,
            parcelas: Vec::new(),
            criada_em: agora,
            criada_por: usuario_id,
        })
    }

    pub fn adicionar_parcela(&mut self, valor: Money, vencimento: DateTime<Utc>) -> DomainResult<()> {
        if self.parcelas.len() >= 12 {
            return Err(DomainError::EstadoInvalido(
                "máximo de 12 parcelas excedido".into(),
            ));
        }
        self.parcelas.push(Parcela::nova(valor, vencimento)?);
        Ok(())
    }

    pub fn liquidar_parcela(&mut self, indice: usize, quando: DateTime<Utc>) -> DomainResult<()> {
        let p = self.parcelas.get_mut(indice).ok_or_else(|| DomainError::NaoEncontrado {
            entidade: "Parcela",
            id: format!("índice {}", indice),
        })?;
        p.marcar_como_paga(quando)
    }

    pub fn valor_total(&self) -> DomainResult<Money> {
        total(self.parcelas.iter().map(|p| p.valor))
    }

    pub fn valor_pago(&self) -> DomainResult<Money> {
        total(self.parcelas.iter().map(|p| p.valor_pago()))
    }

    pub fn valor_em_aberto(&self) -> DomainResult<Money> {
        total(self.parcelas.iter().map(|p| p.valor_em_aberto()))
    }

    pub fn quitada(&self) -> bool {
        !self.parcelas.is_empty()
            && self.parcelas.iter().all(|p| p.status == StatusParcela::Paga)
    }

    pub fn status(&self) -> &'static str {
        if self.parcelas.is_empty() {
            "SEM_PARCELAS"
        } else if self.quitada() {
            "QUITADA"
        } else if self.parcelas.iter().all(|p| p.status == StatusParcela::Cancelada) {
            "CANCELADA"
        } else {
            "ABERTA"
        }
    }
}

/// Conta a receber.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContaReceber {
    pub id: ContaReceberId,
    pub descricao: String,
    pub plano_conta: PlanoConta,
    pub parcelas: Vec<Parcela>,
    pub criada_em: DateTime<Utc>,
    pub criada_por: UsuarioId,
    /// Cliente devedor (crediário). `None` para recebimentos sem pessoa.
    pub pessoa_id: Option<String>,
    /// Documento de origem (por exemplo, a venda que gerou o crediário).
    pub referencia_id: Option<String>,
    pub referencia_tipo: Option<String>,
}

impl ContaReceber {
    pub fn nova(
        descricao: String,
        plano_conta: PlanoConta,
        usuario_id: UsuarioId,
        agora: DateTime<Utc>,
    ) -> DomainResult<Self> {
        let d = descricao.trim();
        if d.is_empty() {
            return Err(DomainError::Invalido(
                "descrição da conta não pode ser vazia".into(),
            ));
        }
        Ok(Self {
            id: ContaReceberId::new(),
            descricao: d.to_string(),
            plano_conta,
            parcelas: Vec::new(),
            criada_em: agora,
            criada_por: usuario_id,
            pessoa_id: None,
            referencia_id: None,
            referencia_tipo: None,
        })
    }

    /// Vincula a conta ao cliente e ao documento que a originou.
    pub fn vincular_origem(
        &mut self,
        pessoa_id: String,
        referencia_id: String,
        referencia_tipo: String,
    ) -> DomainResult<()> {
        if pessoa_id.trim().is_empty() {
            return Err(DomainError::Invalido(
                "conta a receber de crediário exige um cliente".into(),
            ));
        }
        self.pessoa_id = Some(pessoa_id);
        self.referencia_id = Some(referencia_id);
        self.referencia_tipo = Some(referencia_tipo);
        Ok(())
    }

    pub fn adicionar_parcela(&mut self, valor: Money, vencimento: DateTime<Utc>) -> DomainResult<()> {
        if self.parcelas.len() >= 12 {
            return Err(DomainError::EstadoInvalido(
                "máximo de 12 parcelas excedido".into(),
            ));
        }
        self.parcelas.push(Parcela::nova(valor, vencimento)?);
        Ok(())
    }

    pub fn liquidar_parcela(&mut self, indice: usize, quando: DateTime<Utc>) -> DomainResult<()> {
        let p = self.parcelas.get_mut(indice).ok_or_else(|| DomainError::NaoEncontrado {
            entidade: "Parcela",
            id: format!("índice {}", indice),
        })?;
        p.marcar_como_paga(quando)
    }

    pub fn valor_total(&self) -> DomainResult<Money> {
        total(self.parcelas.iter().map(|p| p.valor))
    }

    pub fn valor_pago(&self) -> DomainResult<Money> {
        total(self.parcelas.iter().map(|p| p.valor_pago()))
    }

    pub fn valor_em_aberto(&self) -> DomainResult<Money> {
        total(self.parcelas.iter().map(|p| p.valor_em_aberto()))
    }

    pub fn quitada(&self) -> bool {
        !self.parcelas.is_empty()
            && self.parcelas.iter().all(|p| p.status == StatusParcela::Paga)
    }

    pub fn status(&self) -> &'static str {
        if self.parcelas.is_empty() {
            "SEM_PARCELAS"
        } else if self.quitada() {
            "QUITADA"
        } else if self.parcelas.iter().all(|p| p.status == StatusParcela::Cancelada) {
            "CANCELADA"
        } else {
            "ABERTA"
        }
    }
}

/// Ajusta vencimento para dia útil (sábado → sexta, domingo → segunda).
///
/// Usa aritmética verificada: uma data no limite do calendário não deve
/// derrubar o processo com panic.
pub fn ajustar_para_dia_util(dt: DateTime<Utc>) -> DateTime<Utc> {
    match dt.weekday() {
        Weekday::Sat => dt
            .checked_sub_signed(chrono::Duration::days(1))
            .unwrap_or(dt),
        Weekday::Sun => dt
            .checked_add_signed(chrono::Duration::days(1))
            .unwrap_or(dt),
        _ => dt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn hoje() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap()
    }

    #[test]
    fn conta_pagar_cria_e_soma_parcelas() {
        let plano = PlanoConta::novo("3.1.01".into(), "Fornecedores".into()).unwrap();
        let mut c = ContaPagar::nova("Compra".into(), plano, UsuarioId::new(), hoje()).unwrap();
        c.adicionar_parcela(Money::from_centavos(10000), hoje()).unwrap();
        c.adicionar_parcela(Money::from_centavos(5000), hoje()).unwrap();
        assert_eq!(c.valor_total().unwrap().centavos(), 15000);
        assert_eq!(c.valor_em_aberto().unwrap().centavos(), 15000);
        assert!(!c.quitada());
        assert_eq!(c.status(), "ABERTA");
    }

    #[test]
    fn conta_pagar_liquida_parcela() {
        let plano = PlanoConta::novo("3.1.01".into(), "Fornecedores".into()).unwrap();
        let mut c = ContaPagar::nova("Conta".into(), plano, UsuarioId::new(), hoje()).unwrap();
        c.adicionar_parcela(Money::from_centavos(10000), hoje()).unwrap();
        c.adicionar_parcela(Money::from_centavos(5000), hoje()).unwrap();
        c.liquidar_parcela(0, hoje()).unwrap();
        assert_eq!(c.valor_pago().unwrap().centavos(), 10000);
        assert_eq!(c.valor_em_aberto().unwrap().centavos(), 5000);
        assert!(!c.quitada());
    }

    #[test]
    fn conta_pagar_quitada() {
        let plano = PlanoConta::novo("3.1.01".into(), "Fornecedores".into()).unwrap();
        let mut c = ContaPagar::nova("Conta".into(), plano, UsuarioId::new(), hoje()).unwrap();
        c.adicionar_parcela(Money::from_centavos(10000), hoje()).unwrap();
        c.liquidar_parcela(0, hoje()).unwrap();
        assert!(c.quitada());
        assert_eq!(c.status(), "QUITADA");
    }

    #[test]
    fn parcela_duplicada_erro() {
        let mut p = Parcela::nova(Money::from_centavos(100), hoje()).unwrap();
        p.marcar_como_paga(hoje()).unwrap();
        assert!(p.marcar_como_paga(hoje()).is_err());
    }

    #[test]
    fn maximo_12_parcelas() {
        let plano = PlanoConta::novo("3.1.01".into(), "Fornecedores".into()).unwrap();
        let mut c = ContaPagar::nova("Teste".into(), plano, UsuarioId::new(), hoje()).unwrap();
        for _ in 0..12 {
            c.adicionar_parcela(Money::from_centavos(100), hoje()).unwrap();
        }
        assert!(c.adicionar_parcela(Money::from_centavos(100), hoje()).is_err());
    }

    #[test]
    fn descricao_vazia_falha() {
        let plano = PlanoConta::novo("3.1.01".into(), "Fornecedores".into()).unwrap();
        assert!(ContaPagar::nova("  ".into(), plano, UsuarioId::new(), hoje()).is_err());
    }

    #[test]
    fn ajustar_dia_util() {
        let sat = Utc.with_ymd_and_hms(2024, 1, 13, 10, 0, 0).unwrap();
        assert_eq!(ajustar_para_dia_util(sat).weekday(), Weekday::Fri);
        let sun = Utc.with_ymd_and_hms(2024, 1, 14, 10, 0, 0).unwrap();
        assert_eq!(ajustar_para_dia_util(sun).weekday(), Weekday::Mon);
        let wed = Utc.with_ymd_and_hms(2024, 1, 17, 10, 0, 0).unwrap();
        assert_eq!(ajustar_para_dia_util(wed), wed);
    }

    #[test]
    fn plano_conta_vazio_falha() {
        assert!(PlanoConta::novo("".into(), "Nome".into()).is_err());
        assert!(PlanoConta::novo("cod".into(), "".into()).is_err());
    }

    #[test]
    fn cancelar_parcela() {
        let mut p = Parcela::nova(Money::from_centavos(5000), hoje()).unwrap();
        p.cancelar().unwrap();
        assert_eq!(p.status, StatusParcela::Cancelada);
        assert!(p.cancelar().is_err());
    }

    #[test]
    fn parcela_com_valor_zero_ou_negativo_e_rejeitada() {
        assert!(Parcela::nova(Money::ZERO, hoje()).is_err());
        assert!(Parcela::nova(Money::from_centavos(-1), hoje()).is_err());
    }

    #[test]
    fn tipo_conta_roundtrip() {
        assert_eq!(TipoConta::Pagamento.as_str(), "PAGAMENTO");
        assert!(TipoConta::from_db("PAGAMENTO").unwrap() == TipoConta::Pagamento);
    }

    #[test]
    fn status_parcela_roundtrip() {
        assert!(StatusParcela::from_db("PAGA").unwrap() == StatusParcela::Paga);
        assert!(StatusParcela::from_db("CANCELADA").unwrap() == StatusParcela::Cancelada);
    }
}
