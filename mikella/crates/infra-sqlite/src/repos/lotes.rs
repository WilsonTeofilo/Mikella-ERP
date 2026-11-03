//! Repositório de lotes (validade / FEFO).

use rusqlite::{params, Connection, OptionalExtension, Row};

use mikella_domain_inventory::lote::Lote;
use mikella_domain_inventory::ports::LoteRepository;
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{LoteId, ProdutoId},
    money::Money,
};

use super::{
    bool_para_inteiro, data_hora_sql, inteiro_para_bool, parse_data, parse_data_hora, parse_id,
    sql, ParaDominio,
};
use crate::error::InfraResult;

pub struct LoteRepo<'a> {
    conn: &'a Connection,
}

impl<'a> LoteRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn buscar_por_id_inner(&self, id: LoteId) -> InfraResult<Option<Lote>> {
        let bruto = self
            .conn
            .query_row(
                "SELECT id, produto_id, numero_lote, validade, custo_centavos, ativo, created_at
                 FROM lotes WHERE id = ?1",
                params![id.to_string()],
                LoteBruto::da_linha,
            )
            .optional()
            .map_err(crate::error::InfraError::from)?;
        bruto.map(Lote::try_from).transpose()
    }

    fn listar(&self, sql_texto: &str, params: &[&dyn rusqlite::ToSql]) -> DomainResult<Vec<Lote>> {
        let mut stmt = sql(self.conn.prepare(sql_texto))?;
        let linhas = sql(stmt.query_map(params, LoteBruto::da_linha))?;
        let mut lotes = Vec::new();
        for linha in linhas {
            lotes.push(Lote::try_from(sql(linha)?)?);
        }
        Ok(lotes)
    }
}

impl LoteRepository for LoteRepo<'_> {
    fn salvar(&self, lote: &Lote) -> DomainResult<()> {
        sql(self.conn.execute(
            "INSERT INTO lotes
                 (id, produto_id, numero_lote, validade, custo_centavos, ativo, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                lote.id.to_string(),
                lote.produto_id.to_string(),
                lote.numero_lote,
                lote.validade
                    .map(|data| data.format("%Y-%m-%d").to_string()),
                lote.custo_centavos.centavos(),
                bool_para_inteiro(lote.ativo),
                data_hora_sql(lote.created_at),
            ],
        ))?;
        Ok(())
    }

    fn atualizar(&self, lote: &Lote) -> DomainResult<()> {
        let alteradas = sql(self.conn.execute(
            "UPDATE lotes SET
                 numero_lote = ?2, validade = ?3, custo_centavos = ?4, ativo = ?5
             WHERE id = ?1",
            params![
                lote.id.to_string(),
                lote.numero_lote,
                lote.validade
                    .map(|data| data.format("%Y-%m-%d").to_string()),
                lote.custo_centavos.centavos(),
                bool_para_inteiro(lote.ativo),
            ],
        ))?;
        if alteradas == 0 {
            return Err(DomainError::NaoEncontrado {
                entidade: "Lote",
                id: lote.id.to_string(),
            });
        }
        Ok(())
    }

    fn buscar_por_id(&self, id: LoteId) -> DomainResult<Option<Lote>> {
        self.buscar_por_id_inner(id).dominio()
    }

    fn listar_por_produto_fefo(&self, produto_id: ProdutoId) -> DomainResult<Vec<Lote>> {
        self.listar(
            "SELECT id, produto_id, numero_lote, validade, custo_centavos, ativo, created_at
             FROM lotes
             WHERE produto_id = ?1 AND ativo = 1
             ORDER BY validade IS NULL, validade ASC, created_at ASC",
            &[&produto_id.to_string()],
        )
    }

    fn lotes_proximos_vencimento(&self, dias: u32) -> DomainResult<Vec<Lote>> {
        self.listar(
            "SELECT id, produto_id, numero_lote, validade, custo_centavos, ativo, created_at
             FROM lotes
             WHERE ativo = 1
               AND validade IS NOT NULL
               AND validade <= date('now','localtime', ?1)
               AND validade >= date('now','localtime')
             ORDER BY validade ASC",
            &[&format!("+{dias} days")],
        )
    }

    fn estoque_atual(&self, lote_id: LoteId) -> DomainResult<i32> {
        let saldo: Option<i64> = self
            .conn
            .query_row(
                "SELECT saldo_atual FROM vw_estoque_lote WHERE lote_id = ?1",
                params![lote_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(crate::error::InfraError::from)?;
        Ok(saldo.unwrap_or(0) as i32)
    }
}

struct LoteBruto {
    id: String,
    produto_id: String,
    numero_lote: Option<String>,
    validade: Option<String>,
    custo_centavos: i64,
    ativo: i64,
    created_at: String,
}

impl LoteBruto {
    fn da_linha(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            produto_id: row.get(1)?,
            numero_lote: row.get(2)?,
            validade: row.get(3)?,
            custo_centavos: row.get(4)?,
            ativo: row.get(5)?,
            created_at: row.get(6)?,
        })
    }
}

impl TryFrom<LoteBruto> for Lote {
    type Error = crate::error::InfraError;

    fn try_from(bruto: LoteBruto) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_id(&bruto.id, "lotes.id")?,
            produto_id: parse_id(&bruto.produto_id, "lotes.produto_id")?,
            numero_lote: bruto.numero_lote,
            validade: bruto
                .validade
                .as_deref()
                .map(|valor| parse_data(valor, "lotes.validade"))
                .transpose()?,
            custo_centavos: Money::from_centavos(bruto.custo_centavos),
            ativo: inteiro_para_bool(bruto.ativo, "lotes.ativo")?,
            created_at: parse_data_hora(&bruto.created_at, "lotes.created_at")?,
        })
    }
}
