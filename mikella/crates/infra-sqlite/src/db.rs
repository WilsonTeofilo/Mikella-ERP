//! Conexão SQLite única (single-writer) com PRAGMAs e migrações.
//!
//! O banco da loja é local e tem **um único escritor**: o processo servidor.
//! Por isso a conexão não é compartilhada por vários threads ao mesmo tempo;
//! a camada HTTP serializa o acesso com um `Mutex`. Isso evita `SQLITE_BUSY`
//! e mantém as transações curtas e previsíveis.

use std::path::Path;

use rusqlite::{Connection, Transaction};

use crate::error::InfraResult;
use crate::migrate;

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Abre (ou cria) o banco em `caminho`, aplica os PRAGMAs e roda migrações.
    pub fn abrir(caminho: impl AsRef<Path>) -> InfraResult<Self> {
        let caminho = caminho.as_ref();
        if let Some(pai) = caminho.parent() {
            if !pai.as_os_str().is_empty() {
                std::fs::create_dir_all(pai)?;
            }
        }

        let conn = Connection::open(caminho)?;
        let mut db = Self { conn };
        db.configurar()?;
        migrate::aplicar(&mut db.conn)?;
        Ok(db)
    }

    /// Banco efêmero em memória — usado em testes e no primeiro boot do app.
    pub fn em_memoria() -> InfraResult<Self> {
        let conn = Connection::open_in_memory()?;
        let mut db = Self { conn };
        db.configurar()?;
        migrate::aplicar(&mut db.conn)?;
        Ok(db)
    }

    fn configurar(&mut self) -> InfraResult<()> {
        // WAL: leitores não bloqueiam o escritor; sobrevive a queda de energia
        // com `synchronous = FULL`. Em memória o SQLite devolve "memory".
        let modo: String = self
            .conn
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        tracing::debug!(journal_mode = %modo, "SQLite configurado");

        self.conn.pragma_update(None, "synchronous", "FULL")?;
        self.conn.pragma_update(None, "foreign_keys", true)?;
        self.conn.pragma_update(None, "busy_timeout", 5_000)?;
        self.conn.pragma_update(None, "temp_store", "MEMORY")?;
        // Impede que funções perigosas sejam usadas em schema/trigger.
        self.conn.pragma_update(None, "trusted_schema", false)?;
        // Zera páginas liberadas (dados financeiros não ficam no disco solto).
        self.conn.pragma_update(None, "secure_delete", true)?;
        Ok(())
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Abre uma transação de escrita. Use para TODA operação que grava mais
    /// de uma linha (venda, sangria, fechamento).
    pub fn transacao(&mut self) -> InfraResult<Transaction<'_>> {
        Ok(self.conn.transaction()?)
    }

    /// `PRAGMA integrity_check` + `foreign_key_check`.
    pub fn verificar_integridade(&self) -> InfraResult<()> {
        let resultado: String = self
            .conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if resultado != "ok" {
            return Err(crate::error::InfraError::Integridade(resultado));
        }

        let fks_quebradas: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get(0)
                })?;
        if fks_quebradas > 0 {
            return Err(crate::error::InfraError::Integridade(format!(
                "{fks_quebradas} violação(ões) de chave estrangeira"
            )));
        }
        Ok(())
    }

    /// Versão da migração mais recente aplicada.
    /// Ordena por `versao` (string lexicográfica "0001" < "0002") em vez de
    /// `aplicada_em`, pois duas migrações rápidas podem ter o mesmo timestamp.
    pub fn versao_schema(&self) -> InfraResult<String> {
        let versao: String = self.conn.query_row(
            "SELECT versao FROM schema_migrations ORDER BY versao DESC LIMIT 1",
            [],
            |row| row.get(0),
        )?;
        Ok(versao)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn abre_em_memoria_com_integridade() {
        let db = Database::em_memoria().unwrap();
        db.verificar_integridade().unwrap();
        assert_eq!(
            db.versao_schema().unwrap(),
            crate::migrate::versao_mais_recente()
        );
    }

    #[test]
    fn foreign_keys_ligadas() {
        let db = Database::em_memoria().unwrap();
        let ativo: i64 = db
            .conn()
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ativo, 1);
    }

    #[test]
    fn transacao_rollback_em_erro() {
        let mut db = Database::em_memoria().unwrap();
        {
            let tx = db.transacao().unwrap();
            tx.execute(
                "INSERT INTO usuarios (id, nome, username, senha_hash, papel)
                 VALUES ('u1', 'Teste', 'teste', 'hash', 'DONO')",
                [],
            )
            .unwrap();
            // Sem commit: Transaction faz rollback ao cair no drop.
        }

        let total: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM usuarios", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 0);
    }

    #[test]
    fn banco_em_arquivo_persiste() {
        let dir = std::env::temp_dir().join(format!("mikella-teste-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("loja.db");

        {
            let mut db = Database::abrir(&caminho).unwrap();
            let tx = db.transacao().unwrap();
            tx.execute(
                "INSERT INTO usuarios (id, nome, username, senha_hash, papel)
                 VALUES ('u1', 'Dono', 'dono', 'hash', 'DONO')",
                [],
            )
            .unwrap();
            tx.commit().unwrap();
        }

        let db = Database::abrir(&caminho).unwrap();
        let total: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM usuarios", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 1);

        drop(db);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn modo_wal_com_synchronous_full_esta_ativo() {
        let dir = std::env::temp_dir().join(format!("mikella-teste-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("loja.db");
        let db = Database::abrir(&caminho).unwrap();

        let journal: String = db
            .conn()
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        let sync: i64 = db
            .conn()
            .query_row("PRAGMA synchronous", [], |r| r.get(0))
            .unwrap();

        assert_eq!(journal.to_ascii_lowercase(), "wal");
        // 2 = FULL: o commit só retorna depois de gravar no disco.
        assert_eq!(sync, 2);

        drop(db);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn imagem_de_queda_descarta_transacao_nao_commitada() {
        // Simula o estado dos arquivos no instante de uma queda de energia:
        // uma transação commited (u1) e outra em andamento (u2) convivem no
        // WAL. Ao reabrir, o SQLite precisa manter u1 e descartar u2.
        let dir = std::env::temp_dir().join(format!("mikella-teste-{}", uuid::Uuid::new_v4()));
        let origens = dir.join("origem");
        let snapshot = dir.join("snapshot");
        std::fs::create_dir_all(&origens).unwrap();
        std::fs::create_dir_all(&snapshot).unwrap();

        let caminho = origens.join("loja.db");
        let db = Database::abrir(&caminho).unwrap();
        db.conn()
            .execute(
                "INSERT INTO usuarios (id, nome, username, senha_hash, papel)
                 VALUES ('u1', 'Commitado', 'u1', 'hash', 'DONO')",
                [],
            )
            .unwrap();

        {
            let conn = rusqlite::Connection::open(&caminho).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            conn.pragma_update(None, "synchronous", "FULL").unwrap();
            conn.execute("BEGIN IMMEDIATE", []).unwrap();
            conn.execute(
                "INSERT INTO usuarios (id, nome, username, senha_hash, papel)
                 VALUES ('u2', 'NaoCommitado', 'u2', 'hash', 'DONO')",
                [],
            )
            .unwrap();

            // Copia o "disco" sem passar pelo fechamento limpo da conexão.
            // O `-shm` é descartável; o SQLite o reconstrói na abertura.
            for sufixo in ["", "-wal"] {
                let de = PathBuf::from(format!("{}{sufixo}", caminho.display()));
                if de.exists() {
                    std::fs::copy(&de, snapshot.join(format!("loja.db{sufixo}"))).unwrap();
                }
            }

            // Deixa a transação aberta de propósito; o snapshot acima é o que
            // um desligamento abrupto teria deixado no disco.
            conn.execute("ROLLBACK", []).unwrap();
        }
        drop(db);

        let recuperado = Database::abrir(snapshot.join("loja.db")).unwrap();
        let total: i64 = recuperado
            .conn()
            .query_row("SELECT COUNT(*) FROM usuarios", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 1, "só a transação commitada sobrevive à queda");

        let existe_u1: i64 = recuperado
            .conn()
            .query_row("SELECT COUNT(*) FROM usuarios WHERE id = 'u1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(existe_u1, 1);
        recuperado.verificar_integridade().unwrap();

        drop(recuperado);
        std::fs::remove_dir_all(&dir).ok();
    }
}
