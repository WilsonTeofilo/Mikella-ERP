//! Retenção e expurgo de dados pessoais (LGPD / F8).
//!
//! A regra de negócio é conservadora de propósito:
//! - **Nada** é apagado de `livro_caixa`, `auditoria` ou `vendas`: são registros
//!   fiscais/contábeis protegidos por trigger e pela obrigação legal de guarda.
//! - Clientes sem movimentação há mais de `prazo_anos` têm os **dados pessoais
//!   anonimizados**, preservando o `id` para integridade referencial.
//! - Cliente com crediário em aberto ou movimentação dentro do prazo nunca é
//!   tocado.
//!
//! O fluxo tem dois passos obrigatórios: `aplicar = false` (simulação, padrão)
//! mostra quantos seriam anonimizados; só depois o DONO pede `aplicar = true`.
//! A revisão jurídica do prazo e do contrato loja × Mikella continua pendente
//! (ver `docs/LGPD-RETENCAO.md`).

use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::{erro_campo, InfraResult};
use crate::tempo::{hoje_local, intervalo_utc_do_dia};

/// Resultado de uma simulação ou execução do expurgo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatorioExpurgoLgpd {
    pub prazo_anos: u32,
    /// Data local de corte (`AAAA-MM-DD`) no fuso da loja.
    pub data_corte: String,
    /// `simulacao` ou `aplicado`.
    pub modo: String,
    pub clientes_elegiveis: i64,
    pub clientes_anonimizados: i64,
    pub observacao: String,
}

/// Calcula o início do dia local `anos` atrás, em UTC e em data local.
fn janela_de_corte(fuso: &str, anos: u32) -> InfraResult<(String, String)> {
    let hoje = hoje_local(fuso);
    let ano = i32::try_from(anos).map_err(|_| erro_campo("anos", "valor grande demais"))?;
    let corte = NaiveDate::from_ymd_opt(hoje.year() - ano, hoje.month(), hoje.day())
        // 29 de fevereiro sem ano bissexto: usa o último dia do mês.
        .or_else(|| NaiveDate::from_ymd_opt(hoje.year() - ano, hoje.month(), 28))
        .ok_or_else(|| erro_campo("anos", "data de corte inválida"))?;
    let corte_data = corte.to_string();
    let (corte_utc, _) = intervalo_utc_do_dia(fuso, &corte_data)?;
    Ok((corte_data, corte_utc))
}

fn clientes_elegiveis(
    conn: &Connection,
    corte_data: &str,
    corte_utc: &str,
) -> InfraResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT p.id
           FROM pessoas p
          WHERE p.tipo IN ('CLIENTE', 'AMBOS')
            AND p.nome <> '[CLIENTE ANONIMIZADO]'
            AND p.updated_at < ?1
            AND NOT EXISTS (
                SELECT 1 FROM contas_receber cr
                 WHERE cr.pessoa_id = p.id
                   AND cr.created_at >= ?1
            )
            AND NOT EXISTS (
                SELECT 1
                  FROM parcelas pa
                  JOIN contas_receber cr ON cr.id = pa.conta_id
                 WHERE cr.pessoa_id = p.id
                   AND pa.conta_tipo = 'RECEBIMENTO'
                   AND (
                        pa.status = 'ABERTA'
                        OR pa.vencimento >= ?2
                        OR (pa.pago_em IS NOT NULL AND pa.pago_em >= ?1)
                   )
            )
          ORDER BY p.id",
    )?;
    let rows = stmt.query_map(params![corte_utc, corte_data], |row| {
        row.get::<_, String>(0)
    })?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row?);
    }
    Ok(ids)
}

/// Anonimiza clientes sem movimentação dentro do prazo legal.
///
/// `aplicar = false` apenas conta. `aplicar = true` grava a anonimização e
/// devolve quantas linhas foram alteradas. Rode dentro de uma transação quando
/// for aplicar, para que o relatório e a auditoria entrem juntos.
pub fn expurgar_clientes_por_retencao(
    conn: &Connection,
    fuso: &str,
    prazo_anos: u32,
    aplicar: bool,
) -> InfraResult<RelatorioExpurgoLgpd> {
    if prazo_anos < 5 {
        return Err(erro_campo(
            "anos",
            "a retenção mínima fiscal é de 5 anos; prazos menores exigem revisão jurídica",
        ));
    }

    let (corte_data, corte_utc) = janela_de_corte(fuso, prazo_anos)?;
    let ids = clientes_elegiveis(conn, &corte_data, &corte_utc)?;
    let elegiveis = i64::try_from(ids.len()).unwrap_or(i64::MAX);

    let mut anonimizados = 0i64;
    if aplicar {
        for id in &ids {
            let linhas = conn.execute(
                "UPDATE pessoas
                    SET nome = '[CLIENTE ANONIMIZADO]',
                        documento = NULL,
                        documento_blind = 'ANON_' || id,
                        telefone = NULL,
                        email = NULL,
                        endereco = NULL,
                        ativo = 0,
                        consentimento_marketing = 0,
                        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                  WHERE id = ?1",
                params![id],
            )?;
            if linhas > 0 {
                anonimizados += i64::try_from(linhas).unwrap_or(1);
            }
        }
    }

    Ok(RelatorioExpurgoLgpd {
        prazo_anos,
        data_corte: corte_data,
        modo: if aplicar { "aplicado" } else { "simulacao" }.to_string(),
        clientes_elegiveis: elegiveis,
        clientes_anonimizados: anonimizados,
        observacao: if aplicar {
            "Dados pessoais anonimizados; registros fiscais de vendas, livro-caixa e auditoria preservados."
                .to_string()
        } else {
            "Simulação: nenhum dado foi alterado. Revise a lista e repita com aplicar=true."
                .to_string()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    fn inserir_cliente(conn: &Connection, id: &str, atualizado_em: &str) {
        conn.execute(
            "INSERT INTO pessoas (id, tipo, nome, documento, documento_blind, telefone, email, endereco, ativo, created_at, updated_at)
             VALUES (?1, 'CLIENTE', 'Cliente Teste', 'ENC:CPF', ?2, 'ENC:TEL', 'ENC:EMAIL', 'ENC:ENDERECO', 1, ?3, ?3)",
            params![id, format!("BLIND_{id}"), atualizado_em],
        )
        .unwrap();
    }

    #[test]
    fn simulacao_nao_altera_e_aplicacao_anonimiza() {
        let db = Database::em_memoria().unwrap();
        inserir_cliente(db.conn(), "antigo", "2015-01-01T00:00:00.000Z");
        inserir_cliente(db.conn(), "recente", "2026-01-01T00:00:00.000Z");

        let simulado =
            expurgar_clientes_por_retencao(db.conn(), "America/Sao_Paulo", 5, false).unwrap();
        assert_eq!(simulado.clientes_elegiveis, 1);
        assert_eq!(simulado.clientes_anonimizados, 0);

        let nome_antigo: String = db
            .conn()
            .query_row("SELECT nome FROM pessoas WHERE id = 'antigo'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(nome_antigo, "Cliente Teste");

        let aplicado =
            expurgar_clientes_por_retencao(db.conn(), "America/Sao_Paulo", 5, true).unwrap();
        assert_eq!(aplicado.clientes_anonimizados, 1);

        let (nome, doc, blind, tel): (String, Option<String>, String, Option<String>) = db
            .conn()
            .query_row(
                "SELECT nome, documento, documento_blind, telefone FROM pessoas WHERE id = 'antigo'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(nome, "[CLIENTE ANONIMIZADO]");
        assert!(
            doc.is_none(),
            "o documento deve ser apagado, não virar sentinela"
        );
        assert_eq!(blind, "ANON_antigo");
        assert!(tel.is_none());

        // O recente continua intacto.
        let nome_recente: String = db
            .conn()
            .query_row("SELECT nome FROM pessoas WHERE id = 'recente'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(nome_recente, "Cliente Teste");
    }

    #[test]
    fn cliente_com_crediario_aberto_nunca_e_anonimizado() {
        let db = Database::em_memoria().unwrap();
        inserir_cliente(db.conn(), "devedor", "2015-01-01T00:00:00.000Z");
        db.conn()
            .execute(
                "INSERT INTO contas_receber (id, descricao, plano_codigo, plano_tipo, usuario_id, pessoa_id, created_at, updated_at)
                 SELECT 'conta1', 'Crediário antigo', '3.1.01', 'RECEBIMENTO', u.id, 'devedor', '2015-01-01T00:00:00.000Z', '2015-01-01T00:00:00.000Z'
                   FROM usuarios u LIMIT 1",
                [],
            )
            .unwrap();
        // Sem usuário no banco, insere um mínimo para a FK e repete.
        let sem_conta: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM contas_receber", [], |r| r.get(0))
            .unwrap();
        if sem_conta == 0 {
            db.conn()
                .execute(
                    "INSERT INTO usuarios (id, nome, username, senha_hash, papel)
                     VALUES ('u1', 'Dono', 'dono', 'hash', 'DONO')",
                    [],
                )
                .unwrap();
            db.conn()
                .execute(
                    "INSERT INTO contas_receber (id, descricao, plano_codigo, plano_tipo, usuario_id, pessoa_id, created_at, updated_at)
                     VALUES ('conta1', 'Crediário antigo', '3.1.01', 'RECEBIMENTO', 'u1', 'devedor', '2015-01-01T00:00:00.000Z', '2015-01-01T00:00:00.000Z')",
                    [],
                )
                .unwrap();
        }
        db.conn()
            .execute(
                "INSERT INTO parcelas (id, conta_id, conta_tipo, valor_centavos, vencimento, status, created_at, updated_at)
                 VALUES ('parc1', 'conta1', 'RECEBIMENTO', 1000, '2015-02-01', 'ABERTA', '2015-01-01T00:00:00.000Z', '2015-01-01T00:00:00.000Z')",
                [],
            )
            .unwrap();

        let relatorio =
            expurgar_clientes_por_retencao(db.conn(), "America/Sao_Paulo", 5, true).unwrap();
        assert_eq!(relatorio.clientes_elegiveis, 0);
        assert_eq!(relatorio.clientes_anonimizados, 0);

        let nome: String = db
            .conn()
            .query_row("SELECT nome FROM pessoas WHERE id = 'devedor'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(nome, "Cliente Teste");
    }

    #[test]
    fn prazo_menor_que_cinco_anos_e_recusado() {
        let db = Database::em_memoria().unwrap();
        assert!(expurgar_clientes_por_retencao(db.conn(), "America/Sao_Paulo", 4, false).is_err());
    }
}
