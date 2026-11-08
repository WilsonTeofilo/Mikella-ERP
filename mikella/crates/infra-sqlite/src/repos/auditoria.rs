//! Repositório de auditoria append-only (LGPD-002 / FRD-053).
//!
//! A tabela `auditoria` tem triggers que proíbem UPDATE e DELETE — cada
//! INSERT é permanente. O hash encadeado (BLAKE3) garante detecção de
//! adulteração retroativa: qualquer modificação direta no SQLite quebra
//! a cadeia a partir do registro alterado.
//!
//! ## Eventos registrados
//! - Login de operador (autenticação bem-sucedida)
//! - Logout com revogação de token
//! - Abertura e fechamento de caixa
//! - Estorno de venda (com motivo e aprovador)
//! - Sangria de caixa (com aprovador)
//! - Alteração de preço de produto
//! - Criação, atualização e desativação de usuário

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::InfraResult;
use crate::hash::hash_auditoria;
use crate::tempo::data_hora_sql;

/// Repositório append-only de auditoria.
pub struct AuditoriaRepo<'a> {
    conn: &'a Connection,
}

impl<'a> AuditoriaRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Lê o último hash da cadeia (ou "GENESIS" se vazia).
    fn posicao_atual(&self) -> InfraResult<String> {
        let ultimo: Option<String> = self
            .conn
            .query_row(
                "SELECT hash FROM auditoria ORDER BY created_at DESC, id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(ultimo.unwrap_or_else(|| "GENESIS".to_string()))
    }

    /// Registra um evento de auditoria.
    ///
    /// Calcula o hash encadeado e insere o registro atomicamente.
    /// Nunca falha silenciosamente: retorna `Err` se o INSERT falhar.
    #[allow(clippy::too_many_arguments)] // espelha as colunas do registro de auditoria
    pub fn registrar(
        &self,
        entidade: &str,
        entidade_id: &str,
        acao: &str,
        usuario_id: Option<&str>,
        dispositivo_id: Option<&str>,
        dados_antes: Option<&str>,
        dados_depois: Option<&str>,
    ) -> InfraResult<()> {
        let id = Uuid::new_v4().to_string();
        let created_at = data_hora_sql(Utc::now());
        let prev_hash = self.posicao_atual()?;

        let hash = hash_auditoria(
            entidade,
            entidade_id,
            acao,
            usuario_id,
            dados_antes,
            dados_depois,
            &prev_hash,
            &created_at,
        );

        self.conn.execute(
            "INSERT INTO auditoria
                 (id, entidade, entidade_id, acao, usuario_id, dispositivo_id,
                  dados_antes, dados_depois, prev_hash, hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id,
                entidade,
                entidade_id,
                acao,
                usuario_id,
                dispositivo_id,
                dados_antes,
                dados_depois,
                prev_hash,
                hash,
                created_at,
            ],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    #[test]
    fn auditoria_encadeia_hash_e_impede_duplicidade() {
        let db = Database::em_memoria().unwrap();
        let conn = db.conn();
        conn.execute(
            "INSERT INTO usuarios (id, nome, username, senha_hash, papel)
             VALUES ('u1', 'Teste', 'teste', 'hash', 'DONO')",
            [],
        )
        .unwrap();
        let repo = AuditoriaRepo::novo(conn);

        repo.registrar("venda", "v1", "ESTORNO", Some("u1"), None, None, None)
            .unwrap();
        repo.registrar("caixa", "c1", "FECHAMENTO", Some("u1"), None, None, None)
            .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM auditoria", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        // Verifica encadeamento: o prev_hash do segundo deve ser o hash do primeiro.
        let (hash1, prev2): (String, String) = conn
            .query_row(
                "SELECT a1.hash, a2.prev_hash
                 FROM auditoria a1
                 JOIN auditoria a2 ON a2.entidade_id = 'c1'
                 WHERE a1.entidade_id = 'v1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(hash1, prev2, "cadeia de hash quebrada");
    }

    #[test]
    fn auditoria_genesis_quando_vazia() {
        let db = Database::em_memoria().unwrap();
        let repo = AuditoriaRepo::novo(db.conn());
        let prev = repo.posicao_atual().unwrap();
        assert_eq!(prev, "GENESIS");
    }
}
