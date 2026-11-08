//! Repositório de vendas — cabeçalho, itens e pagamentos.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use mikella_domain_sales::item::ItemVenda;
use mikella_domain_sales::ports::VendaRepository;
use mikella_domain_sales::venda::{EstadoVenda, FormaPagamento, Pagamento, Venda};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{DispositivoId, LoteId, ProdutoId, SessaoId, UsuarioId, VendaId},
    money::Money,
    pagination::{Page, PageParams},
};

use super::{data_hora_sql, limit_seguro, parse_data_hora, parse_id, sql, ParaDominio};
use crate::error::InfraResult;

/// Filtros do histórico de vendas (query params já validados pelo handler).
#[derive(Debug, Clone, Default)]
pub struct FiltroVendas {
    pub codigo: Option<String>,
    /// Limite inferior do intervalo (timestamp UTC RFC-3339, inclusivo).
    pub data_inicio: Option<String>,
    /// Limite superior do intervalo (timestamp UTC RFC-3339, inclusivo).
    pub data_fim: Option<String>,
    /// Quando `true`, o filtro por data é ignorado e retorna todos os registros no limite dado.
    pub sem_filtro_data: bool,
    pub limite: i64,
}

/// Linha crua do read model de vendas (sem formatação monetária).
#[derive(Debug, Clone)]
pub struct VendaResumo {
    pub id: String,
    pub sessao_id: String,
    pub operador_id: String,
    pub estado: String,
    pub total_centavos: i64,
    pub recebido_centavos: i64,
    pub created_at: String,
    pub coo: i64,
    pub operador: String,
}

impl VendaResumo {
    fn da_linha(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            sessao_id: row.get(1)?,
            operador_id: row.get(2)?,
            estado: row.get(3)?,
            total_centavos: row.get(4)?,
            recebido_centavos: row.get(5)?,
            created_at: row.get(6)?,
            coo: row.get(7)?,
            operador: row.get(8)?,
        })
    }
}

pub struct VendaRepo<'a> {
    conn: &'a Connection,
}

impl<'a> VendaRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn buscar_por_id_inner(&self, id: VendaId) -> InfraResult<Option<Venda>> {
        let bruto = self
            .conn
            .query_row(
                "SELECT id, sessao_id, dispositivo_id, operador_id, vendedor_id, comanda_id,
                        estado, forma_pagamento, subtotal_centavos, desconto_centavos,
                        total_centavos, recebido_centavos, troco_centavos, created_at,
                        forma_pagamento_detalhe
                 FROM vendas WHERE id = ?1",
                params![id.to_string()],
                VendaBruta::da_linha,
            )
            .optional()?;

        let Some(bruto) = bruto else {
            return Ok(None);
        };

        let itens = self.carregar_itens(id)?;
        let pagamentos = self.carregar_pagamentos(id)?;
        let venda = bruto.para_venda(itens, pagamentos)?;
        Ok(Some(venda))
    }

    /// Read model do histórico de vendas (F3/estorno), com filtro por COO ou
    /// período e nome do operador. O SQL fica na infraestrutura (ARQ-001).
    pub fn listar_resumo(&self, filtro: &FiltroVendas) -> InfraResult<Vec<VendaResumo>> {
        use rusqlite::types::Value;

        let mut sql_query = String::from(
            "SELECT v.id, v.sessao_id, v.operador_id, v.estado, v.total_centavos, \
                    v.recebido_centavos, v.created_at, v.rowid, \
                    COALESCE(u.nome, '—') \
             FROM vendas v LEFT JOIN usuarios u ON u.id = v.operador_id WHERE 1=1",
        );
        let mut args: Vec<Value> = Vec::new();

        if let Some(codigo) = filtro
            .codigo
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            // COO = rowid; também aceita prefixo do UUID. Tudo parametrizado.
            sql_query.push_str(" AND (v.id LIKE ? OR CAST(v.rowid AS TEXT) = ?)");
            args.push(Value::Text(format!("%{codigo}%")));
            args.push(Value::Text(codigo.to_string()));
        } else {
            if let Some(ini) = filtro
                .data_inicio
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty())
            {
                sql_query.push_str(" AND v.created_at >= ?");
                args.push(Value::Text(ini.to_string()));
            }
            if let Some(fim) = filtro
                .data_fim
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty())
            {
                sql_query.push_str(" AND v.created_at <= ?");
                args.push(Value::Text(fim.to_string()));
            }
            // Sem filtro de data: `sem_filtro_data` ou o chamador passou os limites explicitamente.
            // O antigo `date('now')` era UTC — usaria o dia errado entre 21h-0h no fuso de Brasília.
            // O filtro correto deve ser calculado pelo handler com o fuso IANA da loja_config.
            // Se nenhum filtro for fornecido e sem_filtro_data == false, retorna registros de hoje
            // em UTC como fallback conservador (evita retornar tabela inteira).
            if filtro.data_inicio.is_none() && filtro.data_fim.is_none() && !filtro.sem_filtro_data {
                sql_query.push_str(" AND date(v.created_at) = date('now')");
            }
        }

        sql_query.push_str(" ORDER BY v.created_at DESC, v.rowid DESC LIMIT ?");
        args.push(Value::Integer(filtro.limite));

        let mut stmt = self.conn.prepare(&sql_query)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(args.iter()),
            VendaResumo::da_linha,
        )?;

        let mut vendas = Vec::new();
        for row in rows {
            vendas.push(row?);
        }
        Ok(vendas)
    }

    fn carregar_itens(&self, venda_id: VendaId) -> InfraResult<Vec<ItemVenda>> {
        let mut stmt = self.conn.prepare(
            "SELECT produto_id, lote_id, quantidade, preco_unitario_centavos,
                    desconto_centavos, comissao_regra_id, comissao_centavos
             FROM venda_itens
             WHERE venda_id = ?1
             ORDER BY created_at ASC, id ASC",
        )?;
        let linhas = stmt.query_map(params![venda_id.to_string()], |row| {
            Ok(ItemBruto {
                produto_id: row.get(0)?,
                lote_id: row.get(1)?,
                quantidade: row.get(2)?,
                preco_unitario_centavos: row.get(3)?,
                desconto_centavos: row.get(4)?,
                comissao_regra_id: row.get(5)?,
                comissao_centavos: row.get(6)?,
            })
        })?;

        let mut itens = Vec::new();
        for linha in linhas {
            let bruto = linha?;
            itens.push(ItemVenda {
                produto_id: parse_id::<ProdutoId>(&bruto.produto_id, "venda_itens.produto_id")?,
                lote_id: bruto
                    .lote_id
                    .as_deref()
                    .map(|valor| parse_id::<LoteId>(valor, "venda_itens.lote_id"))
                    .transpose()?,
                quantidade: u32::try_from(bruto.quantidade)
                    .map_err(|erro| crate::error::erro_campo("venda_itens.quantidade", erro))?,
                preco_unitario: Money::from_centavos(bruto.preco_unitario_centavos),
                desconto_item: Money::from_centavos(bruto.desconto_centavos),
                comissao_regra_id: bruto.comissao_regra_id,
                comissao_valor: Money::from_centavos(bruto.comissao_centavos),
            });
        }
        Ok(itens)
    }

    fn carregar_pagamentos(&self, venda_id: VendaId) -> InfraResult<Vec<Pagamento>> {
        let mut stmt = self.conn.prepare(
            "SELECT forma, valor_centavos, recebido_centavos, forma_detalhe,
                    COALESCE(parcelas, 1), maquininha, modalidade, COALESCE(taxa_centavos, 0)
             FROM venda_pagamentos
             WHERE venda_id = ?1
             ORDER BY created_at ASC, id ASC",
        )?;
        let linhas = stmt.query_map(params![venda_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })?;

        let mut pagamentos = Vec::new();
        for linha in linhas {
            let (forma, valor, recebido, detalhe, parcelas, maquininha, modalidade, taxa_centavos) = linha?;
            let forma = FormaPagamento::from_db_com_detalhe(&forma, detalhe.as_deref())
                .map_err(|erro| crate::error::erro_campo("venda_pagamentos.forma", erro))?;
            pagamentos.push(Pagamento {
                forma,
                valor: Money::from_centavos(valor),
                recebido: recebido.map(Money::from_centavos),
                parcelas: (parcelas as u32).max(1),
                maquininha,
                modalidade,
                taxa: Money::from_centavos(taxa_centavos),
            });
        }
        Ok(pagamentos)
    }
}

impl VendaRepository for VendaRepo<'_> {
    fn salvar(&self, venda: &Venda) -> DomainResult<()> {
        if venda.pagamentos.is_empty() {
            return Err(DomainError::InvarianteViolada(
                "Venda sem pagamento não pode ser persistida".into(),
            ));
        }
        let total_pago = venda.total_pago();
        if total_pago != venda.total {
            return Err(DomainError::InvarianteViolada(format!(
                "Soma dos pagamentos ({total_pago}) difere do total da venda ({})",
                venda.total
            )));
        }

        sql(self.conn.execute(
            "INSERT INTO vendas
                 (id, sessao_id, dispositivo_id, operador_id, vendedor_id, comanda_id,
                  estado, forma_pagamento, subtotal_centavos, desconto_centavos,
                  total_centavos, recebido_centavos, troco_centavos, created_at,
                  forma_pagamento_detalhe)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                venda.id.to_string(),
                venda.sessao_id.to_string(),
                venda.dispositivo_id.to_string(),
                venda.operador_id.to_string(),
                venda.vendedor_id.map(|id| id.to_string()),
                venda.comanda_id,
                venda.estado.as_str(),
                venda.forma_pagamento.as_str(),
                venda.subtotal.centavos(),
                venda.desconto_total.centavos(),
                venda.total.centavos(),
                venda.recebido.centavos(),
                venda.troco.centavos(),
                data_hora_sql(venda.created_at),
                venda.forma_pagamento.as_detalhe(),
            ],
        ))?;

        for item in &venda.itens {
            let subtotal = item.subtotal()?;
            sql(self.conn.execute(
                "INSERT INTO venda_itens
                     (id, venda_id, produto_id, lote_id, quantidade,
                      preco_unitario_centavos, desconto_centavos, subtotal_centavos,
                      comissao_regra_id, comissao_centavos)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    venda.id.to_string(),
                    item.produto_id.to_string(),
                    item.lote_id.map(|id| id.to_string()),
                    item.quantidade,
                    item.preco_unitario.centavos(),
                    item.desconto_item.centavos(),
                    subtotal.centavos(),
                    item.comissao_regra_id,
                    item.comissao_valor.centavos(),
                ],
            ))?;
        }

        for pagamento in &venda.pagamentos {
            sql(self.conn.execute(
                "INSERT INTO venda_pagamentos
                     (id, venda_id, forma, valor_centavos, recebido_centavos, troco_centavos,
                      forma_detalhe, parcelas, maquininha, modalidade, taxa_centavos)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    venda.id.to_string(),
                    pagamento.forma.as_str(),
                    pagamento.valor.centavos(),
                    pagamento.recebido.map(|valor| valor.centavos()),
                    pagamento.troco().centavos(),
                    pagamento.forma.as_detalhe(),
                    pagamento.parcelas,
                    pagamento.maquininha,
                    pagamento.modalidade,
                    pagamento.taxa.centavos(),
                ],
            ))?;
        }

        Ok(())
    }

    fn buscar_por_id(&self, id: VendaId) -> DomainResult<Option<Venda>> {
        self.buscar_por_id_inner(id).dominio()
    }

    fn listar_por_sessao(
        &self,
        sessao_id: SessaoId,
        params: PageParams,
    ) -> DomainResult<Page<Venda>> {
        let cursor: Option<CursorVenda> = match params.cursor.as_deref() {
            Some(texto) => Some(
                serde_json::from_str(texto)
                    .map_err(|_| DomainError::Invalido("Cursor de vendas inválido".into()))?,
            ),
            None => None,
        };
        let limite = limit_seguro(&params);

        let mut stmt = sql(self.conn.prepare(
            "SELECT id, sessao_id, dispositivo_id, operador_id, vendedor_id, comanda_id,
                    estado, forma_pagamento, subtotal_centavos, desconto_centavos,
                    total_centavos, recebido_centavos, troco_centavos, created_at,
                    forma_pagamento_detalhe
             FROM vendas
             WHERE sessao_id = ?1
               AND (?2 IS NULL OR (created_at, id) < (?2, ?3))
             ORDER BY created_at DESC, id DESC
             LIMIT ?4",
        ))?;

        let linhas = sql(stmt.query_map(
            params![
                sessao_id.to_string(),
                cursor.as_ref().map(|c| c.created_at.as_str()),
                cursor.as_ref().map(|c| c.id.as_str()),
                limite
            ],
            VendaBruta::da_linha,
        ))?;

        let mut vendas = Vec::new();
        for linha in linhas {
            let bruto = sql(linha)?;
            let venda_id: VendaId = parse_id(&bruto.id, "vendas.id")?;
            let itens = self.carregar_itens(venda_id)?;
            let pagamentos = self.carregar_pagamentos(venda_id)?;
            vendas.push(bruto.para_venda(itens, pagamentos)?);
        }

        let next_cursor = if vendas.len() as i64 == limite {
            vendas.last().map(|ultima| {
                serde_json::to_string(&CursorVenda {
                    created_at: data_hora_sql(ultima.created_at),
                    id: ultima.id.to_string(),
                })
                .expect("serializar cursor de venda nunca falha")
            })
        } else {
            None
        };

        Ok(Page::new(vendas, next_cursor))
    }

    fn total_por_sessao(&self, sessao_id: SessaoId) -> DomainResult<i64> {
        let total: i64 = sql(self.conn.query_row(
            "SELECT COALESCE(SUM(total_centavos), 0)
             FROM vendas
             WHERE sessao_id = ?1 AND estado = 'FINALIZADA'",
            params![sessao_id.to_string()],
            |row| row.get(0),
        ))?;
        Ok(total)
    }

    fn atualizar_estado(&self, id: VendaId, estado: EstadoVenda) -> DomainResult<()> {
        let alteradas = sql(self.conn.execute(
            "UPDATE vendas SET estado = ?2 WHERE id = ?1",
            params![id.to_string(), estado.as_str()],
        ))?;
        if alteradas == 0 {
            return Err(DomainError::NaoEncontrado {
                entidade: "Venda",
                id: id.to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct CursorVenda {
    created_at: String,
    id: String,
}

struct ItemBruto {
    produto_id: String,
    lote_id: Option<String>,
    quantidade: i64,
    preco_unitario_centavos: i64,
    desconto_centavos: i64,
    comissao_regra_id: Option<String>,
    comissao_centavos: i64,
}

struct VendaBruta {
    id: String,
    sessao_id: String,
    dispositivo_id: String,
    operador_id: String,
    vendedor_id: Option<String>,
    comanda_id: Option<String>,
    estado: String,
    forma_pagamento: String,
    subtotal_centavos: i64,
    desconto_centavos: i64,
    total_centavos: i64,
    recebido_centavos: i64,
    troco_centavos: i64,
    created_at: String,
    forma_pagamento_detalhe: Option<String>,
}

impl VendaBruta {
    fn da_linha(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            sessao_id: row.get(1)?,
            dispositivo_id: row.get(2)?,
            operador_id: row.get(3)?,
            vendedor_id: row.get(4)?,
            comanda_id: row.get(5)?,
            estado: row.get(6)?,
            forma_pagamento: row.get(7)?,
            subtotal_centavos: row.get(8)?,
            desconto_centavos: row.get(9)?,
            total_centavos: row.get(10)?,
            recebido_centavos: row.get(11)?,
            troco_centavos: row.get(12)?,
            created_at: row.get(13)?,
            forma_pagamento_detalhe: row.get(14)?,
        })
    }

    fn para_venda(self, itens: Vec<ItemVenda>, pagamentos: Vec<Pagamento>) -> InfraResult<Venda> {
        let venda = Venda {
            id: parse_id::<VendaId>(&self.id, "vendas.id")?,
            sessao_id: parse_id::<SessaoId>(&self.sessao_id, "vendas.sessao_id")?,
            dispositivo_id: parse_id::<DispositivoId>(
                &self.dispositivo_id,
                "vendas.dispositivo_id",
            )?,
            operador_id: parse_id::<UsuarioId>(&self.operador_id, "vendas.operador_id")?,
            vendedor_id: self
                .vendedor_id
                .as_deref()
                .map(|valor| parse_id::<UsuarioId>(valor, "vendas.vendedor_id"))
                .transpose()?,
            comanda_id: self.comanda_id,
            estado: EstadoVenda::from_db(&self.estado)
                .map_err(|erro| crate::error::erro_campo("vendas.estado", erro))?,
            forma_pagamento: FormaPagamento::from_db_com_detalhe(
                &self.forma_pagamento,
                self.forma_pagamento_detalhe.as_deref(),
            )
            .map_err(|erro| crate::error::erro_campo("vendas.forma_pagamento", erro))?,
            pagamentos,
            itens,
            subtotal: Money::from_centavos(self.subtotal_centavos),
            desconto_total: Money::from_centavos(self.desconto_centavos),
            total: Money::from_centavos(self.total_centavos),
            recebido: Money::from_centavos(self.recebido_centavos),
            troco: Money::from_centavos(self.troco_centavos),
            created_at: parse_data_hora(&self.created_at, "vendas.created_at")?,
        };
        Ok(venda)
    }
}
