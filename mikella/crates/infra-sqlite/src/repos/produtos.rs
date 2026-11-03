//! Repositório de produtos (SQLite + FTS5).

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use mikella_domain_inventory::ports::ProdutoRepository;
use mikella_domain_inventory::produto::{Produto, Unidade};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::ProdutoId,
    money::Money,
    pagination::{Page, PageParams},
};

use super::{
    bool_para_inteiro, data_hora_sql, inteiro_para_bool, limit_seguro, parse_data_hora, parse_id,
    sql, ParaDominio,
};
use crate::error::{erro_campo, InfraResult};

pub struct ProdutoRepo<'a> {
    conn: &'a Connection,
}

impl<'a> ProdutoRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn buscar_por_id_inner(&self, id: ProdutoId) -> InfraResult<Option<Produto>> {
        let bruto = sql_opt(
            self.conn
                .query_row(
                    "SELECT id, codigo_barras, sku, nome, categoria_id, unidade,
                    preco_centavos, custo_centavos, estoque_minimo,
                    perecivel, alerta_validade_dias, arquivado, arquivado_em
             FROM produtos WHERE id = ?1",
                    params![id.to_string()],
                    ProdutoBruto::da_linha,
                )
                .optional(),
        )?;
        bruto.map(Produto::try_from).transpose()
    }
}

impl ProdutoRepository for ProdutoRepo<'_> {
    fn salvar(&self, produto: &Produto) -> DomainResult<()> {
        sql(self.conn.execute(
            "INSERT INTO produtos
                 (id, codigo_barras, sku, nome, categoria_id, unidade,
                  preco_centavos, custo_centavos, estoque_minimo,
                  perecivel, alerta_validade_dias, arquivado, arquivado_em)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                produto.id.to_string(),
                produto.codigo_barras,
                produto.sku,
                produto.nome,
                produto.categoria_id.map(|id| id.to_string()),
                produto.unidade.as_str(),
                produto.preco_centavos.centavos(),
                produto.custo_centavos.centavos(),
                produto.estoque_minimo,
                bool_para_inteiro(produto.perecivel),
                produto.alerta_validade_dias,
                bool_para_inteiro(produto.arquivado),
                produto.arquivado_em.map(data_hora_sql),
            ],
        ))?;
        Ok(())
    }

    fn atualizar(&self, produto: &Produto) -> DomainResult<()> {
        let alteradas = sql(self.conn.execute(
            "UPDATE produtos SET
                 codigo_barras = ?2, sku = ?3, nome = ?4, categoria_id = ?5,
                 unidade = ?6, preco_centavos = ?7, custo_centavos = ?8,
                 estoque_minimo = ?9, perecivel = ?10, alerta_validade_dias = ?11,
                 arquivado = ?12, arquivado_em = ?13,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?1",
            params![
                produto.id.to_string(),
                produto.codigo_barras,
                produto.sku,
                produto.nome,
                produto.categoria_id.map(|id| id.to_string()),
                produto.unidade.as_str(),
                produto.preco_centavos.centavos(),
                produto.custo_centavos.centavos(),
                produto.estoque_minimo,
                bool_para_inteiro(produto.perecivel),
                produto.alerta_validade_dias,
                bool_para_inteiro(produto.arquivado),
                produto.arquivado_em.map(data_hora_sql),
            ],
        ))?;
        if alteradas == 0 {
            return Err(DomainError::NaoEncontrado {
                entidade: "Produto",
                id: produto.id.to_string(),
            });
        }
        Ok(())
    }

    fn buscar_por_id(&self, id: ProdutoId) -> DomainResult<Option<Produto>> {
        self.buscar_por_id_inner(id).dominio()
    }

    fn buscar_por_codigo_barras(&self, codigo: &str) -> DomainResult<Option<Produto>> {
        let bruto = sql_opt(
            self.conn
                .query_row(
                    "SELECT id, codigo_barras, sku, nome, categoria_id, unidade,
                    preco_centavos, custo_centavos, estoque_minimo,
                    perecivel, alerta_validade_dias, arquivado, arquivado_em
             FROM produtos WHERE codigo_barras = ?1",
                    params![codigo],
                    ProdutoBruto::da_linha,
                )
                .optional(),
        )?;
        bruto.map(Produto::try_from).transpose().map_err(Into::into)
    }

    fn buscar_por_sku(&self, sku: &str) -> DomainResult<Option<Produto>> {
        let bruto = sql_opt(
            self.conn
                .query_row(
                    "SELECT id, codigo_barras, sku, nome, categoria_id, unidade,
                    preco_centavos, custo_centavos, estoque_minimo,
                    perecivel, alerta_validade_dias, arquivado, arquivado_em
             FROM produtos WHERE sku = ?1",
                    params![sku],
                    ProdutoBruto::da_linha,
                )
                .optional(),
        )?;
        bruto.map(Produto::try_from).transpose().map_err(Into::into)
    }

    fn pesquisar(&self, query: &str, params: PageParams) -> DomainResult<Page<Produto>> {
        // A busca do usuário não é sintaxe FTS5. Quebramos em palavras
        // (apenas letras/números) e montamos uma frase com prefixo — assim
        // aspas, parênteses, `*` e operadores colados não viram comando.
        let palavras: Vec<&str> = query
            .split(|caractere: char| !caractere.is_alphanumeric())
            .filter(|palavra| !palavra.is_empty())
            .collect();

        if palavras.is_empty() {
            return self.listar_ativos(params);
        }

        let match_fts = format!("\"{}\"*", palavras.join(" "));
        let cursor_rowid: i64 = match params.cursor.as_deref() {
            Some(cursor) => cursor
                .parse()
                .map_err(|_| DomainError::Invalido("Cursor de busca inválido".into()))?,
            None => 0,
        };
        let limite = limit_seguro(&params);

        let mut stmt = sql(self.conn.prepare(
            "SELECT p.id, p.codigo_barras, p.sku, p.nome, p.categoria_id, p.unidade,
                    p.preco_centavos, p.custo_centavos, p.estoque_minimo,
                    p.perecivel, p.alerta_validade_dias, p.arquivado, p.arquivado_em,
                    p.rowid
             FROM produtos_fts f
             JOIN produtos p ON p.rowid = f.rowid
             WHERE produtos_fts MATCH ?1
               AND p.arquivado = 0
               AND p.rowid > ?2
             ORDER BY p.rowid ASC
             LIMIT ?3",
        ))?;

        let linhas = sql(
            stmt.query_map(params![match_fts, cursor_rowid, limite], |row| {
                Ok((ProdutoBruto::da_linha(row)?, row.get::<_, i64>(13)?))
            }),
        )?;

        let mut itens = Vec::new();
        let mut ultimo_rowid = None;
        for linha in linhas {
            let (bruto, rowid) = sql(linha)?;
            itens.push(Produto::try_from(bruto)?);
            ultimo_rowid = Some(rowid);
        }

        let next_cursor = if itens.len() as i64 == limite {
            ultimo_rowid.map(|valor| valor.to_string())
        } else {
            None
        };
        Ok(Page::new(itens, next_cursor))
    }

    fn listar_ativos(&self, params: PageParams) -> DomainResult<Page<Produto>> {
        let cursor: Option<CursorProduto> = match params.cursor.as_deref() {
            Some(texto) => Some(
                serde_json::from_str(texto)
                    .map_err(|_| DomainError::Invalido("Cursor de produtos inválido".into()))?,
            ),
            None => None,
        };
        let limite = limit_seguro(&params);

        let mut stmt = sql(self.conn.prepare(
            "SELECT id, codigo_barras, sku, nome, categoria_id, unidade,
                    preco_centavos, custo_centavos, estoque_minimo,
                    perecivel, alerta_validade_dias, arquivado, arquivado_em
             FROM produtos
             WHERE arquivado = 0
               AND (?1 IS NULL OR (nome, id) > (?1, ?2))
             ORDER BY nome ASC, id ASC
             LIMIT ?3",
        ))?;

        let linhas = sql(stmt.query_map(
            params![
                cursor.as_ref().map(|c| c.nome.as_str()),
                cursor.as_ref().map(|c| c.id.as_str()),
                limite
            ],
            ProdutoBruto::da_linha,
        ))?;

        let mut itens = Vec::new();
        for linha in linhas {
            itens.push(Produto::try_from(sql(linha)?)?);
        }

        let next_cursor = if itens.len() as i64 == limite {
            itens.last().map(|ultimo| {
                serde_json::to_string(&CursorProduto {
                    nome: ultimo.nome.clone(),
                    id: ultimo.id.to_string(),
                })
                .expect("serializar cursor de produto nunca falha")
            })
        } else {
            None
        };

        Ok(Page::new(itens, next_cursor))
    }

    fn estoque_atual(&self, produto_id: ProdutoId) -> DomainResult<i32> {
        let saldo: Option<i64> = sql_opt(
            self.conn
                .query_row(
                    "SELECT saldo_atual FROM vw_estoque_atual WHERE produto_id = ?1",
                    params![produto_id.to_string()],
                    |row| row.get(0),
                )
                .optional(),
        )?;
        Ok(saldo.unwrap_or(0) as i32)
    }
}

#[derive(Serialize, Deserialize)]
struct CursorProduto {
    nome: String,
    id: String,
}

struct ProdutoBruto {
    id: String,
    codigo_barras: Option<String>,
    sku: Option<String>,
    nome: String,
    categoria_id: Option<String>,
    unidade: String,
    preco_centavos: i64,
    custo_centavos: i64,
    estoque_minimo: i64,
    perecivel: i64,
    alerta_validade_dias: i64,
    arquivado: i64,
    arquivado_em: Option<String>,
}

impl ProdutoBruto {
    fn da_linha(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            codigo_barras: row.get(1)?,
            sku: row.get(2)?,
            nome: row.get(3)?,
            categoria_id: row.get(4)?,
            unidade: row.get(5)?,
            preco_centavos: row.get(6)?,
            custo_centavos: row.get(7)?,
            estoque_minimo: row.get(8)?,
            perecivel: row.get(9)?,
            alerta_validade_dias: row.get(10)?,
            arquivado: row.get(11)?,
            arquivado_em: row.get(12)?,
        })
    }
}

impl TryFrom<ProdutoBruto> for Produto {
    type Error = crate::error::InfraError;

    fn try_from(bruto: ProdutoBruto) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_id(&bruto.id, "produtos.id")?,
            codigo_barras: bruto.codigo_barras,
            sku: bruto.sku,
            nome: bruto.nome,
            categoria_id: bruto
                .categoria_id
                .as_deref()
                .map(|valor| parse_id(valor, "produtos.categoria_id"))
                .transpose()?,
            unidade: Unidade::from_db(&bruto.unidade)
                .map_err(|erro| erro_campo("produtos.unidade", erro))?,
            preco_centavos: Money::from_centavos(bruto.preco_centavos),
            custo_centavos: Money::from_centavos(bruto.custo_centavos),
            estoque_minimo: i32::try_from(bruto.estoque_minimo)
                .map_err(|erro| erro_campo("produtos.estoque_minimo", erro))?,
            perecivel: inteiro_para_bool(bruto.perecivel, "produtos.perecivel")?,
            alerta_validade_dias: u32::try_from(bruto.alerta_validade_dias)
                .map_err(|erro| erro_campo("produtos.alerta_validade_dias", erro))?,
            arquivado: inteiro_para_bool(bruto.arquivado, "produtos.arquivado")?,
            arquivado_em: bruto
                .arquivado_em
                .as_deref()
                .map(|valor| parse_data_hora(valor, "produtos.arquivado_em"))
                .transpose()?,
        })
    }
}

/// `query_row(...).optional()` retorna `Result<Option<T>, rusqlite::Error>`,
/// mas `sql()` só converte erros. Este atalho deixa o código legível.
fn sql_opt<T>(resultado: Result<Option<T>, rusqlite::Error>) -> InfraResult<Option<T>> {
    resultado.map_err(Into::into)
}
