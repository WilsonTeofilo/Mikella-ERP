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
    pub fn partes(self) -> (i64, u8) {
        let abs = self.0.unsigned_abs();
        let reais = (abs / 100) as i64 * self.0.signum();
        (reais, (abs % 100) as u8)
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
    fn serde_roundtrip() {
        let m = Money::from_centavos(9999);
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, "9999");
        let de: Money = serde_json::from_str(&json).unwrap();
        assert_eq!(de, m);
    }
}
