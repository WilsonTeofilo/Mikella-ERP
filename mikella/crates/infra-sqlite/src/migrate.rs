//! Runner de migrações com checksum.
//!
//! Regras:
//!   • Cada migração é um arquivo SQL embutido no binário (`include_str!`).
//!   • O checksum blake3 é gravado em `schema_migrations` na mesma transação
//!     que aplica o SQL. Se o arquivo mudar depois de aplicado, o banco se
//!     recusa a abrir — isso impede "migração editada no calor do momento".
//!   • Tudo roda em transação: ou a migração inteira entra, ou nada entra.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{InfraError, InfraResult};

struct Migracao {
    versao: &'static str,
    sql: &'static str,
}

const MIGRACOES: &[Migracao] = &[
    Migracao {
        versao: "0001_initial",
        sql: include_str!("migrations/0001_initial.sql"),
    },
    Migracao {
        versao: "0002_app_preferences",
        sql: include_str!("migrations/0002_app_preferences.sql"),
    },
    Migracao {
        versao: "0003_email_usuario",
        sql: include_str!("migrations/0003_email_usuario.sql"),
    },
    Migracao {
        versao: "0004_financeiro",
        sql: include_str!("migrations/0004_financeiro.sql"),
    },
];

/// Aplica todas as migrações pendentes. Idempotente.
pub fn aplicar(conn: &mut Connection) -> InfraResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             versao      TEXT PRIMARY KEY,
             checksum    TEXT NOT NULL,
             aplicada_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
         ) STRICT;",
    )?;

    for migracao in MIGRACOES {
        let checksum_atual = checksum(migracao.sql);

        let aplicado: Option<String> = conn
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE versao = ?1",
                params![migracao.versao],
                |row| row.get(0),
            )
            .optional()?;

        match aplicado {
            Some(checksum_aplicado) => {
                if checksum_aplicado != checksum_atual {
                    return Err(InfraError::ChecksumDivergente {
                        versao: migracao.versao.to_string(),
                        aplicado: checksum_aplicado,
                        atual: checksum_atual,
                    });
                }
            }
            None => {
                let tx = conn.transaction()?;
                tx.execute_batch(migracao.sql)
                    .map_err(|erro| InfraError::Migracao {
                        versao: migracao.versao.to_string(),
                        detalhe: erro.to_string(),
                    })?;
                tx.execute(
                    "INSERT INTO schema_migrations (versao, checksum) VALUES (?1, ?2)",
                    params![migracao.versao, checksum_atual],
                )?;
                tx.commit()?;
            }
        }
    }

    Ok(())
}

fn checksum(sql: &str) -> String {
    blake3::hash(sql.as_bytes()).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aplica_uma_vez_e_e_idempotente() {
        let mut conn = Connection::open_in_memory().unwrap();
        aplicar(&mut conn).unwrap();
        aplicar(&mut conn).unwrap();

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 4);
    }

    #[test]
    fn checksum_divergente_falha() {
        let mut conn = Connection::open_in_memory().unwrap();
        aplicar(&mut conn).unwrap();

        conn.execute(
            "UPDATE schema_migrations SET checksum = 'adulterado' WHERE versao = '0001_initial'",
            [],
        )
        .unwrap();

        let erro = aplicar(&mut conn).unwrap_err();
        assert!(matches!(erro, InfraError::ChecksumDivergente { .. }));
    }

    #[test]
    fn todas_as_tabelas_existem() {
        let mut conn = Connection::open_in_memory().unwrap();
        aplicar(&mut conn).unwrap();

        for tabela in [
            "instance_identity",
            "usuarios",
            "produtos",
            "lotes",
            "movimentacoes_estoque",
            "sessoes_caixa",
            "livro_caixa",
            "vendas",
            "venda_itens",
            "venda_pagamentos",
            "operacoes_idempotentes",
            "outbox_eventos",
            "app_preferences",
            "planos_conta",
            "contas_pagar",
            "contas_receber",
            "parcelas",
        ] {
            let existe: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![tabela],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(existe, 1, "tabela {tabela} não foi criada");
        }
    }

    #[test]
    fn triggers_impedem_update_em_movimentacao() {
        let mut conn = Connection::open_in_memory().unwrap();
        aplicar(&mut conn).unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();

        // Dados mínimos para satisfazer as FKs.
        conn.execute_batch(
            "INSERT INTO usuarios (id, nome, username, senha_hash, papel)
                 VALUES ('u1', 'Teste', 'teste', 'hash', 'DONO');
             INSERT INTO produtos (id, nome, preco_centavos)
                 VALUES ('p1', 'Produto', 100);
             INSERT INTO movimentacoes_estoque
                 (id, produto_id, tipo, quantidade, usuario_id)
                 VALUES ('m1', 'p1', 'ENTRADA_COMPRA', 5, 'u1');",
        )
        .unwrap();

        let erro = conn
            .execute(
                "UPDATE movimentacoes_estoque SET quantidade = 99 WHERE id = 'm1'",
                [],
            )
            .unwrap_err();
        assert!(erro.to_string().contains("UPDATE proibido"));

        let erro = conn
            .execute("DELETE FROM movimentacoes_estoque WHERE id = 'm1'", [])
            .unwrap_err();
        assert!(erro.to_string().contains("DELETE proibido"));
    }

    #[test]
    fn uma_sessao_aberta_por_caixa() {
        let mut conn = Connection::open_in_memory().unwrap();
        aplicar(&mut conn).unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();

        conn.execute_batch(
            "INSERT INTO usuarios (id, nome, username, senha_hash, papel)
                 VALUES ('u1', 'Teste', 'teste', 'hash', 'DONO');
             INSERT INTO dispositivos (id, nome, usuario_so, fingerprint, papel)
                 VALUES ('d1', 'PC', 'dono', 'fp', 'CAIXA');
             INSERT INTO sessoes_caixa (id, numero_caixa, dispositivo_id, operador_id)
                 VALUES ('s1', 1, 'd1', 'u1');",
        )
        .unwrap();

        let erro = conn
            .execute(
                "INSERT INTO sessoes_caixa (id, numero_caixa, dispositivo_id, operador_id)
                 VALUES ('s2', 1, 'd1', 'u1')",
                [],
            )
            .unwrap_err();
        assert!(
            erro.to_string().contains("UNIQUE"),
            "erro inesperado: {erro}"
        );
    }
}
