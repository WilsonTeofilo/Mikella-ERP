//! Repositório de identidade da instância da loja (SQLite).

use rusqlite::{params, Connection};

use crate::error::InfraResult;

/// Repositório para consulta e gravação do pareamento da instância da loja.
pub struct InstanceRepo<'a> {
    pub conn: &'a Connection,
}

impl<'a> InstanceRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Verifica se a loja já passou pelo processo inicial de onboarding.
    pub fn esta_onboarded(&self) -> InfraResult<bool> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM instance_identity WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count > 0)
    }

    /// Registra a identidade da instância da loja no banco.
    pub fn registrar(&self, account_id: &str, store_id: &str) -> InfraResult<()> {
        self.conn.execute(
            "INSERT INTO instance_identity (id, account_id, store_id, db_epoch, manifest_json)
             VALUES (1, ?1, ?2, 1, '{}')",
            params![account_id, store_id],
        )?;
        Ok(())
    }
}
