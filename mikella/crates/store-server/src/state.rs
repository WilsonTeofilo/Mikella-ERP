//! Estado da aplicação e backend de e-mail injetável.
//!
//! `AppState` é compartilhado entre todos os handlers via `axum::extract::State`.
//! Contém o banco (com Mutex para serializar o único escritor) e os
//! repositórios in-memory de sessões, códigos 2FA e rate limit.
//!
//! SEG-004: o código 2FA nunca é guardado em claro — apenas um hash keyed
//! (blake3) com segredo efêmero do processo. SEG-006: tentativas de login e
//! 2FA são limitadas por conta. Nada disso é persistido em disco.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mikella_infra_sqlite::Database;
use rand::rngs::OsRng;
use rand::RngCore;
use tokio::sync::Mutex;

/// TTL absoluto da sessão local (SEG-012).
pub const SESSAO_TTL: Duration = Duration::from_secs(12 * 60 * 60);

/// Inatividade máxima antes de exigir novo login.
pub const SESSAO_IDLE: Duration = Duration::from_secs(30 * 60);

/// Máximo de tentativas de senha antes de bloquear temporariamente a conta.
pub const MAX_TENTATIVAS_LOGIN: u32 = 5;

/// Máximo de tentativas de código 2FA por código emitido.
pub const MAX_TENTATIVAS_2FA: u8 = 5;

/// Janela de bloqueio após exceder as tentativas (SEG-006).
pub const JANELA_BLOQUEIO: Duration = Duration::from_secs(5 * 60);

/// Trait para envio de e-mails.
///
/// Implementação injetável permite testes unitários sem provider real.
/// A assinatura é `Send + Sync` porque o `Arc<dyn EmailBackend>` precisa
/// ser compartilhado entre tasks tokio.
pub trait EmailBackend: Send + Sync {
    /// Envia e-mail para `to` com assunto `subject` e corpo `body`.
    fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), std::io::Error>;

    /// Indica se existe um canal de entrega utilizável.
    ///
    /// O onboarding recusa criar o banco quando não há como entregar o
    /// código 2FA — caso contrário a loja nasceria inacessível.
    fn disponivel(&self) -> bool {
        true
    }
}

/// Implementação de console para desenvolvimento local.
///
/// Só imprime o corpo (que contém o código) quando `MIKELLA_EMAIL_CONSOLE=1`.
/// Em compilação de release sem essa variável, o backend se declara
/// indisponível e o onboarding falha antes de gravar qualquer dado.
pub struct ConsoleEmailBackend {
    habilitado: bool,
}

impl ConsoleEmailBackend {
    pub fn new() -> Self {
        // SEG-080: backend de desenvolvimento é opt-in explícito. Ligá-lo por
        // padrão colocava o código 2FA no stderr de qualquer instalação.
        let habilitado = std::env::var("MIKELLA_EMAIL_CONSOLE")
            .map(|v| v == "1")
            .unwrap_or(false);
        Self { habilitado }
    }
}

impl Default for ConsoleEmailBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl EmailBackend for ConsoleEmailBackend {
    fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), std::io::Error> {
        if !self.habilitado {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "backend de e-mail de console desabilitado (defina MIKELLA_EMAIL_CONSOLE=1)",
            ));
        }
        // SEG-080: e-mail mascarado e corpo omitido por padrão, mesmo no
        // backend de console. O corpo (que contém o código 2FA) só aparece com
        // MIKELLA_EMAIL_CONSOLE_CORPO=1, jamais em produção.
        let mostrar_corpo = std::env::var("MIKELLA_EMAIL_CONSOLE_CORPO")
            .map(|v| v == "1")
            .unwrap_or(false);
        if mostrar_corpo {
            eprintln!(
                "[ConsoleEmailBackend][DEV] para: {} | assunto: {} | corpo: {}",
                mascarar_email(to),
                subject,
                body
            );
        } else {
            eprintln!(
                "[ConsoleEmailBackend][DEV] para: {} | assunto: {} | corpo omitido \
                 (defina MIKELLA_EMAIL_CONSOLE_CORPO=1 para ver o código em dev)",
                mascarar_email(to),
                subject
            );
        }
        Ok(())
    }

    fn disponivel(&self) -> bool {
        self.habilitado
    }
}

/// Mascara a parte local do e-mail para não gravar PII em log.
fn mascarar_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, dominio)) if !local.is_empty() => {
            let inicial: String = local.chars().take(1).collect();
            format!("{inicial}***@{dominio}")
        }
        _ => "<inválido>".to_string(),
    }
}

/// Sessão autenticada em memória.
#[derive(Debug, Clone)]
pub struct SessaoAtiva {
    pub username: String,
    pub criada_em: Instant,
    pub ultimo_uso: Instant,
}

/// Código 2FA em memória. Guarda apenas o hash keyed (SEG-004).
#[derive(Debug, Clone)]
pub struct Codigo2FA {
    pub username: String,
    pub hash: [u8; 32],
    pub expira_em: Instant,
    pub tentativas: u8,
}

/// Contador de falhas por chave (conta + finalidade).
#[derive(Debug, Clone, Default)]
pub struct Tentativas {
    pub falhas: u32,
    pub bloqueado_ate: Option<Instant>,
}

/// Rate limiter simples em memória (SEG-006).
#[derive(Debug, Default)]
pub struct RateLimiter {
    inner: HashMap<String, Tentativas>,
}

impl RateLimiter {
    /// Retorna `true` se a chave ainda pode tentar.
    pub fn permitir(&mut self, chave: &str) -> bool {
        let agora = Instant::now();
        match self.inner.get(chave) {
            Some(t) => t.bloqueado_ate.is_none_or(|fim| agora >= fim),
            None => true,
        }
    }

    /// Registra uma falha; bloqueia ao atingir `MAX_TENTATIVAS_LOGIN`.
    pub fn registrar_falha(&mut self, chave: &str) {
        let agora = Instant::now();
        let entrada = self.inner.entry(chave.to_string()).or_default();
        if entrada.bloqueado_ate.is_some_and(|fim| agora >= fim) {
            entrada.falhas = 0;
            entrada.bloqueado_ate = None;
        }
        entrada.falhas = entrada.falhas.saturating_add(1);
        if entrada.falhas >= MAX_TENTATIVAS_LOGIN {
            entrada.bloqueado_ate = Some(agora + JANELA_BLOQUEIO);
        }
    }

    /// Limpa o contador após sucesso.
    pub fn limpar(&mut self, chave: &str) {
        self.inner.remove(chave);
    }

    /// Remove entradas já expiradas (chamado oportunisticamente).
    pub fn limpar_expirados(&mut self) {
        let agora = Instant::now();
        self.inner
            .retain(|_, t| t.bloqueado_ate.is_none_or(|fim| agora < fim));
    }
}

/// Estado compartilhado entre handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    /// token -> sessão (SEG-012: no máximo uma por usuário).
    pub sessoes: Arc<Mutex<HashMap<String, SessaoAtiva>>>,
    /// username normalizado -> código 2FA hasheado.
    pub codigos_2fa: Arc<Mutex<HashMap<String, Codigo2FA>>>,
    /// Rate limit por conta/finalidade.
    pub rate_limit: Arc<Mutex<RateLimiter>>,
    /// Segredo efêmero para o keyed hash do código 2FA.
    pub segredo_2fa: Arc<[u8; 32]>,
    /// Chave estável da loja — deriva blind indexes de CPF/CNPJ (SEC-014 / LGPD-003).
    /// Lida de `MIKELLA_STORE_KEY` (hex 64 chars). Nunca rotacionar sem migrar os blinds.
    pub chave_loja: Arc<[u8; 32]>,
    /// Backend de e-mail (injeta via trait).
    pub email: Arc<dyn EmailBackend>,
}

impl AppState {
    /// Cria um novo estado com o banco já aberto e o backend de e-mail fornecido.
    ///
    /// Em testes e execuções sem `MIKELLA_STORE_KEY`, gera uma chave efêmera em
    /// memória. O boot de produção usa [`AppState::com_chave_loja`] depois de
    /// [`resolver_chave_loja`], garantindo uma chave estável entre reinícios.
    #[cfg(test)]
    pub fn new(db: Database, email: Arc<dyn EmailBackend>) -> Self {
        let chave_loja = chave_loja_do_env().unwrap_or_else(chave_efemera);
        Self::com_chave_loja(db, email, chave_loja)
    }

    /// Cria o estado com uma chave de loja já resolvida (boot de produção).
    pub fn com_chave_loja(
        db: Database,
        email: Arc<dyn EmailBackend>,
        chave_loja: [u8; 32],
    ) -> Self {
        let mut segredo = [0u8; 32];
        OsRng.fill_bytes(&mut segredo);

        Self {
            db: Arc::new(Mutex::new(db)),
            sessoes: Arc::new(Mutex::new(HashMap::new())),
            codigos_2fa: Arc::new(Mutex::new(HashMap::new())),
            rate_limit: Arc::new(Mutex::new(RateLimiter::default())),
            segredo_2fa: Arc::new(segredo),
            chave_loja: Arc::new(chave_loja),
            email,
        }
    }

    /// Normaliza o username para uso como chave (trim + lowercase).
    pub fn chave_usuario(username: &str) -> String {
        username.trim().to_lowercase()
    }
}

/// Resolve a chave estável da loja para o blind index de documentos.
///
/// Ordem de precedência:
/// 1. `MIKELLA_STORE_KEY` (hex de 64 chars) — recomendado em produção.
/// 2. Arquivo `MIKELLA_KEY_PATH` ou `loja.key` ao lado do banco — gerado com
///    32 bytes aleatórios no primeiro boot.
///
/// Uma variável presente mas inválida é erro fatal: cair para a chave zero ou
/// para uma chave nova silenciosamente invalidaria todos os blinds existentes.
pub fn resolver_chave_loja(db_path: &str) -> io::Result<[u8; 32]> {
    if let Ok(hex) = std::env::var("MIKELLA_STORE_KEY") {
        return parse_chave_hex(&hex).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "MIKELLA_STORE_KEY inválida: use exatamente 64 caracteres hexadecimais",
            )
        });
    }

    let caminho = caminho_chave(db_path);
    match fs::read_to_string(&caminho) {
        Ok(conteudo) => parse_chave_hex(&conteudo).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "arquivo de chave da loja inválido: {} (use 64 caracteres hexadecimais)",
                    caminho.display()
                ),
            )
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let chave = chave_efemera();
            if let Some(dir) = caminho.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(&caminho, chave_para_hex(&chave))?;
            aplicar_permissao_restrita(&caminho);
            tracing::warn!(
                caminho = %caminho.display(),
                "MIKELLA_STORE_KEY ausente; chave da loja gerada e persistida. \
                 Proteja este arquivo e os backups que o incluírem."
            );
            Ok(chave)
        }
        Err(e) => Err(e),
    }
}

fn caminho_chave(db_path: &str) -> PathBuf {
    if let Ok(caminho) = std::env::var("MIKELLA_KEY_PATH") {
        return PathBuf::from(caminho);
    }
    let banco = Path::new(db_path);
    let dir = banco
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("dados"));
    dir.join("loja.key")
}

fn chave_efemera() -> [u8; 32] {
    let mut chave = [0u8; 32];
    OsRng.fill_bytes(&mut chave);
    chave
}

fn parse_chave_hex(hex: &str) -> Option<[u8; 32]> {
    let hex = hex.trim();
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut chave = [0u8; 32];
    for (i, byte) in chave.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(chave)
}

fn chave_para_hex(chave: &[u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(64);
    for byte in chave {
        // Escrever em String não falha; o resultado é ignorado de propósito.
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

#[cfg(unix)]
fn aplicar_permissao_restrita(caminho: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadados) = fs::metadata(caminho) {
        let mut permissoes = metadados.permissions();
        permissoes.set_mode(0o600);
        let _ = fs::set_permissions(caminho, permissoes);
    }
}

#[cfg(not(unix))]
fn aplicar_permissao_restrita(_caminho: &Path) {
    // Windows: a ACL herdada da pasta do usuário já limita o acesso. Um
    // endurecimento explícito via ACL fica para o instalador (OPS).
}

/// Lê a chave estável da loja de `MIKELLA_STORE_KEY` (hex 64 chars = 32 bytes).
#[cfg(test)]
fn chave_loja_do_env() -> Option<[u8; 32]> {
    match std::env::var("MIKELLA_STORE_KEY") {
        Ok(hex) => match parse_chave_hex(&hex) {
            Some(chave) => Some(chave),
            None => {
                tracing::error!(
                    "MIKELLA_STORE_KEY tem comprimento/formato incorreto \
                     (deve ser hex de 64 chars); usando chave efêmera nesta execução."
                );
                None
            }
        },
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appstate_clone_com_console_email_backend() {
        let db = Database::em_memoria().unwrap();
        let email: Arc<dyn EmailBackend> = Arc::new(ConsoleEmailBackend::new());
        let estado = AppState::new(db, email);
        let _estado_clonado = estado.clone();
    }

    #[test]
    fn rate_limiter_bloqueia_apos_limite() {
        let mut rl = RateLimiter::default();
        for _ in 0..MAX_TENTATIVAS_LOGIN {
            assert!(rl.permitir("login:joao"));
            rl.registrar_falha("login:joao");
        }
        assert!(!rl.permitir("login:joao"));
        rl.limpar("login:joao");
        assert!(rl.permitir("login:joao"));
    }

    #[test]
    fn chave_usuario_e_case_insensitive() {
        assert_eq!(AppState::chave_usuario("  Joao "), "joao");
    }

    #[test]
    fn email_e_mascarado_no_log() {
        assert_eq!(mascarar_email("joao@loja.com"), "j***@loja.com");
        assert_eq!(mascarar_email("sem-arroba"), "<inválido>");
        assert_eq!(mascarar_email("@loja.com"), "<inválido>");
    }
}
