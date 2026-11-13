//! mikella-infra-crypto
//!
//! Primitivas criptográficas para proteção de dados em repouso (LGPD / SEC-040 / SEC-091)
//! e segurança operacional.
//!
//! Implementa AES-256-GCM para criptografia autenticada (AEAD) de dados pessoais (PII).

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use thiserror::Error;

pub const PREFIXO_CIFRADO: &str = "ENC:";
const NONCE_LEN: usize = 12;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    #[error("Falha ao cifrar dados")]
    CifragemFalhou,
    #[error("Falha ao decifrar dados (chave incorreta ou dado corrompido)")]
    DecifragemFalhou,
    #[error("Payload cifrado inválido ou mal formatado")]
    PayloadInvalido,
}

/// Cifra uma string em texto claro usando AES-256-GCM com chave de 32 bytes e nonce aleatório de 96 bits.
///
/// O resultado é prefixado com `ENC:` seguido de codificação hexadecimal do nonce + ciphertext + tag.
pub fn cifrar_texto(texto: &str, chave: &[u8; 32]) -> Result<String, CryptoError> {
    if texto.is_empty() {
        return Ok(String::new());
    }

    let cipher = Aes256Gcm::new_from_slice(chave).map_err(|_| CryptoError::CifragemFalhou)?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, texto.as_bytes())
        .map_err(|_| CryptoError::CifragemFalhou)?;

    // Combina: [12 bytes nonce] + [ciphertext + tag]
    let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    payload.extend_from_slice(&nonce_bytes);
    payload.extend_from_slice(&ciphertext);

    Ok(format!("{}{}", PREFIXO_CIFRADO, hex::encode(payload)))
}

/// Cifra um dado pessoal (PII como telefone, documento, email) com AEAD determinístico (SIV).
///
/// O nonce de 96 bits é derivado via BLAKE3 keyed da chave + contexto + dado.
/// Isso permite que buscas por igualdade (`WHERE telefone = ?` ou `documento = ?`)
/// funcionem de forma exata e rápida no SQLite, mantendo o banco completamente
/// protegido contra dumps físicos em repouso (LGPD Art. 46 / SEC-040).
pub fn cifrar_pii(texto: &str, chave: &[u8; 32], contexto: &str) -> Result<String, CryptoError> {
    if texto.is_empty() {
        return Ok(String::new());
    }

    let mut hasher = blake3::Hasher::new_keyed(chave);
    hasher.update(b"mikella.pii.v1:");
    hasher.update(contexto.as_bytes());
    hasher.update(b":");
    hasher.update(texto.as_bytes());
    let nonce_bytes: [u8; NONCE_LEN] = hasher.finalize().as_bytes()[..NONCE_LEN]
        .try_into()
        .map_err(|_| CryptoError::CifragemFalhou)?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let cipher = Aes256Gcm::new_from_slice(chave).map_err(|_| CryptoError::CifragemFalhou)?;
    let ciphertext = cipher
        .encrypt(nonce, texto.as_bytes())
        .map_err(|_| CryptoError::CifragemFalhou)?;

    let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    payload.extend_from_slice(&nonce_bytes);
    payload.extend_from_slice(&ciphertext);

    Ok(format!("{}{}", PREFIXO_CIFRADO, hex::encode(payload)))
}

/// Decifra uma string que foi cifrada com `cifrar_texto`.
///
/// Se o texto NÃO começar com `ENC:`, assume que é texto legado e retorna o próprio texto (backward-compatibility).
pub fn decifrar_texto(valor: &str, chave: &[u8; 32]) -> Result<String, CryptoError> {
    let Some(resto) = valor.strip_prefix(PREFIXO_CIFRADO) else {
        // Texto em claro legado ou não cifrado
        return Ok(valor.to_string());
    };

    if resto.is_empty() {
        return Ok(String::new());
    }

    let payload = hex::decode(resto).map_err(|_| CryptoError::PayloadInvalido)?;
    if payload.len() < NONCE_LEN + 16 {
        return Err(CryptoError::PayloadInvalido);
    }

    let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(chave).map_err(|_| CryptoError::DecifragemFalhou)?;
    let nonce = Nonce::from_slice(nonce_bytes);

    let bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::DecifragemFalhou)?;

    String::from_utf8(bytes).map_err(|_| CryptoError::PayloadInvalido)
}

mod hex {
    pub fn encode<T: AsRef<[u8]>>(data: T) -> String {
        let mut s = String::with_capacity(data.as_ref().len() * 2);
        for &b in data.as_ref() {
            use std::fmt::Write;
            let _ = write!(s, "{:02x}", b);
        }
        s
    }

    pub fn decode(hex_str: &str) -> Result<Vec<u8>, ()> {
        if !hex_str.len().is_multiple_of(2) {
            return Err(());
        }
        let mut bytes = Vec::with_capacity(hex_str.len() / 2);
        for i in (0..hex_str.len()).step_by(2) {
            let byte = u8::from_str_radix(&hex_str[i..i + 2], 16).map_err(|_| ())?;
            bytes.push(byte);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_cifrar_decifrar() {
        let chave = [42u8; 32];
        let original = "11987654321";

        let cifrado = cifrar_texto(original, &chave).unwrap();
        assert!(cifrado.starts_with(PREFIXO_CIFRADO));
        assert_ne!(cifrado, original);

        let decifrado = decifrar_texto(&cifrado, &chave).unwrap();
        assert_eq!(decifrado, original);
    }

    #[test]
    fn decifrar_legado_sem_prefixo_preserva_texto() {
        let chave = [42u8; 32];
        let legado = "11987654321";
        let res = decifrar_texto(legado, &chave).unwrap();
        assert_eq!(res, legado);
    }

    #[test]
    fn decifrar_com_chave_errada_falha() {
        let chave1 = [1u8; 32];
        let chave2 = [2u8; 32];
        let original = "segredo123";

        let cifrado = cifrar_texto(original, &chave1).unwrap();
        let res = decifrar_texto(&cifrado, &chave2);
        assert_eq!(res, Err(CryptoError::DecifragemFalhou));
    }

    #[test]
    fn cifrar_pii_e_deterministico_para_mesmo_dado_e_contexto() {
        let chave = [77u8; 32];
        let tel = "11988887777";

        let c1 = cifrar_pii(tel, &chave, "telefone").unwrap();
        let c2 = cifrar_pii(tel, &chave, "telefone").unwrap();
        assert_eq!(
            c1, c2,
            "cifragem PII com mesmo contexto deve ser determinística"
        );

        // Contextos diferentes produzem ciphertexts diferentes
        let c_outro = cifrar_pii(tel, &chave, "outro").unwrap();
        assert_ne!(c1, c_outro);

        // Decifragem comum decifra sem problema
        let d = decifrar_texto(&c1, &chave).unwrap();
        assert_eq!(d, tel);
    }
}
