/// Paginação por keyset cursor — nunca OFFSET (PERF-025).
///
/// Evita carregar tabelas inteiras na memória.
/// O cursor é o `id` (ou `created_at` + `id`) do último item visto.
use serde::{Deserialize, Serialize};

/// Parâmetros de paginação enviados pelo cliente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageParams {
    /// Cursor do último item visto. `None` = primeira página.
    pub cursor: Option<String>,
    /// Máximo de itens por página. Limitado pelo servidor (ex.: máx 100).
    pub limit: u32,
}

impl Default for PageParams {
    fn default() -> Self {
        Self {
            cursor: None,
            limit: 50,
        }
    }
}

impl PageParams {
    pub fn limit_clamped(&self, max: u32) -> u32 {
        self.limit.min(max)
    }
}

/// Resposta paginada.
#[derive(Debug, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// Cursor para a próxima página. `None` = última página.
    pub next_cursor: Option<String>,
    /// Total aproximado (opcional — caro para calcular sempre).
    pub total_hint: Option<u64>,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, next_cursor: Option<String>) -> Self {
        Self {
            items,
            next_cursor,
            total_hint: None,
        }
    }

    pub fn empty() -> Self {
        Self {
            items: vec![],
            next_cursor: None,
            total_hint: Some(0),
        }
    }
}
