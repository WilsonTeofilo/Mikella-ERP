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
    Migracao {
        versao: "0005_pessoas_endereco_doc",
        sql: include_str!("migrations/0005_pessoas_endereco_doc.sql"),
    },
    Migracao {
        versao: "0006_crediario_formas",
        sql: include_str!("migrations/0006_crediario_formas.sql"),
    },
    Migracao {
        versao: "0007_parcelas_numero",
        sql: include_str!("migrations/0007_parcelas_numero.sql"),
    },
    Migracao {
        versao: "0008_blind_index_cpf",
        sql: include_str!("migrations/0008_blind_index_cpf.sql"),
    },
    Migracao {
        versao: "0009_integridade_parcelas",
        sql: include_str!("migrations/0009_integridade_parcelas.sql"),
    },
    Migracao {
        versao: "0010_pagamentos_e_loja_config",
        sql: include_str!("migrations/0010_pagamentos_e_loja_config.sql"),
    },
    Migracao {
        versao: "0011_lgpd_consentimento",
        sql: include_str!("migrations/0011_lgpd_consentimento.sql"),
    },
    Migracao {
        versao: "0012_config_pagamento",
        sql: include_str!("migrations/0012_config_pagamento.sql"),
    },
];

/// Versão da migração mais recente embutida no binário.
///
/// Testes e diagnóstico usam este valor em vez de repetir o número à mão:
/// adicionar uma migração não deve deixar uma asserção silenciosamente velha.
pub fn versao_mais_recente() -> &'static str {
    match MIGRACOES.last() {
        Some(m) => m.versao,
        None => "0000_sem_migracao",
    }
}

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
                    // DAT-001: a 0004 antiga apagava as tabelas financeiras.
                    // Bancos que já a aplicaram têm exatamente o schema novo
                    // (planos_conta + parcelas.conta_tipo). Nesse caso o
                    // arquivo foi corrigido apenas para instalações futuras;
                    // aceitamos o checksum legado e registramos o atual.
                    if migracao.versao == "0004_financeiro"
                        && schema_financeiro_unificado_existe(conn)
                    {
                        tracing::warn!(
                            versao = migracao.versao,
                            "aceitando checksum legado da migração financeira: schema novo já está presente"
                        );
                        conn.execute(
                            "UPDATE schema_migrations SET checksum = ?1 WHERE versao = ?2",
                            params![checksum_atual, migracao.versao],
                        )?;
                        continue;
                    }
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

/// Reconhece o schema financeiro unificado criado pela 0004.
///
/// Usado apenas no caminho de compatibilidade de checksum: prova que a
/// migração antiga (destrutiva) já rodou antes de aceitar o checksum legado.
fn schema_financeiro_unificado_existe(conn: &Connection) -> bool {
    let tem_planos: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'table' AND name = 'planos_conta'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !tem_planos {
        return false;
    }

    let tem_conta_tipo = conn
        .prepare("PRAGMA table_info(parcelas)")
        .and_then(|mut stmt| {
            let colunas = stmt.query_map([], |row| row.get::<_, String>(1))?;
            for coluna in colunas {
                if coluna? == "conta_tipo" {
                    return Ok(true);
                }
            }
            Ok(false)
        })
        .unwrap_or(false);
    tem_conta_tipo
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
        // Deriva de MIGRACOES: adicionar uma migração não deve exigir editar o
        // teste, mas também não deve deixar a contagem silenciosamente errada.
        assert_eq!(total as usize, MIGRACOES.len());
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
    fn migracao_financeira_preserva_contas_e_parcelas() {
        // Simula um banco parado na 0003 com dados financeiros reais.
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations (
                 versao      TEXT PRIMARY KEY,
                 checksum    TEXT NOT NULL,
                 aplicada_em TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ) STRICT;",
        )
        .unwrap();

        for (versao, sql) in [
            ("0001_initial", include_str!("migrations/0001_initial.sql")),
            (
                "0002_app_preferences",
                include_str!("migrations/0002_app_preferences.sql"),
            ),
            (
                "0003_email_usuario",
                include_str!("migrations/0003_email_usuario.sql"),
            ),
        ] {
            conn.execute_batch(sql).unwrap();
            conn.execute(
                "INSERT INTO schema_migrations (versao, checksum) VALUES (?1, ?2)",
                params![versao, checksum(sql)],
            )
            .unwrap();
        }

        conn.execute(
            "INSERT INTO usuarios (id, nome, username, senha_hash, papel) \
             VALUES ('u1', 'Dono', 'dono', 'hash', 'DONO')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO contas_pagar (id, descricao, total_centavos) \
             VALUES ('cp1', 'Aluguel', 10000)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO parcelas_pagar (id, conta_id, numero, vencimento, valor_centavos, estado) \
             VALUES ('pp1', 'cp1', 1, '2026-01-10', 5000, 'PENDENTE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO parcelas_pagar (id, conta_id, numero, vencimento, valor_centavos, estado) \
             VALUES ('pp2', 'cp1', 2, '2026-02-10', 5000, 'PENDENTE')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO contas_receber (id, descricao, total_centavos) \
             VALUES ('cr1', 'Serviço', 7500)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO parcelas_receber (id, conta_id, numero, vencimento, valor_centavos, estado) \
             VALUES ('pr1', 'cr1', 1, '2026-03-10', 7500, 'RECEBIDO')",
            [],
        )
        .unwrap();

        aplicar(&mut conn).unwrap();

        // Contas preservadas e classificadas como pendentes.
        let contas_pagar: i64 = conn
            .query_row("SELECT COUNT(*) FROM contas_pagar", [], |r| r.get(0))
            .unwrap();
        let contas_receber: i64 = conn
            .query_row("SELECT COUNT(*) FROM contas_receber", [], |r| r.get(0))
            .unwrap();
        assert_eq!((contas_pagar, contas_receber), (1, 1));

        let plano: String = conn
            .query_row(
                "SELECT plano_codigo FROM contas_pagar WHERE id = 'cp1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(plano, "2.1.99");

        // Parcelas preservadas, com status convertido e numeração refeita.
        let parcelas: i64 = conn
            .query_row("SELECT COUNT(*) FROM parcelas", [], |r| r.get(0))
            .unwrap();
        assert_eq!(parcelas, 3);

        let (status, numero): (String, i64) = conn
            .query_row(
                "SELECT status, numero FROM parcelas WHERE id = 'pr1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "PAGA");
        assert_eq!(numero, 1);

        let numeros: Vec<i64> = conn
            .prepare("SELECT numero FROM parcelas WHERE conta_id = 'cp1' ORDER BY vencimento")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(numeros, vec![1, 2]);
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

    #[test]
    fn integridade_estrutural_parcelas_dat_004() {
        let mut conn = Connection::open_in_memory().unwrap();
        aplicar(&mut conn).unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();

        // Usuário e plano de contas
        conn.execute_batch(
            "INSERT INTO usuarios (id, nome, username, senha_hash, papel)
                 VALUES ('u1', 'Dono', 'dono', 'hash', 'DONO');
             INSERT INTO contas_receber (id, descricao, plano_codigo, plano_tipo, usuario_id)
                 VALUES ('cr1', 'Venda Crediário', '1.1.01', 'RECEBIMENTO', 'u1');",
        )
        .unwrap();

        // 1. Inserir parcela com valor zero ou negativo falha
        let erro_zero = conn.execute(
            "INSERT INTO parcelas (id, conta_id, conta_tipo, numero, valor_centavos, vencimento)
             VALUES ('p0', 'cr1', 'RECEBIMENTO', 1, 0, '2026-12-01T00:00:00Z')",
            [],
        ).unwrap_err();
        assert!(
            erro_zero.to_string().contains("DAT-004"),
            "deveria barrar valor 0: {erro_zero}"
        );

        let erro_negativo = conn.execute(
            "INSERT INTO parcelas (id, conta_id, conta_tipo, numero, valor_centavos, vencimento)
             VALUES ('pn', 'cr1', 'RECEBIMENTO', 1, -500, '2026-12-01T00:00:00Z')",
            [],
        ).unwrap_err();
        assert!(
            erro_negativo.to_string().contains("DAT-004"),
            "deveria barrar valor negativo: {erro_negativo}"
        );

        // 2. Inserir parcela com conta_id inexistente falha
        let erro_fk = conn.execute(
            "INSERT INTO parcelas (id, conta_id, conta_tipo, numero, valor_centavos, vencimento)
             VALUES ('pfk', 'conta_fantasma', 'RECEBIMENTO', 1, 1000, '2026-12-01T00:00:00Z')",
            [],
        ).unwrap_err();
        assert!(
            erro_fk.to_string().contains("DAT-004"),
            "deveria barrar conta inexistente: {erro_fk}"
        );

        // 3. Parcela válida é inserida com sucesso
        conn.execute(
            "INSERT INTO parcelas (id, conta_id, conta_tipo, numero, valor_centavos, vencimento)
             VALUES ('pok', 'cr1', 'RECEBIMENTO', 1, 2500, '2026-12-01T00:00:00Z')",
            [],
        )
        .unwrap();

        // 4. Excluir conta que possui parcelas é bloqueado
        let erro_del = conn
            .execute("DELETE FROM contas_receber WHERE id = 'cr1'", [])
            .unwrap_err();
        assert!(
            erro_del.to_string().contains("DAT-004"),
            "deveria impedir delete de conta com parcelas: {erro_del}"
        );
    }
}
