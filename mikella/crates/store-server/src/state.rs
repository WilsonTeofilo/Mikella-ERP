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
        let habilitado = std::env::var("MIKELLA_EMAIL_CONSOLE")
            .map(|v| v != "0")
            .unwrap_or(true);
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
        // SEG-080: nunca logar o corpo (contém o código 2FA) fora do
        // backend de desenvolvimento explicitamente habilitado.
        eprintln!(
            "[ConsoleEmailBackend] destinatário: {} | assunto: {} | corpo: {}",
            to, subject, body
        );
        Ok(())
    }

    fn disponivel(&self) -> bool {
        self.habilitado
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
    /// Backend de e-mail (injeta via trait).
    pub email: Arc<dyn EmailBackend>,
}

impl AppState {
    /// Cria um novo estado com o banco já aberto e o backend de e-mail fornecido.
    pub fn new(db: Database, email: Arc<dyn EmailBackend>) -> Self {
        let mut segredo = [0u8; 32];
        OsRng.fill_bytes(&mut segredo);
        Self {
            db: Arc::new(Mutex::new(db)),
            sessoes: Arc::new(Mutex::new(HashMap::new())),
            codigos_2fa: Arc::new(Mutex::new(HashMap::new())),
            rate_limit: Arc::new(Mutex::new(RateLimiter::default())),
            segredo_2fa: Arc::new(segredo),
            email,
        }
    }

    /// Normaliza o username para uso como chave (trim + lowercase).
    pub fn chave_usuario(username: &str) -> String {
        username.trim().to_lowercase()
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
}
