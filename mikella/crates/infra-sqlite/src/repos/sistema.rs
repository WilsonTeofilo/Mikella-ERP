//! Read models de diagnóstico e manutenção do banco (OPS-001 / RES-030).
//!
//! Mantém `PRAGMA`, `VACUUM INTO` e contagens fora dos handlers Axum (ARQ-001).
//! Nenhum método aqui devolve PII: apenas métricas volumétricas e metadados.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::InfraResult;

/// Contagens estruturais apresentadas no pacote de diagnóstico.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContagensDiagnostico {
    pub produtos: i64,
    pub vendas: i64,
    pub movimentacoes_estoque: i64,
    pub entradas_livro_caixa: i64,
    pub registros_auditoria: i64,
}

/// Uma migração aplicada, como aparece no pacote de suporte.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigracaoAplicada {
    pub versao: String,
    pub checksum: String,
    pub aplicada_em: String,
}

pub struct DiagnosticoRepo<'a> {
    conn: &'a Connection,
}

impl<'a> DiagnosticoRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Resultado textual do `PRAGMA integrity_check` (normalmente `ok`).
    pub fn integridade(&self) -> InfraResult<String> {
        let resultado = self
            .conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        Ok(resultado)
    }

    /// Versão do SQLite embutido no binário.
    pub fn sqlite_versao(&self) -> InfraResult<String> {
        let versao = self
            .conn
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))?;
        Ok(versao)
    }

    /// Totais estruturais sem PII.
    pub fn contagens(&self) -> InfraResult<ContagensDiagnostico> {
        let contar = |tabela: &str| -> InfraResult<i64> {
            // A tabela é constante interna, nunca entrada do usuário.
            let sql = format!("SELECT COUNT(*) FROM {tabela}");
            let total = self.conn.query_row(&sql, [], |row| row.get(0))?;
            Ok(total)
        };

        Ok(ContagensDiagnostico {
            produtos: contar("produtos")?,
            vendas: contar("vendas")?,
            movimentacoes_estoque: contar("movimentacoes_estoque")?,
            entradas_livro_caixa: contar("livro_caixa")?,
            registros_auditoria: contar("auditoria")?,
        })
    }

    /// Confere o encadeamento `prev_hash` do livro-caixa (FRD-053).
    ///
    /// Devolve `true` quando cada entrada aponta para o hash da anterior e a
    /// primeira aponta para `GENESIS`. Linhas ilegíveis viram erro observável,
    /// não um `false` silencioso.
    pub fn cadeia_livro_caixa_integra(&self) -> InfraResult<bool> {
        let mut stmt = self.conn.prepare(
            "SELECT prev_hash, hash FROM livro_caixa ORDER BY created_at ASC, rowid ASC",
        )?;
        let mut rows = stmt.query([])?;

        let mut prev = "GENESIS".to_string();
        while let Some(row) = rows.next()? {
            let prev_hash: String = row.get(0)?;
            let hash: String = row.get(1)?;
            if prev_hash != prev {
                return Ok(false);
            }
            prev = hash;
        }
        Ok(true)
    }

    /// Confere o encadeamento `prev_hash` da trilha de auditoria append-only (LGPD-002 / FRD-053).
    pub fn cadeia_auditoria_integra(&self) -> InfraResult<bool> {
        let mut stmt = self
            .conn
            .prepare("SELECT prev_hash, hash FROM auditoria ORDER BY created_at ASC, rowid ASC")?;
        let mut rows = stmt.query([])?;

        let mut prev = "GENESIS".to_string();
        while let Some(row) = rows.next()? {
            let prev_hash: String = row.get(0)?;
            let hash: String = row.get(1)?;
            if prev_hash != prev {
                return Ok(false);
            }
            prev = hash;
        }
        Ok(true)
    }

    /// Gera uma cópia consistente do banco no caminho indicado.
    ///
    /// `VACUUM INTO` é parametrizado: o caminho nunca entra na string SQL.
    pub fn backup_vacuum_into(&self, destino: &str) -> InfraResult<()> {
        self.conn.execute("VACUUM INTO ?1", [destino])?;
        Ok(())
    }

    /// Lê uma preferência da tabela `loja_config` (criada pela migração 0010).
    /// Retorna `Ok(None)` se a chave não existir.
    pub fn loja_config(&self, chave: &str) -> InfraResult<Option<String>> {
        super::config_loja::ConfigLojaRepo::novo(self.conn).obter(chave)
    }

    /// Lê o fuso horário IANA configurado para a loja (default: `America/Sao_Paulo`).
    pub fn fuso_horario(&self) -> String {
        super::config_loja::ConfigLojaRepo::novo(self.conn).fuso_horario()
    }

    /// Lista as migrações já aplicadas, em ordem cronológica.
    ///
    /// Usado pelo pacote de suporte para provar em qual schema o banco estava.
    /// Não inclui dados de negócio nem PII.
    pub fn migracoes(&self) -> InfraResult<Vec<MigracaoAplicada>> {
        let mut stmt = self.conn.prepare(
            "SELECT versao, checksum, aplicada_em
               FROM schema_migrations
              ORDER BY versao ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(MigracaoAplicada {
                versao: row.get(0)?,
                checksum: row.get(1)?,
                aplicada_em: row.get(2)?,
            })
        })?;
        let mut migracoes = Vec::new();
        for row in rows {
            migracoes.push(row?);
        }
        Ok(migracoes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    #[test]
    fn diagnostico_le_metricas_do_schema_real() {
        let db = Database::em_memoria().unwrap();
        let repo = DiagnosticoRepo::novo(db.conn());

        assert_eq!(repo.integridade().unwrap(), "ok");
        assert!(!repo.sqlite_versao().unwrap().is_empty());
        assert_eq!(repo.contagens().unwrap(), ContagensDiagnostico::default());
        assert!(repo.cadeia_livro_caixa_integra().unwrap());
        assert!(repo.cadeia_auditoria_integra().unwrap());

        let migracoes = repo.migracoes().unwrap();
        assert!(!migracoes.is_empty());
        assert!(migracoes.iter().any(|m| m.versao == "0001_initial"));
        assert!(migracoes.iter().all(|m| !m.checksum.is_empty()));
        assert_eq!(repo.fuso_horario(), "America/Sao_Paulo");
    }
}
