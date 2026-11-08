//! Repositório de contas a pagar / receber (RF-030).
//!
//! Regras de robustez:
//! - Nenhum UUID, data ou status inválido vira valor "default" silencioso.
//!   Dados corrompidos geram `DomainError::Infraestrutura` e aparecem no log.
//! - `planos_conta` é upsertado antes da conta, e `plano_tipo` é sempre
//!   gravado (a tabela tem NOT NULL + FK composta).
//! - O nome do plano é lido por JOIN, não reconstruído como "Plano {codigo}".
//! - As transações são abertas pelo handler; aqui só executamos comandos.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

use mikella_domain_finance::{
    conta::{ContaPagar, ContaReceber, Parcela, PlanoConta, StatusParcela},
    ports::{ContaPagarRepository, ContaReceberRepository, ParcelaRepository},
};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{ContaPagarId, ContaReceberId, ParcelaId, UsuarioId},
    money::Money,
};

fn err(e: rusqlite::Error) -> DomainError {
    DomainError::Infraestrutura(e.to_string())
}

fn infixo(campo: &str, valor: &str) -> DomainError {
    DomainError::Infraestrutura(format!("valor inválido em {campo}: {valor}"))
}

fn parse_data(s: &str) -> DomainResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| infixo("data", s))
}

fn parse_id<T: std::str::FromStr>(s: &str, campo: &str) -> DomainResult<T> {
    s.parse::<T>()
        .map_err(|_| infixo(campo, s))
}

fn invalid(idx: usize, e: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e))
}

/// Converte uma linha de `parcelas` em domínio, sem fallback silencioso.
fn parse_parcela(row: &Row) -> rusqlite::Result<Parcela> {
    let id_s: String = row.get(0)?;
    let valor: i64 = row.get(1)?;
    let venc_s: String = row.get(2)?;
    let pago_opt: Option<String> = row.get(3)?;
    let status_s: String = row.get(4)?;

    let id = id_s.parse::<ParcelaId>().map_err(|e| invalid(0, e))?;
    let vencimento =
        DateTime::parse_from_rfc3339(&venc_s).map_err(|e| invalid(2, e))?.with_timezone(&Utc);
    let pago_em = pago_opt
        .as_deref()
        .map(|s| {
            DateTime::parse_from_rfc3339(s)
                .map(|d| d.with_timezone(&Utc))
                .map_err(|e| invalid(3, e))
        })
        .transpose()?;
    let status = StatusParcela::from_db(&status_s).map_err(|e| invalid(4, e))?;

    Ok(Parcela {
        id,
        valor: Money::from_centavos(valor),
        vencimento,
        pago_em,
        status,
    })
}

fn upsert_plano(
    conn: &Connection,
    plano: &PlanoConta,
    tipo: &str,
) -> DomainResult<()> {
    conn.execute(
        "INSERT INTO planos_conta (codigo, nome, tipo) VALUES (?1, ?2, ?3)
         ON CONFLICT(codigo, tipo) DO UPDATE SET nome = excluded.nome",
        params![&plano.codigo, &plano.nome, tipo],
    )
    .map_err(err)?;
    Ok(())
}

// --- ContaPagarRepo ---

pub struct ContaPagarRepo<'a> {
    conn: &'a Connection,
}

impl<'a> ContaPagarRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn carregar_parcelas(&self, conta_id: &str) -> DomainResult<Vec<Parcela>> {
        let q = "SELECT id, valor_centavos, vencimento, pago_em, status FROM parcelas WHERE conta_id = ?1 AND conta_tipo = 'PAGAMENTO' ORDER BY vencimento";
        let mut stmt = self.conn.prepare(q).map_err(err)?;
        let rows = stmt
            .query_map(params![conta_id], parse_parcela)
            .map_err(err)?;
        rows.into_iter().map(|r| r.map_err(err)).collect()
    }

    fn linha_para_conta(
        &self,
        row: &Row<'_>,
    ) -> DomainResult<ContaPagar> {
        let id_s: String = row.get(0).map_err(err)?;
        let descricao: String = row.get(1).map_err(err)?;
        let plano_codigo: String = row.get(2).map_err(err)?;
        let plano_nome: String = row.get(3).map_err(err)?;
        let uid_s: String = row.get(4).map_err(err)?;
        let created_s: String = row.get(5).map_err(err)?;

        Ok(ContaPagar {
            id: parse_id::<ContaPagarId>(&id_s, "conta_pagar.id")?,
            descricao,
            plano_conta: PlanoConta {
                codigo: plano_codigo,
                nome: plano_nome,
            },
            parcelas: self.carregar_parcelas(&id_s)?,
            criada_em: parse_data(&created_s)?,
            criada_por: parse_id::<UsuarioId>(&uid_s, "conta_pagar.usuario_id")?,
        })
    }
}

impl ContaPagarRepository for ContaPagarRepo<'_> {
    fn salvar(&self, conta: &ContaPagar) -> DomainResult<()> {
        upsert_plano(self.conn, &conta.plano_conta, "PAGAMENTO")?;

        self.conn
            .execute(
                "INSERT INTO contas_pagar (id, descricao, plano_codigo, plano_tipo, usuario_id)
                 VALUES (?1, ?2, ?3, 'PAGAMENTO', ?4)",
                params![
                    conta.id.to_string(),
                    &conta.descricao,
                    &conta.plano_conta.codigo,
                    conta.criada_por.to_string(),
                ],
            )
            .map_err(err)?;

        for (indice, p) in conta.parcelas.iter().enumerate() {
            self.conn
                .execute(
                    "INSERT INTO parcelas (id, conta_id, conta_tipo, numero, valor_centavos, vencimento, pago_em, status)
                     VALUES (?1, ?2, 'PAGAMENTO', ?3, ?4, ?5, ?6, ?7)",
                    params![
                        p.id.to_string(),
                        conta.id.to_string(),
                        (indice + 1) as i64,
                        p.valor.centavos(),
                        p.vencimento.to_rfc3339(),
                        p.pago_em.as_ref().map(|d| d.to_rfc3339()),
                        p.status.as_str(),
                    ],
                )
                .map_err(err)?;
        }
        Ok(())
    }

    fn atualizar(&self, conta: &ContaPagar) -> DomainResult<()> {
        upsert_plano(self.conn, &conta.plano_conta, "PAGAMENTO")?;

        let n = self
            .conn
            .execute(
                "UPDATE contas_pagar
                 SET descricao = ?2, plano_codigo = ?3, plano_tipo = 'PAGAMENTO'
                 WHERE id = ?1",
                params![
                    conta.id.to_string(),
                    &conta.descricao,
                    &conta.plano_conta.codigo,
                ],
            )
            .map_err(err)?;
        if n == 0 {
            return Err(DomainError::NaoEncontrado {
                entidade: "ContaPagar",
                id: conta.id.to_string(),
            });
        }
        for p in &conta.parcelas {
            let n = self
                .conn
                .execute(
                    "UPDATE parcelas SET pago_em = ?3, status = ?4
                     WHERE id = ?1 AND conta_id = ?2",
                    params![
                        p.id.to_string(),
                        conta.id.to_string(),
                        p.pago_em.as_ref().map(|d| d.to_rfc3339()),
                        p.status.as_str(),
                    ],
                )
                .map_err(err)?;
            if n == 0 {
                return Err(DomainError::NaoEncontrado {
                    entidade: "Parcela",
                    id: p.id.to_string(),
                });
            }
        }
        Ok(())
    }

    fn buscar_por_id(&self, id: ContaPagarId) -> DomainResult<Option<ContaPagar>> {
        let q = "SELECT c.id, c.descricao, c.plano_codigo, p.nome, c.usuario_id, c.created_at
                 FROM contas_pagar c
                 JOIN planos_conta p ON p.codigo = c.plano_codigo AND p.tipo = 'PAGAMENTO'
                 WHERE c.id = ?1";
        let mut stmt = self.conn.prepare(q).map_err(err)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(err)?;
        match rows.next().map_err(err)? {
            Some(row) => Ok(Some(self.linha_para_conta(row)?)),
            None => Ok(None),
        }
    }

    fn buscar_abertas(&self) -> DomainResult<Vec<ContaPagar>> {
        let q = "SELECT c.id, c.descricao, c.plano_codigo, p.nome, c.usuario_id, c.created_at
                 FROM contas_pagar c
                 JOIN planos_conta p ON p.codigo = c.plano_codigo AND p.tipo = 'PAGAMENTO'
                 WHERE EXISTS (
                     SELECT 1 FROM parcelas pa
                     WHERE pa.conta_id = c.id AND pa.conta_tipo = 'PAGAMENTO' AND pa.status <> 'PAGA'
                 )
                 ORDER BY c.created_at";
        let mut stmt = self.conn.prepare(q).map_err(err)?;
        let mut rows = stmt.query([]).map_err(err)?;
        let mut v = Vec::new();
        while let Some(row) = rows.next().map_err(err)? {
            v.push(self.linha_para_conta(row)?);
        }
        Ok(v)
    }

    fn listar_planos(&self) -> DomainResult<Vec<PlanoConta>> {
        let q = "SELECT codigo, nome FROM planos_conta WHERE tipo = 'PAGAMENTO' ORDER BY codigo";
        let mut stmt = self.conn.prepare(q).map_err(err)?;
        let mut rows = stmt.query([]).map_err(err)?;
        let mut v = Vec::new();
        while let Some(row) = rows.next().map_err(err)? {
            v.push(PlanoConta {
                codigo: row.get(0).map_err(err)?,
                nome: row.get(1).map_err(err)?,
            });
        }
        Ok(v)
    }

    fn cancelar_abertas_por_referencia(&self, _referencia_id: &str) -> DomainResult<u32> {
        // Para contas a pagar, não temos referência a venda, então retornamos 0
        Ok(0)
    }
}

// --- ContaReceberRepo ---

pub struct ContaReceberRepo<'a> {
    conn: &'a Connection,
}

impl<'a> ContaReceberRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn carregar_parcelas(&self, conta_id: &str) -> DomainResult<Vec<Parcela>> {
        let q = "SELECT id, valor_centavos, vencimento, pago_em, status FROM parcelas WHERE conta_id = ?1 AND conta_tipo = 'RECEBIMENTO' ORDER BY vencimento";
        let mut stmt = self.conn.prepare(q).map_err(err)?;
        let rows = stmt
            .query_map(params![conta_id], parse_parcela)
            .map_err(err)?;
        rows.into_iter().map(|r| r.map_err(err)).collect()
    }

    fn linha_para_conta(
        &self,
        row: &Row<'_>,
    ) -> DomainResult<ContaReceber> {
        let id_s: String = row.get(0).map_err(err)?;
        let descricao: String = row.get(1).map_err(err)?;
        let plano_codigo: String = row.get(2).map_err(err)?;
        let plano_nome: String = row.get(3).map_err(err)?;
        let uid_s: String = row.get(4).map_err(err)?;
        let created_s: String = row.get(5).map_err(err)?;
        let pessoa_id: Option<String> = row.get(6).map_err(err)?;
        let referencia_id: Option<String> = row.get(7).map_err(err)?;
        let referencia_tipo: Option<String> = row.get(8).map_err(err)?;

        Ok(ContaReceber {
            id: parse_id::<ContaReceberId>(&id_s, "conta_receber.id")?,
            descricao,
            plano_conta: PlanoConta {
                codigo: plano_codigo,
                nome: plano_nome,
            },
            parcelas: self.carregar_parcelas(&id_s)?,
            criada_em: parse_data(&created_s)?,
            criada_por: parse_id::<UsuarioId>(&uid_s, "conta_receber.usuario_id")?,
            pessoa_id,
            referencia_id,
            referencia_tipo,
        })
    }
}

impl ContaReceberRepository for ContaReceberRepo<'_> {
    fn salvar(&self, conta: &ContaReceber) -> DomainResult<()> {
        upsert_plano(self.conn, &conta.plano_conta, "RECEBIMENTO")?;

        self.conn
            .execute(
                "INSERT INTO contas_receber
                     (id, descricao, plano_codigo, plano_tipo, usuario_id,
                      pessoa_id, referencia_id, referencia_tipo)
                 VALUES (?1, ?2, ?3, 'RECEBIMENTO', ?4, ?5, ?6, ?7)",
                params![
                    conta.id.to_string(),
                    &conta.descricao,
                    &conta.plano_conta.codigo,
                    conta.criada_por.to_string(),
                    &conta.pessoa_id,
                    &conta.referencia_id,
                    &conta.referencia_tipo,
                ],
            )
            .map_err(err)?;

        for (indice, p) in conta.parcelas.iter().enumerate() {
            self.conn
                .execute(
                    "INSERT INTO parcelas (id, conta_id, conta_tipo, numero, valor_centavos, vencimento, pago_em, status)
                     VALUES (?1, ?2, 'RECEBIMENTO', ?3, ?4, ?5, ?6, ?7)",
                    params![
                        p.id.to_string(),
                        conta.id.to_string(),
                        (indice + 1) as i64,
                        p.valor.centavos(),
                        p.vencimento.to_rfc3339(),
                        p.pago_em.as_ref().map(|d| d.to_rfc3339()),
                        p.status.as_str(),
                    ],
                )
                .map_err(err)?;
        }
        Ok(())
    }

    fn atualizar(&self, conta: &ContaReceber) -> DomainResult<()> {
        upsert_plano(self.conn, &conta.plano_conta, "RECEBIMENTO")?;

        let n = self
            .conn
            .execute(
                "UPDATE contas_receber
                 SET descricao = ?2, plano_codigo = ?3, plano_tipo = 'RECEBIMENTO'
                 WHERE id = ?1",
                params![
                    conta.id.to_string(),
                    &conta.descricao,
                    &conta.plano_conta.codigo,
                ],
            )
            .map_err(err)?;
        if n == 0 {
            return Err(DomainError::NaoEncontrado {
                entidade: "ContaReceber",
                id: conta.id.to_string(),
            });
        }
        for p in &conta.parcelas {
            let n = self
                .conn
                .execute(
                    "UPDATE parcelas SET pago_em = ?3, status = ?4
                     WHERE id = ?1 AND conta_id = ?2",
                    params![
                        p.id.to_string(),
                        conta.id.to_string(),
                        p.pago_em.as_ref().map(|d| d.to_rfc3339()),
                        p.status.as_str(),
                    ],
                )
                .map_err(err)?;
            if n == 0 {
                return Err(DomainError::NaoEncontrado {
                    entidade: "Parcela",
                    id: p.id.to_string(),
                });
            }
        }
        Ok(())
    }

    fn buscar_por_id(&self, id: ContaReceberId) -> DomainResult<Option<ContaReceber>> {
        let q = "SELECT c.id, c.descricao, c.plano_codigo, p.nome, c.usuario_id, c.created_at,
                        c.pessoa_id, c.referencia_id, c.referencia_tipo
                 FROM contas_receber c
                 JOIN planos_conta p ON p.codigo = c.plano_codigo AND p.tipo = 'RECEBIMENTO'
                 WHERE c.id = ?1";
        let mut stmt = self.conn.prepare(q).map_err(err)?;
        let mut rows = stmt.query(params![id.to_string()]).map_err(err)?;
        match rows.next().map_err(err)? {
            Some(row) => Ok(Some(self.linha_para_conta(row)?)),
            None => Ok(None),
        }
    }

    fn buscar_abertas(&self) -> DomainResult<Vec<ContaReceber>> {
        let q = "SELECT c.id, c.descricao, c.plano_codigo, p.nome, c.usuario_id, c.created_at,
                        c.pessoa_id, c.referencia_id, c.referencia_tipo
                 FROM contas_receber c
                 JOIN planos_conta p ON p.codigo = c.plano_codigo AND p.tipo = 'RECEBIMENTO'
                 WHERE EXISTS (
                     SELECT 1 FROM parcelas pa
                     WHERE pa.conta_id = c.id AND pa.conta_tipo = 'RECEBIMENTO' AND pa.status <> 'PAGA'
                 )
                 ORDER BY c.created_at";
        let mut stmt = self.conn.prepare(q).map_err(err)?;
        let mut rows = stmt.query([]).map_err(err)?;
        let mut v = Vec::new();
        while let Some(row) = rows.next().map_err(err)? {
            v.push(self.linha_para_conta(row)?);
        }
        Ok(v)
    }

    fn listar_planos(&self) -> DomainResult<Vec<PlanoConta>> {
        let q = "SELECT codigo, nome FROM planos_conta WHERE tipo = 'RECEBIMENTO' ORDER BY codigo";
        let mut stmt = self.conn.prepare(q).map_err(err)?;
        let mut rows = stmt.query([]).map_err(err)?;
        let mut v = Vec::new();
        while let Some(row) = rows.next().map_err(err)? {
            v.push(PlanoConta {
                codigo: row.get(0).map_err(err)?,
                nome: row.get(1).map_err(err)?,
            });
        }
        Ok(v)
    }

    fn cancelar_abertas_por_referencia(&self, referencia_id: &str) -> DomainResult<u32> {
        let alteradas = self
            .conn
            .execute(
                "UPDATE parcelas
                 SET status = 'CANCELADA',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                 WHERE status = 'ABERTA'
                   AND conta_tipo = 'RECEBIMENTO'
                   AND conta_id IN (
                       SELECT id FROM contas_receber
                       WHERE referencia_id = ?1 AND referencia_tipo = 'VENDA'
                   )",
                params![referencia_id],
            )
            .map_err(err)?;
        Ok(u32::try_from(alteradas).unwrap_or(0))
    }
}

// --- ParcelaRepo ---

pub struct ParcelaRepo<'a> {
    conn: &'a Connection,
}

impl<'a> ParcelaRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl ParcelaRepository for ParcelaRepo<'_> {
    fn buscar_por_id(
        &self,
        id: ParcelaId,
    ) -> DomainResult<Option<(ContaPagarId, Parcela)>> {
        let q = "SELECT conta_id, valor_centavos, vencimento, pago_em, status FROM parcelas WHERE id = ?1";
        let opt: Option<(String, i64, String, Option<String>, String)> = self
            .conn
            .query_row(q, params![id.to_string()], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .optional()
            .map_err(err)?;

        let Some((c_id_s, valor, venc_s, pago_opt, status_s)) = opt else {
            return Ok(None);
        };
        Ok(Some((
            parse_id::<ContaPagarId>(&c_id_s, "parcela.conta_id")?,
            Parcela {
                id,
                valor: Money::from_centavos(valor),
                vencimento: parse_data(&venc_s)?,
                pago_em: pago_opt.as_deref().map(parse_data).transpose()?,
                status: StatusParcela::from_db(&status_s)?,
            },
        )))
    }

    fn buscar_vencidas_ate(
        &self,
        data_limite: DateTime<Utc>,
        usuario_id: UsuarioId,
    ) -> DomainResult<Vec<(ContaPagarId, Parcela)>> {
        let q = "SELECT p.id, p.conta_id, p.valor_centavos, p.vencimento, p.pago_em, p.status
                 FROM parcelas p
                 JOIN contas_pagar c ON p.conta_id = c.id AND p.conta_tipo = 'PAGAMENTO'
                 WHERE p.vencimento <= ?1 AND p.status = 'ABERTA' AND c.usuario_id = ?2
                 ORDER BY p.vencimento";
        self.coletar_parcelas(q, params![data_limite.to_rfc3339(), usuario_id.to_string()])
    }

    fn listar_abertas(
        &self,
        usuario_id: UsuarioId,
    ) -> DomainResult<Vec<(ContaPagarId, Parcela)>> {
        let q = "SELECT p.id, p.conta_id, p.valor_centavos, p.vencimento, p.pago_em, p.status
                 FROM parcelas p
                 JOIN contas_pagar c ON p.conta_id = c.id AND p.conta_tipo = 'PAGAMENTO'
                 WHERE p.status = 'ABERTA' AND c.usuario_id = ?1
                 ORDER BY p.vencimento";
        self.coletar_parcelas(q, params![usuario_id.to_string()])
    }
}

impl ParcelaRepo<'_> {
    fn coletar_parcelas(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> DomainResult<Vec<(ContaPagarId, Parcela)>> {
        let mut stmt = self.conn.prepare(sql).map_err(err)?;
        let mut rows = stmt.query(params).map_err(err)?;
        let mut v = Vec::new();
        while let Some(row) = rows.next().map_err(err)? {
            let pid_s: String = row.get(0).map_err(err)?;
            let cid_s: String = row.get(1).map_err(err)?;
            let valor: i64 = row.get(2).map_err(err)?;
            let venc_s: String = row.get(3).map_err(err)?;
            let pago_opt: Option<String> = row.get(4).map_err(err)?;
            let status_s: String = row.get(5).map_err(err)?;

            v.push((
                parse_id::<ContaPagarId>(&cid_s, "parcela.conta_id")?,
                Parcela {
                    id: parse_id::<ParcelaId>(&pid_s, "parcela.id")?,
                    valor: Money::from_centavos(valor),
                    vencimento: parse_data(&venc_s)?,
                    pago_em: pago_opt.as_deref().map(parse_data).transpose()?,
                    status: StatusParcela::from_db(&status_s)?,
                },
            ));
        }
        Ok(v)
    }
}
