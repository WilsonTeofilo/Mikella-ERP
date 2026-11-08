//! Repositório do lembrete de doação (SQLite).
//!
//! O estado fica em `app_preferences` (`donation_mes`) e a data de ativação em
//! `instance_identity`. Nenhuma decisão de UI mora aqui: apenas leitura e
//! gravação cruas.

use rusqlite::{params, Connection};

use crate::error::InfraResult;

pub struct DoacaoRepo<'a> {
    conn: &'a Connection,
}

impl<'a> DoacaoRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Indica se já houve doação registrada no mês informado (`YYYY-MM`).
    pub fn doado_no_mes(&self, mes: &str) -> InfraResult<bool> {
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM app_preferences
             WHERE chave = 'donation_mes' AND valor = ?1",
            params![mes],
            |row| row.get(0),
        )?;
        Ok(total > 0)
    }

    /// Dias inteiros desde o registro da instância (nunca negativo).
    pub fn dias_desde_ativacao(&self) -> InfraResult<i64> {
        let dias: i64 = self.conn.query_row(
            "SELECT CAST(julianday('now') - julianday(created_at) AS INTEGER)
             FROM instance_identity WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(dias.max(0))
    }

    /// Registra a doação do mês, suprimindo os prompts seguintes.
    pub fn registrar_doacao(&self, mes: &str) -> InfraResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO app_preferences (chave, valor)
             VALUES ('donation_mes', ?1)",
            params![mes],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    #[test]
    fn registro_de_doacao_suprime_o_mes() {
        let db = Database::em_memoria().unwrap();
        let repo = DoacaoRepo::novo(db.conn());

        assert!(!repo.doado_no_mes("2026-11").unwrap());
        repo.registrar_doacao("2026-11").unwrap();
        assert!(repo.doado_no_mes("2026-11").unwrap());
        assert!(!repo.doado_no_mes("2026-12").unwrap());
    }
}
