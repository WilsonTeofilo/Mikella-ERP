//! Resolução de dispositivo.
//!
//! Funções auxiliares que operam sobre o `Database` para garantir registros
//! de FK. O usuário autenticado vem do middleware (`Ator`), nunca de lookup
//! por username em handler.

use mikella_infra_sqlite::Database;
use mikella_infra_sqlite::InfraResult;

/// Garante que o dispositivo referenciado por `dispositivo_id` existe na
/// tabela `dispositivos`. MVP-1: auto-provisioning simples — em MVP-2,
/// a tabela é populada pelo pareamento TLS.
pub fn garantir_dispositivo(db: &Database, dispositivo_id: &str) -> InfraResult<()> {
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM dispositivos WHERE id = ?1",
            [dispositivo_id],
            |row| row.get(0),
        )
        .map_err(mikella_infra_sqlite::InfraError::from)?;

    if count == 0 {
        // Nunca fatiar string por bytes sem checar fronteira: um ID curto ou
        // com acento faria `[..8]` entrar em pânico.
        let sufixo: String = dispositivo_id.chars().take(8).collect();
        db.conn()
            .execute(
                "INSERT INTO dispositivos (id, nome, usuario_so, fingerprint, papel) \
                 VALUES (?1, ?, ?, ?, 'CAIXA')",
                rusqlite::params![
                    dispositivo_id,
                    format!("mikella-caixa-{sufixo}"),
                    "local",
                    "auto-provisionado-mvp1",
                ],
            )
            .map_err(mikella_infra_sqlite::InfraError::from)?;
        tracing::info!(dispositivo_id = %dispositivo_id, "dispositivo auto-provisionado (MVP-1)");
    }
    Ok(())
}
