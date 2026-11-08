/// CPF e CNPJ com validação de dígitos verificadores.
/// O documento em si NUNCA vai para logs (LGPD + SEG-0XX).
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum DocumentoError {
    #[error("CPF inválido")]
    CpfInvalido,
    #[error("CNPJ inválido")]
    CnpjInvalido,
    #[error("Formato de documento inválido")]
    FormatoInvalido,
}

/// CPF validado. Armazenado internamente como 11 dígitos sem formatação.
/// NUNCA exibir em logs — impl Debug mostra apenas "CPF(****.***.**-**)"
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cpf(String); // sempre 11 chars numéricos

impl Cpf {
    pub fn parse(s: &str) -> Result<Self, DocumentoError> {
        let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() != 11 {
            return Err(DocumentoError::CpfInvalido);
        }
        if !validar_cpf(&digits) {
            return Err(DocumentoError::CpfInvalido);
        }
        Ok(Self(digits))
    }

    pub fn as_digits(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Cpf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CPF(***.***.***-**)") // nunca vaza em logs
    }
}

impl fmt::Display for Cpf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = &self.0;
        write!(f, "{}.{}.{}-{}", &d[..3], &d[3..6], &d[6..9], &d[9..])
    }
}

/// CNPJ validado. Armazenado como 14 dígitos sem formatação.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cnpj(String);

impl Cnpj {
    pub fn parse(s: &str) -> Result<Self, DocumentoError> {
        let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() != 14 {
            return Err(DocumentoError::CnpjInvalido);
        }
        if !validar_cnpj(&digits) {
            return Err(DocumentoError::CnpjInvalido);
        }
        Ok(Self(digits))
    }

    pub fn as_digits(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Cnpj {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CNPJ(**.***.***/****.** )")
    }
}

impl fmt::Display for Cnpj {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = &self.0;
        write!(
            f,
            "{}.{}.{}/{}-{}",
            &d[..2],
            &d[2..5],
            &d[5..8],
            &d[8..12],
            &d[12..]
        )
    }
}

fn validar_cpf(digits: &str) -> bool {
    let nums: Vec<u32> = digits
        .chars()
        .map(|c| c.to_digit(10).unwrap_or(0))
        .collect();

    // Rejeita sequências iguais (000.000.000-00, etc.)
    if nums.windows(2).all(|w| w[0] == w[1]) {
        return false;
    }

    let d1: u32 = (0..9).map(|i| nums[i] * (10 - i as u32)).sum::<u32>() % 11;
    let r1 = if d1 < 2 { 0 } else { 11 - d1 };
    if r1 != nums[9] {
        return false;
    }

    let d2: u32 = (0..10).map(|i| nums[i] * (11 - i as u32)).sum::<u32>() % 11;
    let r2 = if d2 < 2 { 0 } else { 11 - d2 };
    r2 == nums[10]
}

/// Mascara um documento (CPF ou CNPJ) para operadores comuns (SEC-013 / LGPD).
///
/// Exemplo:
/// - CPF:  `12345678901` -> `123.***.***-01`
/// - CNPJ: `11222333000181` -> `11.***.***/****-81`
pub fn mascarar_documento(doc: &str) -> String {
    let digits: String = doc.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 11 {
        format!("{}.***.***-{}", &digits[..3], &digits[9..])
    } else if digits.len() == 14 {
        format!("{}.***.***/****-{}", &digits[..2], &digits[12..])
    } else if doc.len() > 4 {
        let inicio = &doc[..2];
        let fim = &doc[doc.len() - 2..];
        format!("{inicio}***{fim}")
    } else {
        "***".to_string()
    }
}

fn validar_cnpj(digits: &str) -> bool {
    let nums: Vec<u32> = digits
        .chars()
        .map(|c| c.to_digit(10).unwrap_or(0))
        .collect();

    if nums.windows(2).all(|w| w[0] == w[1]) {
        return false;
    }

    let pesos1 = [5u32, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    let soma1: u32 = (0..12).map(|i| nums[i] * pesos1[i]).sum();
    let r1 = soma1 % 11;
    let d1 = if r1 < 2 { 0 } else { 11 - r1 };
    if d1 != nums[12] {
        return false;
    }

    let pesos2 = [6u32, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    let soma2: u32 = (0..13).map(|i| nums[i] * pesos2[i]).sum();
    let r2 = soma2 % 11;
    let d2 = if r2 < 2 { 0 } else { 11 - r2 };
    d2 == nums[13]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpf_valido() {
        assert!(Cpf::parse("529.982.247-25").is_ok());
        assert!(Cpf::parse("52998224725").is_ok());
    }

    #[test]
    fn cpf_invalido() {
        assert!(Cpf::parse("111.111.111-11").is_err()); // sequência igual
        assert!(Cpf::parse("000.000.000-00").is_err());
        assert!(Cpf::parse("123.456.789-00").is_err()); // dígito errado
    }

    #[test]
    fn cnpj_valido() {
        assert!(Cnpj::parse("11.222.333/0001-81").is_ok());
    }

    #[test]
    fn cnpj_invalido() {
        assert!(Cnpj::parse("11.111.111/1111-11").is_err());
    }

    #[test]
    fn debug_nao_vaza_cpf() {
        let cpf = Cpf::parse("529.982.247-25").unwrap();
        let debug = format!("{:?}", cpf);
        assert!(!debug.contains("529"));
        assert!(!debug.contains("247"));
    }

    #[test]
    fn mascaramento_cpf_e_cnpj() {
        assert_eq!(mascarar_documento("52998224725"), "529.***.***-25");
        assert_eq!(mascarar_documento("529.982.247-25"), "529.***.***-25");
        assert_eq!(
            mascarar_documento("11222333000181"),
            "11.***.***/****-81"
        );
        assert_eq!(mascarar_documento("12345"), "12***45");
        assert_eq!(mascarar_documento("12"), "***");
    }
}
