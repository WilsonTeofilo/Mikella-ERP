//! Read model de analytics de vendas para o dashboard (DSH-001).
//!
//! Entrega em uma única consulta o que os gráficos da Visão geral precisam:
//! série por dia, série por hora, mix de formas de pagamento, ranking de
//! produtos (com margem) e vendas por operador.
//!
//! Regras:
//! - Só vendas `FINALIZADA` (canceladas e estornadas ficam fora).
//! - O agrupamento por dia/hora é resolvido no fuso IANA da loja, não em UTC.
//! - O crediário é gravado como `forma = 'OUTRO'` + `forma_detalhe = 'CREDIARIO'`
//!   (migração 0006); o mix lê o detalhe para não esconder a venda a prazo.
//! - Nenhum cálculo de preço/margem nasce aqui: apenas agregação do que a venda
//!   registrou (custo congelado no lote quando existir).

use chrono::{DateTime, Duration, NaiveDate, Timelike};
use chrono_tz::Tz;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::InfraResult;
use crate::tempo::{fuso_ou_padrao, intervalo_utc_do_dia};

/// Ponto da série diária de faturamento.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerieDiaView {
    /// `AAAA-MM-DD` no fuso da loja.
    pub data: String,
    pub faturamento_centavos: i64,
    pub num_vendas: i64,
}

/// Ponto da série por hora do dia (0-23) no fuso da loja.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerieHoraView {
    pub hora: u32,
    pub faturamento_centavos: i64,
    pub num_vendas: i64,
}

/// Fatia do mix de formas de pagamento.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormaPagamentoView {
    /// Forma canônica: DINHEIRO, DEBITO, CREDITO, PIX, VOUCHER, CREDIARIO ou OUTRO.
    pub forma: String,
    pub valor_centavos: i64,
}

/// Produto vendido no período, com margem calculada a partir do custo congelado.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProdutoVendidoView {
    pub produto_id: String,
    pub nome: String,
    pub quantidade: i64,
    pub faturamento_centavos: i64,
    pub custo_centavos: i64,
    pub margem_centavos: i64,
    pub margem_pct: f64,
}

/// Desempenho por operador que finalizou a venda.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperadorVendasView {
    pub operador: String,
    pub faturamento_centavos: i64,
    pub num_vendas: i64,
}

/// Pacote completo consumido pelos gráficos do dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsVendasView {
    pub inicio: String,
    pub fim: String,
    pub faturamento_centavos: i64,
    pub num_vendas: i64,
    pub ticket_medio_centavos: i64,
    pub serie_dia: Vec<SerieDiaView>,
    pub serie_hora: Vec<SerieHoraView>,
    pub formas_pagamento: Vec<FormaPagamentoView>,
    pub top_produtos: Vec<ProdutoVendidoView>,
    pub operadores: Vec<OperadorVendasView>,
}

/// Converte o prefixo UTC `AAAA-MM-DDTHH` (saída de `strftime`) no dia e hora
/// locais da loja. Sem isso, uma venda às 22h em São Paulo cairia no dia
/// seguinte por causa do UTC.
fn dia_e_hora_locais(fuso: &str, prefixo_utc: &str) -> Option<(String, u32)> {
    let tz: Tz = fuso_ou_padrao(fuso);
    let texto = format!("{prefixo_utc}:00:00.000Z");
    let instante = DateTime::parse_from_rfc3339(&texto).ok()?;
    let local = instante.with_timezone(&tz);
    Some((local.format("%Y-%m-%d").to_string(), local.hour()))
}

pub struct AnalyticsVendasRepo<'a> {
    conn: &'a Connection,
}

impl<'a> AnalyticsVendasRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Agrega as vendas finalizadas entre `inicio` e `fim` (datas locais
    /// inclusivas, `AAAA-MM-DD`).
    pub fn obter(&self, fuso: &str, inicio: &str, fim: &str) -> InfraResult<AnalyticsVendasView> {
        let (inicio_utc, _) = intervalo_utc_do_dia(fuso, inicio)?;
        let (_, fim_utc) = intervalo_utc_do_dia(fuso, fim)?;

        let (faturamento_centavos, num_vendas): (i64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(total_centavos), 0), COUNT(*)
             FROM vendas
             WHERE estado = 'FINALIZADA' AND created_at >= ?1 AND created_at <= ?2",
            params![inicio_utc, fim_utc],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let ticket_medio_centavos = if num_vendas > 0 {
            faturamento_centavos / num_vendas
        } else {
            0
        };

        let mut dias: Vec<(String, i64, i64)> = Vec::new();
        let mut horas: Vec<(u32, i64, i64)> = (0..24).map(|h| (h, 0, 0)).collect();

        {
            let mut stmt = self.conn.prepare(
                "SELECT strftime('%Y-%m-%dT%H', created_at) AS hora_utc,
                        COALESCE(SUM(total_centavos), 0),
                        COUNT(*)
                 FROM vendas
                 WHERE estado = 'FINALIZADA' AND created_at >= ?1 AND created_at <= ?2
                   AND hora_utc IS NOT NULL
                 GROUP BY hora_utc",
            )?;
            let buckets = stmt
                .query_map(params![inicio_utc, fim_utc], |row| {
                    let prefixo: String = row.get(0)?;
                    let valor: i64 = row.get(1)?;
                    let qtd: i64 = row.get(2)?;
                    Ok((prefixo, valor, qtd))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            for (prefixo, valor, qtd) in buckets {
                let Some((dia, hora)) = dia_e_hora_locais(fuso, &prefixo) else {
                    continue;
                };
                if let Some(item) = dias.iter_mut().find(|(d, _, _)| d == &dia) {
                    item.1 += valor;
                    item.2 += qtd;
                } else {
                    dias.push((dia, valor, qtd));
                }
                if let Some(item) = horas.iter_mut().find(|(h, _, _)| *h == hora) {
                    item.1 += valor;
                    item.2 += qtd;
                }
            }
        }

        // Garante série contínua (dias sem venda com zero) para o gráfico não
        // encurtar a linha do tempo em semanas fracas.
        dias.sort_by(|a, b| a.0.cmp(&b.0));
        let mut serie_dia = Vec::new();
        let inicio_d = NaiveDate::parse_from_str(inicio, "%Y-%m-%d")
            .map_err(|_| crate::error::erro_campo("inicio", "use AAAA-MM-DD"))?;
        let fim_d = NaiveDate::parse_from_str(fim, "%Y-%m-%d")
            .map_err(|_| crate::error::erro_campo("fim", "use AAAA-MM-DD"))?;
        let mut cursor = inicio_d;
        while cursor <= fim_d && serie_dia.len() < 366 {
            let chave = cursor.to_string();
            let (_, valor, qtd) = dias
                .iter()
                .find(|(d, _, _)| d == &chave)
                .map(|(d, v, q)| (d.clone(), *v, *q))
                .unwrap_or((chave.clone(), 0, 0));
            serie_dia.push(SerieDiaView {
                data: chave,
                faturamento_centavos: valor,
                num_vendas: qtd,
            });
            cursor += Duration::days(1);
        }

        let serie_hora = horas
            .into_iter()
            .map(|(hora, faturamento_centavos, num_vendas)| SerieHoraView {
                hora,
                faturamento_centavos,
                num_vendas,
            })
            .collect();

        // Mix de pagamento: `forma_detalhe` separa VOUCHER/CREDIARIO de OUTRO.
        let formas_pagamento: Vec<FormaPagamentoView>;
        {
            let mut stmt = self.conn.prepare(
                "SELECT COALESCE(NULLIF(vp.forma_detalhe, ''), vp.forma) AS forma,
                        COALESCE(SUM(vp.valor_centavos), 0)
                 FROM venda_pagamentos vp
                 JOIN vendas v ON v.id = vp.venda_id
                 WHERE v.estado = 'FINALIZADA' AND v.created_at >= ?1 AND v.created_at <= ?2
                 GROUP BY forma
                 ORDER BY 2 DESC",
            )?;
            formas_pagamento = stmt
                .query_map(params![inicio_utc, fim_utc], |row| {
                    Ok(FormaPagamentoView {
                        forma: row.get(0)?,
                        valor_centavos: row.get(1)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
        }

        // Ranking de produtos. A margem usa o custo congelado no lote (quando a
        // venda baixou lote) ou o custo atual do produto, igual ao CMV do DRE.
        let top_produtos: Vec<ProdutoVendidoView>;
        {
            let mut stmt = self.conn.prepare(
                "SELECT p.id, p.nome,
                        COALESCE(SUM(vi.quantidade), 0) AS quantidade,
                        COALESCE(SUM(vi.subtotal_centavos), 0) AS faturamento,
                        COALESCE(SUM(COALESCE(l.custo_centavos, p.custo_centavos, 0) * vi.quantidade), 0) AS custo
                 FROM venda_itens vi
                 JOIN vendas v ON v.id = vi.venda_id
                 JOIN produtos p ON p.id = vi.produto_id
                 LEFT JOIN lotes l ON l.id = vi.lote_id
                 WHERE v.estado = 'FINALIZADA' AND v.created_at >= ?1 AND v.created_at <= ?2
                 GROUP BY p.id, p.nome
                 ORDER BY faturamento DESC
                 LIMIT 200",
            )?;
            top_produtos = stmt
                .query_map(params![inicio_utc, fim_utc], |row| {
                    let faturamento_centavos: i64 = row.get(3)?;
                    let custo_centavos: i64 = row.get(4)?;
                    let margem_centavos = faturamento_centavos.saturating_sub(custo_centavos);
                    let margem_pct = if faturamento_centavos > 0 {
                        (margem_centavos as f64 / faturamento_centavos as f64) * 100.0
                    } else {
                        0.0
                    };
                    Ok(ProdutoVendidoView {
                        produto_id: row.get(0)?,
                        nome: row.get(1)?,
                        quantidade: row.get(2)?,
                        faturamento_centavos,
                        custo_centavos,
                        margem_centavos,
                        margem_pct,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
        }

        let operadores: Vec<OperadorVendasView>;
        {
            let mut stmt = self.conn.prepare(
                "SELECT u.username, COALESCE(SUM(v.total_centavos), 0), COUNT(*)
                 FROM vendas v
                 JOIN usuarios u ON u.id = v.operador_id
                 WHERE v.estado = 'FINALIZADA' AND v.created_at >= ?1 AND v.created_at <= ?2
                 GROUP BY u.id, u.username
                 ORDER BY 2 DESC",
            )?;
            operadores = stmt
                .query_map(params![inicio_utc, fim_utc], |row| {
                    Ok(OperadorVendasView {
                        operador: row.get(0)?,
                        faturamento_centavos: row.get(1)?,
                        num_vendas: row.get(2)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
        }

        Ok(AnalyticsVendasView {
            inicio: inicio.to_string(),
            fim: fim.to_string(),
            faturamento_centavos,
            num_vendas,
            ticket_medio_centavos,
            serie_dia,
            serie_hora,
            formas_pagamento,
            top_produtos,
            operadores,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate;

    fn banco_com_venda() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate::aplicar(&mut conn).unwrap();
        conn.execute_batch(
            "INSERT INTO usuarios (id, nome, username, senha_hash, papel)
             VALUES ('u1', 'Operador', 'op', 'x', 'OPERADOR_CAIXA');
             INSERT INTO dispositivos (id, nome, usuario_so, fingerprint, papel)
             VALUES ('d1', 'PC', 'op', 'f', 'CAIXA');
             INSERT INTO sessoes_caixa (id, numero_caixa, dispositivo_id, operador_id)
             VALUES ('s1', 1, 'd1', 'u1');
             INSERT INTO produtos (id, nome, preco_centavos, custo_centavos)
             VALUES ('p1', 'Cafe', 1000, 400), ('p2', 'Pao', 500, 100);
             INSERT INTO vendas (id, sessao_id, dispositivo_id, operador_id,
                                 forma_pagamento, subtotal_centavos, total_centavos,
                                 created_at, forma_pagamento_detalhe)
             VALUES ('v1', 's1', 'd1', 'u1', 'OUTRO', 1500, 1500,
                     '2026-11-13T15:00:00.000Z', 'CREDIARIO'),
                    ('v2', 's1', 'd1', 'u1', 'PIX', 1000, 1000,
                     '2026-11-13T16:30:00.000Z', NULL);
             INSERT INTO venda_itens (id, venda_id, produto_id, quantidade,
                                      preco_unitario_centavos, subtotal_centavos)
             VALUES ('i1', 'v1', 'p1', 1, 1000, 1000),
                    ('i2', 'v1', 'p2', 1, 500, 500),
                    ('i3', 'v2', 'p1', 1, 1000, 1000);
             INSERT INTO venda_pagamentos (id, venda_id, forma, valor_centavos,
                                           recebido_centavos, forma_detalhe)
             VALUES ('pg1', 'v1', 'OUTRO', 1500, NULL, 'CREDIARIO'),
                    ('pg2', 'v2', 'PIX', 1000, NULL, NULL);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn agrega_series_e_separa_crediario_do_outro() {
        let conn = banco_com_venda();
        let repo = AnalyticsVendasRepo::novo(&conn);
        let dados = repo
            .obter("America/Sao_Paulo", "2026-11-13", "2026-11-13")
            .unwrap();

        assert_eq!(dados.faturamento_centavos, 2500);
        assert_eq!(dados.num_vendas, 2);
        assert_eq!(dados.serie_dia.len(), 1);
        assert_eq!(dados.serie_dia[0].faturamento_centavos, 2500);
        // 15:00Z e 16:30Z viram 12h e 13h em São Paulo (UTC-3).
        assert_eq!(dados.serie_hora[12].faturamento_centavos, 1500);
        assert_eq!(dados.serie_hora[13].faturamento_centavos, 1000);

        let crediario = dados
            .formas_pagamento
            .iter()
            .find(|f| f.forma == "CREDIARIO")
            .map(|f| f.valor_centavos);
        assert_eq!(crediario, Some(1500));

        assert_eq!(dados.top_produtos[0].nome, "Cafe");
        assert_eq!(dados.top_produtos[0].faturamento_centavos, 2000);
        assert_eq!(dados.top_produtos[0].custo_centavos, 800);
        assert_eq!(dados.top_produtos[0].margem_centavos, 1200);
        assert_eq!(dados.operadores.len(), 1);
        assert_eq!(dados.operadores[0].num_vendas, 2);
    }

    #[test]
    fn dia_sem_venda_aparece_zerado() {
        let conn = banco_com_venda();
        let repo = AnalyticsVendasRepo::novo(&conn);
        let dados = repo
            .obter("America/Sao_Paulo", "2026-11-12", "2026-11-14")
            .unwrap();

        assert_eq!(dados.serie_dia.len(), 3);
        assert_eq!(dados.serie_dia[0].data, "2026-11-12");
        assert_eq!(dados.serie_dia[0].faturamento_centavos, 0);
        assert_eq!(dados.serie_dia[2].data, "2026-11-14");
    }
}
