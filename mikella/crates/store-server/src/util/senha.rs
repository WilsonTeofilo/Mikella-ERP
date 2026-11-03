//! Funções auxiliares de senha e 2FA.
//!
//! Centraliza tudo que envolve hashing de senha (Argon2id) e geração /
//! verificação de códigos 2FA. O código 2FA nunca é armazenado em claro
//! (SEG-004): guardamos apenas um hash keyed com segredo efêmero do processo.

use std::time::Duration;

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand::rngs::OsRng;
use rand::Rng;
use subtle::ConstantTimeEq;

/// Prefixo do token emitido pelo login.
pub const TOKEN_PREFIXO: &str = "mikella_";

/// Gera Argon2id hash para a senha informada.
///
/// O salt é gerado via `SaltString::generate(&mut OsRng)` (CSPRNG) e
/// inserido no próprio hash retornado — não precisa ser armazenado separado.
pub fn hash_senha(senha: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(senha.as_bytes(), &salt)
        .expect("hash de senha válida")
        .to_string()
}

/// Verifica a senha contra o hash Argon2id armazenado.
///
/// O hash armazenado deve estar no formato: `$argon2id$v=...$...`
pub fn verificar_senha(senha: &str, senha_hash: &str) -> bool {
    let parsed = match PasswordHash::new(senha_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(senha.as_bytes(), &parsed)
        .is_ok()
}

/// Hash descartável usado para equalizar o tempo de resposta quando o
/// username não existe (SEG-005, evita oráculo de timing).
pub fn hash_senha_ficticio() -> String {
    static FICTICIO: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    FICTICIO
        .get_or_init(|| hash_senha("mikella-tempo-constante"))
        .clone()
}

/// TTL de um código 2FA (5 minutos — SEG-004).
pub const DOISFA_TTL: Duration = Duration::from_secs(5 * 60);

/// Tamanho mínimo e máximo aceito para senha.
pub const SENHA_MIN: usize = 8;
pub const SENHA_MAX: usize = 256;

/// Política de senha (SEG-003): comprimento explícito, frases longas aceitas
/// e bloqueio de senhas comuns — sem regras de composição previsíveis.
pub fn senha_aceitavel(senha: &str) -> bool {
    if !(SENHA_MIN..=SENHA_MAX).contains(&senha.len()) {
        return false;
    }
    const COMUNS: &[&str] = &[
        "12345678",
        "123456789",
        "1234567890",
        "senha123",
        "senha1234",
        "password",
        "password1",
        "qwerty123",
        "admin123",
        "administrador",
        "mikella123",
        "trocar123",
        "iloveyou",
        "abc12345",
        "11111111",
    ];
    let minuscula = senha.to_lowercase();
    !COMUNS.iter().any(|c| *c == minuscula)
}

/// Gera um código numérico de 6 dígitos usando OsRng (CSPRNG).
///
/// Inclui a faixa `000000..=099999` (códigos com zero à esquerda), que a
/// versão anterior excluía — eram 10% menos combinações e um viés detectável.
pub fn gerar_codigo_2fa() -> String {
    let mut rng = OsRng;
    format!("{:06}", rng.gen_range(0..=999_999u32))
}

/// Gera o hash keyed do código 2FA (nunca armazenar o código em claro).
pub fn hash_codigo_2fa(segredo: &[u8; 32], codigo: &str) -> [u8; 32] {
    *blake3::keyed_hash(segredo, codigo.trim().as_bytes()).as_bytes()
}

/// Compara o código 2FA submetido com o hash armazenado em tempo constante.
pub fn verificar_codigo_2fa(segredo: &[u8; 32], submetido: &str, hash: &[u8; 32]) -> bool {
    let calculado = hash_codigo_2fa(segredo, submetido);
    calculado.ct_eq(hash).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_e_verificacao_sao_consistentes() {
        let hash = hash_senha("minha-senha");
        assert!(hash.starts_with("$argon2id$"));
        assert!(verificar_senha("minha-senha", &hash));
        assert!(!verificar_senha("senha-errada", &hash));
    }

    #[test]
    fn gerar_codigo_2fa_tem_seis_digitos_numericos() {
        for _ in 0..100 {
            let codigo = gerar_codigo_2fa();
            assert_eq!(codigo.len(), 6);
            assert!(codigo.chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn hash_do_codigo_nao_contem_o_codigo() {
        let segredo = [7u8; 32];
        let hash = hash_codigo_2fa(&segredo, "123456");
        assert_ne!(&hash[..], b"123456");
        assert!(verificar_codigo_2fa(&segredo, "123456", &hash));
        assert!(!verificar_codigo_2fa(&segredo, "654321", &hash));
    }

    #[test]
    fn hash_do_codigo_depende_do_segredo() {
        let a = hash_codigo_2fa(&[1u8; 32], "123456");
        let b = hash_codigo_2fa(&[2u8; 32], "123456");
        assert_ne!(a, b);
    }

    #[test]
    fn codigo_com_espacos_e_normalizado() {
        let segredo = [9u8; 32];
        let hash = hash_codigo_2fa(&segredo, "123456");
        assert!(verificar_codigo_2fa(&segredo, " 123456 ", &hash));
    }
}
