//! Repositório de movimentações de estoque — INSERT ONLY.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use mikella_domain_inventory::movimentacao::{MovimentacaoEstoque, OrigemMovimentacao};
use mikella_domain_inventory::ports::MovimentacaoEstoqueRepository;
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{LoteId, MovimentacaoEstoqueId, ProdutoId, VendaId},
    money::Money,
    pagination::{Page, PageParams},
};

use super::{data_hora_sql, limit_seguro, parse_data_hora, parse_id, sql, ParaDominio};
use crate::error::InfraResult;

pub struct MovimentacaoRepo<'a> {
    conn: &'a Connection,
}

impl<'a> MovimentacaoRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn buscar_por_id_inner(
        &self,
        id: MovimentacaoEstoqueId,
    ) -> InfraResult<Option<MovimentacaoEstoque>> {
        let bruto = self
            .conn
            .query_row(
                "SELECT id, produto_id, lote_id, tipo, quantidade, custo_unitario_centavos,
                        referencia_id, referencia_tipo, usuario_id, motivo, created_at
                 FROM movimentacoes_estoque WHERE id = ?1",
                params![id.to_string()],
                MovimentacaoBruta::da_linha,
            )
            .optional()?;
        bruto.map(MovimentacaoEstoque::try_from).transpose()
    }
}

impl MovimentacaoEstoqueRepository for MovimentacaoRepo<'_> {
    fn registrar(&self, mov: &MovimentacaoEstoque) -> DomainResult<()> {
        let referencia_tipo = if mov.venda_id.is_some() {
            Some("VENDA")
        } else {
            None
        };

        sql(self.conn.execute(
            "INSERT INTO movimentacoes_estoque
                 (id, produto_id, lote_id, tipo, quantidade, custo_unitario_centavos,
                  referencia_id, referencia_tipo, usuario_id, motivo, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                mov.id.to_string(),
                mov.produto_id.to_string(),
                mov.lote_id.map(|id| id.to_string()),
                mov.origem.as_str(),
                mov.quantidade,
                mov.custo_unitario_centavos.centavos(),
                mov.referencia_id,
                referencia_tipo,
                mov.usuario_id.to_string(),
                mov.observacao,
                data_hora_sql(mov.created_at),
            ],
        ))?;
        Ok(())
    }

    fn listar_por_produto(
        &self,
        produto_id: ProdutoId,
        params: PageParams,
    ) -> DomainResult<Page<MovimentacaoEstoque>> {
        let cursor: Option<CursorMovimentacao> =
            match params.cursor.as_deref() {
                Some(texto) => Some(serde_json::from_str(texto).map_err(|_| {
                    DomainError::Invalido("Cursor de movimentações inválido".into())
                })?),
                None => None,
            };
        let limite = limit_seguro(&params);

        let mut stmt = sql(self.conn.prepare(
            "SELECT id, produto_id, lote_id, tipo, quantidade, custo_unitario_centavos,
                    referencia_id, referencia_tipo, usuario_id, motivo, created_at
             FROM movimentacoes_estoque
             WHERE produto_id = ?1
               AND (?2 IS NULL OR (created_at, id) < (?2, ?3))
             ORDER BY created_at DESC, id DESC
             LIMIT ?4",
        ))?;

        let linhas = sql(stmt.query_map(
            params![
                produto_id.to_string(),
                cursor.as_ref().map(|c| c.created_at.as_str()),
                cursor.as_ref().map(|c| c.id.as_str()),
                limite
            ],
            MovimentacaoBruta::da_linha,
        ))?;

        let mut itens = Vec::new();
        for linha in linhas {
            itens.push(MovimentacaoEstoque::try_from(sql(linha)?)?);
        }

        let next_cursor = if itens.len() as i64 == limite {
            itens.last().map(|ultimo| {
                serde_json::to_string(&CursorMovimentacao {
                    created_at: data_hora_sql(ultimo.created_at),
                    id: ultimo.id.to_string(),
                })
                .expect("serializar cursor de movimentação nunca falha")
            })
        } else {
            None
        };

        Ok(Page::new(itens, next_cursor))
    }

    fn buscar_por_id(
        &self,
        id: MovimentacaoEstoqueId,
    ) -> DomainResult<Option<MovimentacaoEstoque>> {
        self.buscar_por_id_inner(id).dominio()
    }
}

#[derive(Serialize, Deserialize)]
struct CursorMovimentacao {
    created_at: String,
    id: String,
}

struct MovimentacaoBruta {
    id: String,
    produto_id: String,
    lote_id: Option<String>,
    tipo: String,
    quantidade: i64,
    custo_unitario_centavos: i64,
    referencia_id: Option<String>,
    referencia_tipo: Option<String>,
    usuario_id: String,
    motivo: Option<String>,
    created_at: String,
}

impl MovimentacaoBruta {
    fn da_linha(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            produto_id: row.get(1)?,
            lote_id: row.get(2)?,
            tipo: row.get(3)?,
            quantidade: row.get(4)?,
            custo_unitario_centavos: row.get(5)?,
            referencia_id: row.get(6)?,
            referencia_tipo: row.get(7)?,
            usuario_id: row.get(8)?,
            motivo: row.get(9)?,
            created_at: row.get(10)?,
        })
    }
}

impl TryFrom<MovimentacaoBruta> for MovimentacaoEstoque {
    type Error = crate::error::InfraError;

    fn try_from(bruto: MovimentacaoBruta) -> Result<Self, Self::Error> {
        let venda_id: Option<VendaId> = if bruto.referencia_tipo.as_deref() == Some("VENDA") {
            bruto
                .referencia_id
                .as_deref()
                .map(|valor| parse_id(valor, "movimentacoes_estoque.referencia_id"))
                .transpose()?
        } else {
            None
        };

        Ok(Self {
            id: parse_id(&bruto.id, "movimentacoes_estoque.id")?,
            produto_id: parse_id(&bruto.produto_id, "movimentacoes_estoque.produto_id")?,
            lote_id: bruto
                .lote_id
                .as_deref()
                .map(|valor| parse_id::<LoteId>(valor, "movimentacoes_estoque.lote_id"))
                .transpose()?,
            quantidade: i32::try_from(bruto.quantidade).map_err(|erro| {
                crate::error::erro_campo("movimentacoes_estoque.quantidade", erro)
            })?,
            custo_unitario_centavos: Money::from_centavos(bruto.custo_unitario_centavos),
            origem: OrigemMovimentacao::from_db(&bruto.tipo)
                .map_err(|erro| crate::error::erro_campo("movimentacoes_estoque.tipo", erro))?,
            referencia_id: bruto.referencia_id,
            venda_id,
            usuario_id: parse_id(&bruto.usuario_id, "movimentacoes_estoque.usuario_id")?,
            observacao: bruto.motivo,
            created_at: parse_data_hora(&bruto.created_at, "movimentacoes_estoque.created_at")?,
        })
    }
}
