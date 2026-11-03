use crate::livro::TipoMovimentacao;
use chrono::{DateTime, Utc};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{DispositivoId, SessaoId, UsuarioId},
    money::Money,
};
use serde::{Deserialize, Serialize};

/// Estado da sessão de caixa (RF-013, RF-014, RES-012).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EstadoSessao {
    Aberta,
    EmFechamento,
    Fechada,
}

impl EstadoSessao {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Aberta => "ABERTA",
            Self::EmFechamento => "EM_FECHAMENTO",
            Self::Fechada => "FECHADA",
        }
    }

    pub fn from_db(valor: &str) -> DomainResult<Self> {
        match valor {
            "ABERTA" => Ok(Self::Aberta),
            "EM_FECHAMENTO" => Ok(Self::EmFechamento),
            "FECHADA" => Ok(Self::Fechada),
            outro => Err(DomainError::Invalido(format!(
                "Estado de sessão desconhecido no banco: {outro}"
            ))),
        }
    }
}

/// Uma sessão de caixa — janela de operação de um caixa num dia.
/// O saldo é derivado do livro de movimentações; nunca armazenado como fonte de verdade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessaoCaixa {
    pub id: SessaoId,
    pub numero_caixa: u8, // 1 a 8
    pub dispositivo_id: DispositivoId,
    pub operador_id: UsuarioId,
    pub estado: EstadoSessao,
    pub aberta_em: DateTime<Utc>,
    pub fechada_em: Option<DateTime<Utc>>,
    pub fechamento_cego: bool, // FRD-052: operador conta sem ver o esperado

    // Projeções (calculadas na mesma transação, reconciliáveis com o livro — RES-010)
    pub saldo_inicial_centavos: Money,
    pub saldo_esperado_centavos: Money,
    pub saldo_contado_centavos: Option<Money>, // preenchido no fechamento
}

impl SessaoCaixa {
    /// Abre uma nova sessão de caixa.
    pub fn abrir(
        id: SessaoId,
        numero_caixa: u8,
        dispositivo_id: DispositivoId,
        operador_id: UsuarioId,
        valor_inicial: Money,
        fechamento_cego: bool,
        agora: DateTime<Utc>,
    ) -> DomainResult<Self> {
        if !(1..=8).contains(&numero_caixa) {
            return Err(DomainError::Invalido(format!(
                "Número de caixa inválido: {numero_caixa}. Deve ser de 1 a 8."
            )));
        }
        valor_inicial
            .assert_nao_negativo()
            .map_err(|_| DomainError::Invalido("Valor inicial não pode ser negativo".into()))?;

        Ok(Self {
            id,
            numero_caixa,
            dispositivo_id,
            operador_id,
            estado: EstadoSessao::Aberta,
            aberta_em: agora,
            fechada_em: None,
            fechamento_cego,
            saldo_inicial_centavos: valor_inicial,
            saldo_esperado_centavos: valor_inicial,
            saldo_contado_centavos: None,
        })
    }

    /// Aplica uma movimentação à projeção do saldo esperado.
    ///
    /// Só movimentações que afetam dinheiro físico alteram a projeção
    /// (`TipoMovimentacao::afeta_saldo_fisico`). A fonte de verdade continua
    /// sendo o livro imutável; esta coluna é apenas reconciliação rápida.
    pub fn registrar_movimentacao(
        &mut self,
        tipo: &TipoMovimentacao,
        valor: Money,
        troco: Money,
    ) -> DomainResult<()> {
        self.assert_aberta()?;
        troco
            .assert_nao_negativo()
            .map_err(|_| DomainError::Invalido("Troco não pode ser negativo".into()))?;
        if !tipo.afeta_saldo_fisico() {
            return Ok(());
        }
        self.saldo_esperado_centavos = self
            .saldo_esperado_centavos
            .somar(valor)
            .and_then(|v| v.subtrair(troco))
            .map_err(|_| {
                DomainError::InvarianteViolada("Overflow ao atualizar saldo do caixa".into())
            })?;
        Ok(())
    }

    /// Inicia o processo de fechamento.
    pub fn iniciar_fechamento(&mut self) -> DomainResult<()> {
        self.assert_aberta()?;
        self.estado = EstadoSessao::EmFechamento;
        Ok(())
    }

    /// Finaliza o fechamento com o valor contado pelo operador.
    /// A diferença é calculada após o operador informar o contado.
    pub fn finalizar_fechamento(
        &mut self,
        saldo_contado: Money,
        agora: DateTime<Utc>,
    ) -> DomainResult<DiferencaCaixa> {
        if self.estado != EstadoSessao::EmFechamento {
            return Err(DomainError::EstadoInvalido(
                "Sessão não está em fechamento".into(),
            ));
        }
        self.saldo_contado_centavos = Some(saldo_contado);
        self.fechada_em = Some(agora);
        self.estado = EstadoSessao::Fechada;

        let diferenca = saldo_contado - self.saldo_esperado_centavos;
        Ok(DiferencaCaixa {
            esperado: self.saldo_esperado_centavos,
            contado: saldo_contado,
            diferenca,
        })
    }

    pub fn is_aberta(&self) -> bool {
        self.estado == EstadoSessao::Aberta
    }

    fn assert_aberta(&self) -> DomainResult<()> {
        if self.estado != EstadoSessao::Aberta {
            return Err(DomainError::EstadoInvalido(format!(
                "Sessão {:?} não está aberta (estado: {:?})",
                self.id, self.estado
            )));
        }
        Ok(())
    }
}

/// Resultado do fechamento de caixa.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiferencaCaixa {
    pub esperado: Money,
    pub contado: Money,
    pub diferenca: Money, // positivo = sobra, negativo = falta
}

impl DiferencaCaixa {
    pub fn tem_diferenca(&self) -> bool {
        !self.diferenca.is_zero()
    }

    pub fn e_sobra(&self) -> bool {
        self.diferenca.is_positive()
    }

    pub fn e_falta(&self) -> bool {
        self.diferenca.is_negative()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sessao_exemplo() -> SessaoCaixa {
        SessaoCaixa::abrir(
            SessaoId::new(),
            1,
            DispositivoId::new(),
            UsuarioId::new(),
            Money::from_centavos(50000), // R$ 500,00 inicial
            false,
            Utc::now(),
        )
        .unwrap()
    }

    #[test]
    fn abre_sessao_com_saldo_inicial() {
        let s = sessao_exemplo();
        assert_eq!(s.saldo_esperado_centavos.centavos(), 50000);
        assert!(s.is_aberta());
    }

    #[test]
    fn numero_caixa_invalido() {
        let r = SessaoCaixa::abrir(
            SessaoId::new(),
            9,
            DispositivoId::new(),
            UsuarioId::new(),
            Money::ZERO,
            false,
            Utc::now(),
        );
        assert!(r.is_err());
    }

    #[test]
    fn saldo_atualiza_com_movimentacao() {
        let mut s = sessao_exemplo();
        s.registrar_movimentacao(
            &TipoMovimentacao::VendaDinheiro,
            Money::from_centavos(10000),
            Money::ZERO,
        )
        .unwrap(); // +R$ 100
        assert_eq!(s.saldo_esperado_centavos.centavos(), 60000);
    }

    #[test]
    fn venda_pix_nao_altera_saldo_fisico() {
        let mut s = sessao_exemplo();
        s.registrar_movimentacao(
            &TipoMovimentacao::VendaPix,
            Money::from_centavos(15000),
            Money::ZERO,
        )
        .unwrap();
        assert_eq!(s.saldo_esperado_centavos.centavos(), 50000);
    }

    #[test]
    fn venda_dinheiro_desconta_troco() {
        let mut s = sessao_exemplo();
        s.registrar_movimentacao(
            &TipoMovimentacao::VendaDinheiro,
            Money::from_centavos(20000),
            Money::from_centavos(5000),
        )
        .unwrap();
        assert_eq!(s.saldo_esperado_centavos.centavos(), 65000);
    }

    #[test]
    fn fechamento_calcula_diferenca_corretamente() {
        let mut s = sessao_exemplo();
        s.registrar_movimentacao(
            &TipoMovimentacao::VendaDinheiro,
            Money::from_centavos(20000),
            Money::ZERO,
        )
        .unwrap();
        s.iniciar_fechamento().unwrap();
        // esperado = 500 + 200 = 700; contado = 600 → falta R$ 100
        let dif = s
            .finalizar_fechamento(Money::from_centavos(60000), Utc::now())
            .unwrap();
        assert_eq!(dif.esperado.centavos(), 70000);
        assert_eq!(dif.contado.centavos(), 60000);
        assert_eq!(dif.diferenca.centavos(), -10000);
        assert!(dif.e_falta());
    }

    #[test]
    fn sessao_fechada_nao_aceita_mais_movimentacao() {
        let mut s = sessao_exemplo();
        s.iniciar_fechamento().unwrap();
        s.finalizar_fechamento(Money::from_centavos(50000), Utc::now())
            .unwrap();
        let r = s.registrar_movimentacao(
            &TipoMovimentacao::VendaDinheiro,
            Money::from_centavos(100),
            Money::ZERO,
        );
        assert!(r.is_err());
    }
}
