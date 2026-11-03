/// Porta (trait) para obtenção do tempo atual — permite testes determinísticos.
/// O domínio NUNCA chama `chrono::Utc::now()` diretamente.
use chrono::{DateTime, Utc};

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Implementação real — usa o relógio do sistema.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Relógio falso para testes — retorna sempre o mesmo instante.
#[cfg(test)]
pub struct FakeClock(pub DateTime<Utc>);

#[cfg(test)]
impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}
