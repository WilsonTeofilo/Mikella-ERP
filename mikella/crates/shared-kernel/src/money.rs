//! Valor monetário em centavos inteiros.
//!
//! REGRA INVIOLÁVEL: todo dinheiro é representado como `Money` (centavos inteiros).
//! Nunca `f32`, nunca `f64`, nunca `Decimal` com ponto flutuante.
//!
//! Exemplos:
//! ```
//! use mikella_shared_kernel::money::Money;
//!
//! let preco = Money::from_centavos(12990); // R$ 129,90
//! let outro = Money::from_reais(129, 90).unwrap();
//! assert_eq!(preco, outro);
//! assert_eq!(preco.centavos(), 12990);
//! # Ok::<(), mikella_shared_kernel::money::MoneyError>(())
//! ```
use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(i64); // centavos; negativo = débito/desconto

#[derive(Debug, Error, PartialEq)]
pub enum MoneyError {
    #[error("Valor negativo não permitido neste contexto: {0} centavos")]
    ValorNegativo(i64),
    #[error("Overflow aritmético ao operar com Money")]
    Overflow,
    #[error("Centavos ({centavos}) devem estar entre 0 e 99")]
    CentavosInvalidos { centavos: u8 },
    #[error("Percentual inválido: {0} (esperado de 0 a 100)")]
    PercentualInvalido(u8),
    #[error("Número de parcelas inválido: {0} (deve ser entre 1 e 120)")]
    ParcelasInvalidas(u32),
}

impl Money {
    /// Cria `Money` a partir de centavos (pode ser negativo para débitos).
    #[inline]
    pub const fn from_centavos(centavos: i64) -> Self {
        Self(centavos)
    }

    /// Cria `Money` a partir de reais e centavos (ambos não negativos).
    ///
    /// # Errors
    /// Retorna erro se `centavos_parte >= 100`, se `reais` for negativo
    /// ou se o valor em centavos estourar `i64`.
    pub fn from_reais(reais: i64, centavos_parte: u8) -> Result<Self, MoneyError> {
        if reais < 0 {
            return Err(MoneyError::ValorNegativo(reais));
        }
        if centavos_parte > 99 {
            return Err(MoneyError::CentavosInvalidos {
                centavos: centavos_parte,
            });
        }
        reais
            .checked_mul(100)
            .and_then(|v| v.checked_add(i64::from(centavos_parte)))
            .map(Self)
            .ok_or(MoneyError::Overflow)
    }

    /// Retorna o valor em centavos.
    #[inline]
    pub const fn centavos(self) -> i64 {
        self.0
    }

    /// Retorna `true` se o valor é zero.
    #[inline]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Retorna `true` se o valor é positivo (> 0).
    #[inline]
    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Retorna `true` se o valor é negativo (< 0).
    #[inline]
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Garante que o valor não é negativo.
    /// Use em contextos onde negativos são inválidos (preço, total de venda).
    pub fn assert_nao_negativo(self) -> Result<Self, MoneyError> {
        if self.0 < 0 {
            Err(MoneyError::ValorNegativo(self.0))
        } else {
            Ok(self)
        }
    }

    /// Soma com checagem explícita de overflow.
    #[inline]
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    /// Subtração com checagem explícita de overflow.
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    /// Soma que devolve erro de domínio em vez de estourar em silêncio.
    pub fn somar(self, rhs: Self) -> Result<Self, MoneyError> {
        self.checked_add(rhs).ok_or(MoneyError::Overflow)
    }

    /// Subtração que devolve erro de domínio em vez de estourar em silêncio.
    pub fn subtrair(self, rhs: Self) -> Result<Self, MoneyError> {
        self.checked_sub(rhs).ok_or(MoneyError::Overflow)
    }

    /// Multiplica por uma quantidade inteira (ex: preço × qtd).
    pub fn mul_qty(self, qty: u32) -> Result<Self, MoneyError> {
        self.0
            .checked_mul(i64::from(qty))
            .map(Self)
            .ok_or(MoneyError::Overflow)
    }

    /// Aplica percentual de desconto (0..=100), arredondando o centavo para baixo
    /// (o desconto é truncado, portanto o valor final nunca fica menor que o devido).
    pub fn aplicar_desconto_pct(self, pct: u8) -> Result<Self, MoneyError> {
        if pct > 100 {
            return Err(MoneyError::PercentualInvalido(pct));
        }
        let desconto = self.0 / 100 * i64::from(pct) + (self.0 % 100) * i64::from(pct) / 100;
        self.checked_sub(Self(desconto)).ok_or(MoneyError::Overflow)
    }

    /// Retorna `(reais, centavos)` para exibição. O sinal acompanha `reais`.
    ///
    /// Atenção: para valores entre -0,01 e -0,99 centavos, `reais` é `0`.
    /// Nesses casos, use [`partes_sinal`](Self::partes_sinal) para preservar o sinal negativo.
    pub fn partes(self) -> (i64, u8) {
        let abs = self.0.unsigned_abs();
        let reais = (abs / 100) as i64 * self.0.signum();
        (reais, (abs % 100) as u8)
    }

    /// Retorna `(negativo, reais_abs, centavos)` garantindo que o sinal nunca se perca (DAT-010),
    /// mesmo para valores fracionários negativos entre -1 e -99 centavos.
    pub fn partes_sinal(self) -> (bool, u64, u8) {
        let abs = self.0.unsigned_abs();
        let negativo = self.0 < 0;
        let reais = abs / 100;
        let centavos = (abs % 100) as u8;
        (negativo, reais, centavos)
    }

    /// Divide o valor monetário em `n` parcelas inteiras em centavos.
    ///
    /// Regra padrão de ERPs brasileiros:
    /// - As primeiras `n - 1` parcelas recebem o valor base `total / n`.
    /// - A última parcela (`n - 1`) recebe `base + (total % n)` (absorve integralmente o resto da divisão),
    ///   garantindo que a soma exata de todas as parcelas seja estritamente igual a `self`.
    /// - `n` deve estar no intervalo `1..=120`.
    pub fn dividir_em_parcelas(self, n: u32) -> Result<Vec<Money>, MoneyError> {
        if n == 0 || n > 120 {
            return Err(MoneyError::ParcelasInvalidas(n));
        }
        if self.0 < 0 {
            return Err(MoneyError::ValorNegativo(self.0));
        }
        let n_i64 = i64::from(n);
        let base = self.0 / n_i64;
        let resto = self.0 % n_i64;

        let mut parcelas = Vec::with_capacity(n as usize);
        for i in 0..n {
            if i == n - 1 {
                parcelas.push(Money::from_centavos(base + resto));
            } else {
                parcelas.push(Money::from_centavos(base));
            }
        }
        Ok(parcelas)
    }

    pub const ZERO: Money = Money(0);
}

impl Add for Money {
    type Output = Money;

    /// Soma. Em caso de overflow (impossível com valores reais de varejo,
    /// `i64` cobre ~9,2 × 10^16 centavos) o programa falha em vez de
    /// produzir um total errado em silêncio. Prefira `somar()` no caminho crítico.
    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs)
            .expect("overflow aritmético em Money::add")
    }
}

impl Sub for Money {
    type Output = Money;

    /// Subtração, com a mesma política de `Add`.
    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_sub(rhs)
            .expect("overflow aritmético em Money::sub")
    }
}

impl Mul<u32> for Money {
    type Output = Result<Money, MoneyError>;

    fn mul(self, rhs: u32) -> Self::Output {
        self.mul_qty(rhs)
    }
}

impl Neg for Money {
    type Output = Money;

    fn neg(self) -> Self::Output {
        Self(
            self.0
                .checked_neg()
                .expect("overflow aritmético em Money::neg"),
        )
    }
}

impl fmt::Display for Money {
    /// Formata como "R$ 129,90" ou "−R$ 10,00" (padrão brasileiro).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.0.unsigned_abs();
        let reais = abs / 100;
        let centavos = abs % 100;
        if self.0 < 0 {
            write!(f, "−R$ {reais},{centavos:02}")
        } else {
            write!(f, "R$ {reais},{centavos:02}")
        }
    }
}

impl Default for Money {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_reais_valido() {
        let m = Money::from_reais(129, 90).unwrap();
        assert_eq!(m.centavos(), 12990);
    }

    #[test]
    fn from_reais_centavos_invalidos() {
        assert!(Money::from_reais(10, 100).is_err());
    }

    #[test]
    fn from_reais_negativo_falha() {
        assert_eq!(Money::from_reais(-1, 0), Err(MoneyError::ValorNegativo(-1)));
    }

    #[test]
    fn from_reais_overflow_falha() {
        assert_eq!(Money::from_reais(i64::MAX, 0), Err(MoneyError::Overflow));
    }

    #[test]
    fn soma_centavos() {
        let a = Money::from_centavos(100);
        let b = Money::from_centavos(250);
        assert_eq!((a + b).centavos(), 350);
    }

    #[test]
    fn subtrai_centavos() {
        let a = Money::from_centavos(500);
        let b = Money::from_centavos(200);
        assert_eq!((a - b).centavos(), 300);
    }

    #[test]
    fn checked_overflow_devolve_none() {
        let max = Money::from_centavos(i64::MAX);
        assert_eq!(max.checked_add(Money::from_centavos(1)), None);
        assert!(max.somar(Money::from_centavos(1)).is_err());
        let min = Money::from_centavos(i64::MIN);
        assert_eq!(min.checked_sub(Money::from_centavos(1)), None);
        assert!(min.subtrair(Money::from_centavos(1)).is_err());
    }

    #[test]
    fn mul_qty() {
        let preco = Money::from_centavos(1990); // R$ 19,90
        assert_eq!(preco.mul_qty(3).unwrap().centavos(), 5970);
    }

    #[test]
    fn mul_qty_overflow() {
        assert!(Money::from_centavos(i64::MAX).mul_qty(2).is_err());
    }

    #[test]
    fn desconto_pct() {
        let preco = Money::from_centavos(10000); // R$ 100,00
        assert_eq!(preco.aplicar_desconto_pct(10).unwrap().centavos(), 9000); // R$ 90,00
    }

    #[test]
    fn desconto_pct_arredonda_para_baixo() {
        // 33% de 99 centavos = 32,67 → desconto truncado em 32 → 67
        assert_eq!(
            Money::from_centavos(99)
                .aplicar_desconto_pct(33)
                .unwrap()
                .centavos(),
            67
        );
    }

    #[test]
    fn desconto_pct_invalido() {
        assert_eq!(
            Money::from_centavos(100).aplicar_desconto_pct(101),
            Err(MoneyError::PercentualInvalido(101))
        );
    }

    #[test]
    fn display_positivo_usa_virgula() {
        let m = Money::from_centavos(12990);
        assert_eq!(format!("{m}"), "R$ 129,90");
    }

    #[test]
    fn display_negativo_usa_virgula() {
        let m = Money::from_centavos(-1000);
        assert_eq!(format!("{m}"), "−R$ 10,00");
    }

    #[test]
    fn display_milhar() {
        let m = Money::from_reais(1234, 56).unwrap();
        assert_eq!(format!("{m}"), "R$ 1234,56");
    }

    #[test]
    fn assert_nao_negativo_ok() {
        assert!(Money::from_centavos(0).assert_nao_negativo().is_ok());
        assert!(Money::from_centavos(1).assert_nao_negativo().is_ok());
    }

    #[test]
    fn assert_nao_negativo_err() {
        assert!(Money::from_centavos(-1).assert_nao_negativo().is_err());
    }

    #[test]
    fn partes() {
        let m = Money::from_centavos(12990);
        assert_eq!(m.partes(), (129, 90));
    }

    #[test]
    fn partes_negativo() {
        let m = Money::from_centavos(-12990);
        assert_eq!(m.partes(), (-129, 90));
    }

    #[test]
    fn partes_sinal_preserva_fracao_negativa() {
        // DAT-010: entre -1 e -99 centavos, reais é 0 mas o sinal não pode se perder
        let m = Money::from_centavos(-50);
        assert_eq!(m.partes_sinal(), (true, 0, 50));

        let zero = Money::from_centavos(0);
        assert_eq!(zero.partes_sinal(), (false, 0, 0));

        let positivo = Money::from_centavos(150);
        assert_eq!(positivo.partes_sinal(), (false, 1, 50));
    }

    #[test]
    fn serde_roundtrip() {
        let m = Money::from_centavos(9999);
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, "9999");
        let de: Money = serde_json::from_str(&json).unwrap();
        assert_eq!(de, m);
    }

    #[test]
    fn dividir_em_parcelas_de_1_a_12_com_resto_na_ultima() {
        let valores_teste = [
            455,    // R$ 4,55 (com divisões ímpares como 7x e 11x)
            10_000, // R$ 100,00
            1_001,  // R$ 10,01
            5,      // R$ 0,05 (menor que o número de parcelas para testar centavos 0)
            12_347, // R$ 123,47
            99,     // R$ 0,99
        ];

        for &centavos in &valores_teste {
            let total = Money::from_centavos(centavos);
            for n in 1..=12 {
                let parcelas = total.dividir_em_parcelas(n).expect("divisão válida");
                assert_eq!(parcelas.len(), n as usize, "Total {centavos} em {n} parcelas");

                // 1. Invariante fundamental: A soma de todas as parcelas é EXATAMENTE igual ao total
                let soma: i64 = parcelas.iter().map(|p| p.centavos()).sum();
                assert_eq!(
                    soma, centavos,
                    "A soma das {n} parcelas de {centavos} difere do total original!"
                );

                let base = centavos / i64::from(n);
                let resto = centavos % i64::from(n);

                // 2. As primeiras n - 1 parcelas têm o valor base
                for (i, p) in parcelas[..parcelas.len() - 1].iter().enumerate() {
                    assert_eq!(
                        p.centavos(),
                        base,
                        "Parcela {} de {}x de total {} deveria ser a base {}",
                        i + 1, n, centavos, base
                    );
                }

                // 3. A ÚLTIMA parcela absorve o resto da divisão
                let ultima = parcelas.last().unwrap().centavos();
                assert_eq!(
                    ultima,
                    base + resto,
                    "Última parcela de {}x de total {} deveria ter base ({}) + resto ({}) = {}",
                    n, centavos, base, resto, base + resto
                );
            }
        }
    }

    #[test]
    fn dividir_em_parcelas_limites_e_erros() {
        let valor = Money::from_centavos(1000);
        assert_eq!(valor.dividir_em_parcelas(0), Err(MoneyError::ParcelasInvalidas(0)));
        assert_eq!(valor.dividir_em_parcelas(121), Err(MoneyError::ParcelasInvalidas(121)));

        let negativo = Money::from_centavos(-100);
        assert_eq!(negativo.dividir_em_parcelas(3), Err(MoneyError::ValorNegativo(-100)));
    }
}
