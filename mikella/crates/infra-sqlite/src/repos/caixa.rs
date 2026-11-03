//! Repositórios de sessões de caixa e do livro (cadeia de hashes).

use rusqlite::{params, Connection, OptionalExtension, Row};

use mikella_domain_cash::livro::{EntradaLivro, LivroCaixa, Referencia, TipoMovimentacao};
use mikella_domain_cash::ports::{LivroCaixaRepository, PosicaoLivro, SessaoRepository};
use mikella_domain_cash::sessao::{EstadoSessao, SessaoCaixa};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{DispositivoId, LivroCaixaId, SessaoId, UsuarioId},
    money::Money,
};

use super::{
    bool_para_inteiro, data_hora_sql, inteiro_para_bool, parse_data_hora, parse_id, sql,
    ParaDominio,
};
use crate::error::InfraResult;

pub struct SessaoRepo<'a> {
    conn: &'a Connection,
}

impl<'a> SessaoRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn buscar_por_id_inner(&self, id: SessaoId) -> InfraResult<Option<SessaoCaixa>> {
        let bruto = self
            .conn
            .query_row(
                "SELECT id, numero_caixa, dispositivo_id, operador_id, aberta_em, fechada_em,
                        estado, saldo_inicial_centavos, saldo_esperado_centavos,
                        saldo_contado_centavos, fechamento_cego
                 FROM sessoes_caixa WHERE id = ?1",
                params![id.to_string()],
                SessaoBruta::da_linha,
            )
            .optional()?;
        bruto.map(SessaoCaixa::try_from).transpose()
    }
}

impl SessaoRepository for SessaoRepo<'_> {
    fn salvar(&self, sessao: &SessaoCaixa) -> DomainResult<()> {
        sql(self.conn.execute(
            "INSERT INTO sessoes_caixa
                 (id, numero_caixa, dispositivo_id, operador_id, aberta_em, fechada_em,
                  estado, saldo_inicial_centavos, saldo_esperado_centavos,
                  saldo_contado_centavos, diferenca_centavos, fechamento_cego)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                sessao.id.to_string(),
                sessao.numero_caixa,
                sessao.dispositivo_id.to_string(),
                sessao.operador_id.to_string(),
                data_hora_sql(sessao.aberta_em),
                sessao.fechada_em.map(data_hora_sql),
                sessao.estado.as_str(),
                sessao.saldo_inicial_centavos.centavos(),
                sessao.saldo_esperado_centavos.centavos(),
                sessao.saldo_contado_centavos.map(|valor| valor.centavos()),
                diferenca(sessao),
                bool_para_inteiro(sessao.fechamento_cego),
            ],
        ))?;
        Ok(())
    }

    fn buscar_por_id(&self, id: SessaoId) -> DomainResult<Option<SessaoCaixa>> {
        self.buscar_por_id_inner(id).dominio()
    }

    fn buscar_sessoes_abertas(&self) -> DomainResult<Vec<SessaoCaixa>> {
        let mut stmt = sql(self.conn.prepare(
            "SELECT id, numero_caixa, dispositivo_id, operador_id, aberta_em, fechada_em,
                    estado, saldo_inicial_centavos, saldo_esperado_centavos,
                    saldo_contado_centavos, fechamento_cego
             FROM sessoes_caixa
             WHERE estado IN ('ABERTA','EM_FECHAMENTO')
             ORDER BY aberta_em ASC",
        ))?;
        let linhas = sql(stmt.query_map([], SessaoBruta::da_linha))?;

        let mut sessoes = Vec::new();
        for linha in linhas {
            sessoes.push(SessaoCaixa::try_from(sql(linha)?)?);
        }
        Ok(sessoes)
    }

    fn atualizar(&self, sessao: &SessaoCaixa) -> DomainResult<()> {
        let alteradas = sql(self.conn.execute(
            "UPDATE sessoes_caixa SET
                 estado = ?2, fechada_em = ?3,
                 saldo_esperado_centavos = ?4, saldo_contado_centavos = ?5,
                 diferenca_centavos = ?6
             WHERE id = ?1",
            params![
                sessao.id.to_string(),
                sessao.estado.as_str(),
                sessao.fechada_em.map(data_hora_sql),
                sessao.saldo_esperado_centavos.centavos(),
                sessao.saldo_contado_centavos.map(|valor| valor.centavos()),
                diferenca(sessao),
            ],
        ))?;
        if alteradas == 0 {
            return Err(DomainError::NaoEncontrado {
                entidade: "SessaoCaixa",
                id: sessao.id.to_string(),
            });
        }
        Ok(())
    }
}

fn diferenca(sessao: &SessaoCaixa) -> Option<i64> {
    sessao
        .saldo_contado_centavos
        .map(|contado| contado.centavos() - sessao.saldo_esperado_centavos.centavos())
}

struct SessaoBruta {
    id: String,
    numero_caixa: i64,
    dispositivo_id: String,
    operador_id: String,
    aberta_em: String,
    fechada_em: Option<String>,
    estado: String,
    saldo_inicial_centavos: i64,
    saldo_esperado_centavos: i64,
    saldo_contado_centavos: Option<i64>,
    fechamento_cego: i64,
}

impl SessaoBruta {
    fn da_linha(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            numero_caixa: row.get(1)?,
            dispositivo_id: row.get(2)?,
            operador_id: row.get(3)?,
            aberta_em: row.get(4)?,
            fechada_em: row.get(5)?,
            estado: row.get(6)?,
            saldo_inicial_centavos: row.get(7)?,
            saldo_esperado_centavos: row.get(8)?,
            saldo_contado_centavos: row.get(9)?,
            fechamento_cego: row.get(10)?,
        })
    }
}

impl TryFrom<SessaoBruta> for SessaoCaixa {
    type Error = crate::error::InfraError;

    fn try_from(bruto: SessaoBruta) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_id(&bruto.id, "sessoes_caixa.id")?,
            numero_caixa: u8::try_from(bruto.numero_caixa)
                .map_err(|erro| crate::error::erro_campo("sessoes_caixa.numero_caixa", erro))?,
            dispositivo_id: parse_id::<DispositivoId>(
                &bruto.dispositivo_id,
                "sessoes_caixa.dispositivo_id",
            )?,
            operador_id: parse_id::<UsuarioId>(&bruto.operador_id, "sessoes_caixa.operador_id")?,
            estado: EstadoSessao::from_db(&bruto.estado)
                .map_err(|erro| crate::error::erro_campo("sessoes_caixa.estado", erro))?,
            aberta_em: parse_data_hora(&bruto.aberta_em, "sessoes_caixa.aberta_em")?,
            fechada_em: bruto
                .fechada_em
                .as_deref()
                .map(|valor| parse_data_hora(valor, "sessoes_caixa.fechada_em"))
                .transpose()?,
            fechamento_cego: inteiro_para_bool(
                bruto.fechamento_cego,
                "sessoes_caixa.fechamento_cego",
            )?,
            saldo_inicial_centavos: Money::from_centavos(bruto.saldo_inicial_centavos),
            saldo_esperado_centavos: Money::from_centavos(bruto.saldo_esperado_centavos),
            saldo_contado_centavos: bruto.saldo_contado_centavos.map(Money::from_centavos),
        })
    }
}

// ============================================================
// Livro do caixa
// ============================================================

pub struct LivroRepo<'a> {
    conn: &'a Connection,
}

impl<'a> LivroRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl LivroCaixaRepository for LivroRepo<'_> {
    fn inserir_entrada(&self, entrada: &EntradaLivro) -> DomainResult<()> {
        sql(self.conn.execute(
            "INSERT INTO livro_caixa
                 (id, sessao_id, sequencia, tipo, valor_centavos, troco_centavos,
                  referencia_id, referencia_tipo, usuario_id, aprovador_id, motivo,
                  prev_hash, hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                entrada.id.to_string(),
                entrada.sessao_id.to_string(),
                entrada.sequencia,
                entrada.tipo.as_str(),
                entrada.valor_centavos.centavos(),
                entrada.troco_centavos.centavos(),
                entrada.referencia.as_ref().map(|r| r.id.as_str()),
                entrada.referencia.as_ref().map(|r| r.tipo.as_str()),
                entrada.usuario_id.to_string(),
                entrada.aprovador_id.map(|id| id.to_string()),
                entrada.motivo,
                entrada.prev_hash,
                entrada.hash,
                data_hora_sql(entrada.created_at),
            ],
        ))?;
        Ok(())
    }

    fn carregar_livro(&self, sessao_id: SessaoId) -> DomainResult<LivroCaixa> {
        let mut stmt = sql(self.conn.prepare(
            "SELECT id, sessao_id, sequencia, tipo, valor_centavos, troco_centavos,
                    referencia_id, referencia_tipo, usuario_id, aprovador_id, motivo,
                    prev_hash, hash, created_at
             FROM livro_caixa
             WHERE sessao_id = ?1
             ORDER BY sequencia ASC",
        ))?;
        let linhas = sql(stmt.query_map(params![sessao_id.to_string()], EntradaBruta::da_linha))?;

        let mut entradas = Vec::new();
        for linha in linhas {
            entradas.push(EntradaLivro::try_from(sql(linha)?)?);
        }
        Ok(LivroCaixa::de_entradas(sessao_id, entradas))
    }

    fn posicao_atual(&self, sessao_id: SessaoId) -> DomainResult<PosicaoLivro> {
        let ultima: Option<(String, i64)> = sql(self
            .conn
            .query_row(
                "SELECT hash, sequencia FROM livro_caixa
             WHERE sessao_id = ?1
             ORDER BY sequencia DESC LIMIT 1",
                params![sessao_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional())?;

        Ok(match ultima {
            Some((hash, sequencia)) => PosicaoLivro {
                ultimo_hash: hash,
                proxima_sequencia: sequencia + 1,
            },
            None => PosicaoLivro {
                ultimo_hash: "GENESIS".to_string(),
                proxima_sequencia: 1,
            },
        })
    }
}

struct EntradaBruta {
    id: String,
    sessao_id: String,
    sequencia: i64,
    tipo: String,
    valor_centavos: i64,
    troco_centavos: i64,
    referencia_id: Option<String>,
    referencia_tipo: Option<String>,
    usuario_id: String,
    aprovador_id: Option<String>,
    motivo: Option<String>,
    prev_hash: String,
    hash: String,
    created_at: String,
}

impl EntradaBruta {
    fn da_linha(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            sessao_id: row.get(1)?,
            sequencia: row.get(2)?,
            tipo: row.get(3)?,
            valor_centavos: row.get(4)?,
            troco_centavos: row.get(5)?,
            referencia_id: row.get(6)?,
            referencia_tipo: row.get(7)?,
            usuario_id: row.get(8)?,
            aprovador_id: row.get(9)?,
            motivo: row.get(10)?,
            prev_hash: row.get(11)?,
            hash: row.get(12)?,
            created_at: row.get(13)?,
        })
    }
}

impl TryFrom<EntradaBruta> for EntradaLivro {
    type Error = crate::error::InfraError;

    fn try_from(bruto: EntradaBruta) -> Result<Self, Self::Error> {
        let referencia = match (bruto.referencia_id, bruto.referencia_tipo) {
            (Some(id), Some(tipo)) => Some(Referencia { id, tipo }),
            _ => None,
        };

        Ok(Self {
            id: parse_id::<LivroCaixaId>(&bruto.id, "livro_caixa.id")?,
            sessao_id: parse_id(&bruto.sessao_id, "livro_caixa.sessao_id")?,
            sequencia: bruto.sequencia,
            tipo: TipoMovimentacao::from_db(&bruto.tipo)
                .map_err(|erro| crate::error::erro_campo("livro_caixa.tipo", erro))?,
            valor_centavos: Money::from_centavos(bruto.valor_centavos),
            troco_centavos: Money::from_centavos(bruto.troco_centavos),
            referencia,
            usuario_id: parse_id(&bruto.usuario_id, "livro_caixa.usuario_id")?,
            aprovador_id: bruto
                .aprovador_id
                .as_deref()
                .map(|valor| parse_id::<UsuarioId>(valor, "livro_caixa.aprovador_id"))
                .transpose()?,
            motivo: bruto.motivo,
            prev_hash: bruto.prev_hash,
            hash: bruto.hash,
            created_at: parse_data_hora(&bruto.created_at, "livro_caixa.created_at")?,
        })
    }
}
