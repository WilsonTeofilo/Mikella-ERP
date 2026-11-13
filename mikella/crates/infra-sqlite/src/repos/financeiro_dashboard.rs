//! Read models e repositório de métricas financeiras e dashboard do dono (DSH-001 a DSH-024).
//!
//! Responde às 3 perguntas clássicas do dono de pequeno varejo:
//! 1. Quanto entrou, quanto sobra? (Faturamento, CMV congelado, margem bruta, ponto de equilíbrio)
//! 2. O que preciso pagar e receber? (A pagar 7d/vencidas, a receber 7d, crediário, saldo projetado 7d)
//! 3. O que precisa da minha atenção hoje? (Lotes em risco de validade EST-01, quebras de caixa CTL-01, maiores devedores FIN-26, status de backup)
//!
//! Além disso provê:
//! - DRE simplificada (Regime de Competência - 7 linhas)
//! - Extrato diário de Fluxo de Caixa (Regime de Caixa)
//!
//! Toda agregação é 100% no SQLite com queries indexadas (zero sobrecarga de memória para PC fraco).

use chrono::{Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::InfraResult;
use crate::tempo::{
    data_hora_sql, data_local_iso, data_local_mais_dias, hoje_local, intervalo_utc_do_mes,
    mes_local,
};

/// Converte uma janela local (`AAAA-MM-DD`, inclusiva) no fuso IANA da loja
/// para o intervalo UTC fechado usado nas comparações com `created_at`.
///
/// O `date('now')` do SQLite é UTC: entre 21h e 0h em `America/Sao_Paulo` ele
/// apontaria para o dia seguinte. Comparar o instante UTC com o intervalo
/// calculado no fuso da loja elimina esse desvio.
fn intervalo_utc_da_janela(
    fuso: &str,
    data_inicio: &str,
    data_fim: &str,
) -> InfraResult<(String, String)> {
    let tz: Tz = fuso.parse().unwrap_or(chrono_tz::America::Sao_Paulo);
    let inicio = NaiveDate::parse_from_str(data_inicio.trim(), "%Y-%m-%d")
        .map_err(|_| crate::error::erro_campo("data_inicio", "use AAAA-MM-DD"))?;
    let fim = NaiveDate::parse_from_str(data_fim.trim(), "%Y-%m-%d")
        .map_err(|_| crate::error::erro_campo("data_fim", "use AAAA-MM-DD"))?;

    let inicio_local = tz
        .from_local_datetime(&inicio.and_time(NaiveTime::MIN))
        .single()
        .ok_or_else(|| crate::error::erro_campo("data_inicio", "horário inexistente no fuso"))?
        .with_timezone(&Utc);
    let fim_do_dia = NaiveTime::from_hms_milli_opt(23, 59, 59, 999)
        .ok_or_else(|| crate::error::erro_campo("data_fim", "horário inválido"))?;
    let fim_local = tz
        .from_local_datetime(&fim.and_time(fim_do_dia))
        .single()
        .ok_or_else(|| crate::error::erro_campo("data_fim", "horário inexistente no fuso"))?
        .with_timezone(&Utc);

    Ok((data_hora_sql(inicio_local), data_hora_sql(fim_local)))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemAtencaoView {
    pub tipo: String, // "VALIDADE", "QUEBRA_CAIXA", "DEVEDOR", "BACKUP"
    pub titulo: String,
    pub subtitulo: String,
    pub valor_risco_centavos: i64,
    pub acao_rotulo: String,
    pub link_destino: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovimentoFitaView {
    pub tipo: String, // "VENDA", "CAIXA", "RECEBIMENTO"
    pub descricao: String,
    pub valor_centavos: i64,
    pub detalhe: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntradaDetalhadaView {
    pub id: String,
    pub coo: Option<i64>,
    pub created_at: String,
    pub cliente_nome: String,
    pub forma_pagamento: String,
    pub valor_bruto_centavos: i64,
    pub lucro_liquido_centavos: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumoDashboardView {
    pub periodo: String, // "hoje", "semana", "mes"
    pub faturamento_centavos: i64,
    pub cmv_centavos: i64,
    pub lucro_bruto_centavos: i64,
    pub margem_bruta_pct: f64,
    pub num_vendas: i64,
    pub ticket_medio_centavos: i64,
    pub comparativo_faturamento_pct: Option<f64>,
    pub ponto_equilibrio_centavos: i64,
    pub cobertura_ponto_equilibrio_pct: f64,
    pub a_pagar_7d_centavos: i64,
    pub a_pagar_vencidas_centavos: i64,
    pub num_contas_pagar_vencidas: i64,
    pub a_receber_7d_centavos: i64,
    pub crediario_vencido_centavos: i64,
    pub saldo_caixa_dinheiro_centavos: i64,
    pub saldo_projetado_7d_centavos: i64,
    pub total_pix_centavos: i64,
    pub total_cartao_centavos: i64,
    pub total_dinheiro_centavos: i64,
    pub total_crediario_centavos: i64,
    pub total_outros_centavos: i64,
    pub entradas_detalhadas: Vec<EntradaDetalhadaView>,
    pub atencao_itens: Vec<ItemAtencaoView>,
    pub fita_do_dia: Vec<MovimentoFitaView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiaFluxoCaixaView {
    pub data: String, // "AAAA-MM-DD"
    pub entradas_centavos: i64,
    pub saidas_centavos: i64,
    pub saldo_dia_centavos: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FluxoCaixaView {
    pub dias: Vec<DiaFluxoCaixaView>,
    pub saldo_atual_centavos: i64,
    pub total_entradas_periodo_centavos: i64,
    pub total_saidas_periodo_centavos: i64,
    pub projecao_30d_centavos: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultadoDREView {
    pub periodo: String, // "AAAA-MM"
    pub receita_bruta_centavos: i64,
    pub descontos_centavos: i64,
    pub receita_liquida_centavos: i64,
    pub cmv_centavos: i64,
    pub lucro_bruto_centavos: i64,
    pub custos_variaveis_centavos: i64,
    pub margem_contribuicao_centavos: i64,
    pub margem_contribuicao_pct: f64,
    pub despesas_fixas_centavos: i64,
    pub resultado_operacional_centavos: i64,
    pub impostos_estimados_centavos: i64,
    pub resultado_periodo_centavos: i64,
    pub retiradas_dono_centavos: i64,
}

pub struct FinanceiroDashboardRepo<'a> {
    conn: &'a Connection,
}

impl<'a> FinanceiroDashboardRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Obtém o resumo executivo do dono respondendo às 3 perguntas clássicas.
    pub fn obter_resumo(
        &self,
        periodo: &str,
        data_inicio: Option<&str>,
        data_fim: Option<&str>,
        fuso: &str,
    ) -> InfraResult<ResumoDashboardView> {
        let (inicio_local, fim_local, comp_inicio_local, comp_fim_local) =
            if let Some(ini) = data_inicio {
                let fim = data_fim.unwrap_or(ini);
                (
                    ini.to_string(),
                    fim.to_string(),
                    (NaiveDate::parse_from_str(ini.trim(), "%Y-%m-%d")
                        .map_err(|_| crate::error::erro_campo("data_inicio", "use AAAA-MM-DD"))?
                        - Duration::days(30))
                    .to_string(),
                    ini.to_string(),
                )
            } else {
                // Fallback sem data explícita: resolve o "hoje" no fuso da loja.
                let tz: Tz = fuso.parse().unwrap_or(chrono_tz::America::Sao_Paulo);
                let hoje = Utc::now().with_timezone(&tz).date_naive();
                match periodo {
                    "semana" => (
                        (hoje - Duration::days(6)).to_string(),
                        hoje.to_string(),
                        (hoje - Duration::days(13)).to_string(),
                        (hoje - Duration::days(6)).to_string(),
                    ),
                    "mes" => {
                        let primeiro = hoje.with_day(1).unwrap_or(hoje);
                        let anterior = primeiro - Duration::days(1);
                        (
                            primeiro.to_string(),
                            hoje.to_string(),
                            anterior.with_day(1).unwrap_or(anterior).to_string(),
                            primeiro.to_string(),
                        )
                    }
                    _ => (
                        hoje.to_string(),
                        hoje.to_string(),
                        (hoje - Duration::days(1)).to_string(),
                        hoje.to_string(),
                    ),
                }
            };

        let (inicio_utc, fim_utc) = intervalo_utc_da_janela(fuso, &inicio_local, &fim_local)?;
        let (comp_inicio_utc, comp_fim_utc) =
            intervalo_utc_da_janela(fuso, &comp_inicio_local, &comp_fim_local)?;

        // Datas locais para os vencimentos. `date('now')` do SQLite é UTC e
        // erraria o dia entre 21h e 0h no fuso da loja (TZ-001).
        let hoje = hoje_local(fuso);
        let hoje_s = hoje.to_string();
        let em_7_dias = (hoje + Duration::days(7)).to_string();
        let em_60_dias = (hoje + Duration::days(60)).to_string();
        let inicio_mes = hoje.with_day(1).unwrap_or(hoje);
        let proximo_mes = if inicio_mes.month() == 12 {
            NaiveDate::from_ymd_opt(inicio_mes.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(inicio_mes.year(), inicio_mes.month() + 1, 1)
        }
        .unwrap_or(inicio_mes);
        let inicio_mes_s = inicio_mes.to_string();
        let proximo_mes_s = proximo_mes.to_string();

        // 1. Faturamento e quantidade de vendas
        let sql_vendas = "SELECT COALESCE(SUM(total_centavos), 0), COUNT(*)
             FROM vendas
             WHERE estado = 'FINALIZADA'
               AND created_at >= ?1
               AND created_at <= ?2";
        let (faturamento_centavos, num_vendas): (i64, i64) =
            self.conn
                .query_row(sql_vendas, params![inicio_utc, fim_utc], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?;

        let ticket_medio_centavos = if num_vendas > 0 {
            faturamento_centavos / num_vendas
        } else {
            0
        };

        // Comparativo com período anterior
        let sql_comp = "SELECT COALESCE(SUM(total_centavos), 0)
             FROM vendas
             WHERE estado = 'FINALIZADA'
               AND created_at >= ?1
               AND created_at < ?2";
        let faturamento_anterior: i64 =
            self.conn
                .query_row(sql_comp, params![comp_inicio_utc, comp_fim_utc], |row| {
                    row.get(0)
                })?;
        let comparativo_faturamento_pct = if faturamento_anterior > 0 {
            Some(
                ((faturamento_centavos as f64 - faturamento_anterior as f64)
                    / faturamento_anterior as f64)
                    * 100.0,
            )
        } else {
            None
        };

        // 2. Custo da mercadoria vendida (CMV) congelado / snapshot
        let sql_cmv = "SELECT COALESCE(SUM(
                 COALESCE(l.custo_centavos, p.custo_centavos, 0) * vi.quantidade
             ), 0)
             FROM venda_itens vi
             JOIN vendas v ON v.id = vi.venda_id
             LEFT JOIN lotes l ON l.id = vi.lote_id
             LEFT JOIN produtos p ON p.id = vi.produto_id
             WHERE v.estado = 'FINALIZADA'
               AND v.created_at >= ?1
               AND v.created_at <= ?2";
        let cmv_centavos: i64 =
            self.conn
                .query_row(sql_cmv, params![inicio_utc, fim_utc], |row| row.get(0))?;
        let lucro_bruto_centavos = faturamento_centavos.saturating_sub(cmv_centavos);
        let margem_bruta_pct = if faturamento_centavos > 0 {
            (lucro_bruto_centavos as f64 / faturamento_centavos as f64) * 100.0
        } else {
            0.0
        };

        // 3. Ponto de Equilíbrio mensal (despesas fixas / margem)
        let sql_despesas_fixas = "SELECT COALESCE(SUM(p.valor_centavos), 0)
             FROM parcelas p
             JOIN contas_pagar cp ON cp.id = p.conta_id
             WHERE p.conta_tipo = 'PAGAMENTO'
               AND (cp.plano_codigo IN ('2.1.02', '4.1.02', '2.1.01')
                    OR cp.descricao LIKE '%Aluguel%'
                    OR cp.descricao LIKE '%Salário%'
                    OR cp.descricao LIKE '%Energia%')
               AND date(p.vencimento) >= ?1
               AND date(p.vencimento) < ?2";
        let despesas_fixas_centavos: i64 = self.conn.query_row(
            sql_despesas_fixas,
            params![inicio_mes_s, proximo_mes_s],
            |row| row.get(0),
        )?;

        let margem_calculo = if margem_bruta_pct > 5.0 {
            margem_bruta_pct / 100.0
        } else {
            0.30 // margem de referência padrão de comércio (30%) se sem dados suficientes
        };

        let ponto_equilibrio_centavos = if despesas_fixas_centavos > 0 {
            (despesas_fixas_centavos as f64 / margem_calculo) as i64
        } else {
            0
        };

        let cobertura_ponto_equilibrio_pct = if ponto_equilibrio_centavos > 0 {
            (faturamento_centavos as f64 / ponto_equilibrio_centavos as f64) * 100.0
        } else {
            0.0
        };

        // 4. Contas a pagar e receber (7 dias e vencidas)
        let a_pagar_7d_centavos: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(valor_centavos), 0)
             FROM parcelas
             WHERE conta_tipo = 'PAGAMENTO' AND status = 'ABERTA'
               AND date(vencimento) >= ?1
               AND date(vencimento) <= ?2",
            params![hoje_s, em_7_dias],
            |row| row.get(0),
        )?;

        let (a_pagar_vencidas_centavos, num_contas_pagar_vencidas): (i64, i64) =
            self.conn.query_row(
                "SELECT COALESCE(SUM(valor_centavos), 0), COUNT(*)
             FROM parcelas
             WHERE conta_tipo = 'PAGAMENTO' AND status = 'ABERTA'
               AND date(vencimento) < ?1",
                params![hoje_s],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;

        let a_receber_7d_centavos: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(valor_centavos), 0)
             FROM parcelas
             WHERE conta_tipo = 'RECEBIMENTO' AND status = 'ABERTA'
               AND date(vencimento) >= ?1
               AND date(vencimento) <= ?2",
            params![hoje_s, em_7_dias],
            |row| row.get(0),
        )?;

        let crediario_vencido_centavos: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(valor_centavos), 0)
             FROM parcelas
             WHERE conta_tipo = 'RECEBIMENTO' AND status = 'ABERTA'
               AND date(vencimento) < ?1",
            params![hoje_s],
            |row| row.get(0),
        )?;

        let saldo_caixa_dinheiro_centavos: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(saldo_esperado_centavos), 0)
             FROM sessoes_caixa
             WHERE estado = 'ABERTA'",
            [],
            |row| row.get(0),
        )?;

        let saldo_projetado_7d_centavos =
            saldo_caixa_dinheiro_centavos + a_receber_7d_centavos - a_pagar_7d_centavos;

        // 5. Selos de atenção (máximo 3, priorizados por valor e urgência)
        let mut atencao_itens = Vec::new();

        // 5.1 Alerta de validade (EST-01)
        let mut stmt_val = self.conn.prepare(
            "SELECT p.nome,
                    COALESCE(l.numero_lote, 'Padrão') as lote_num,
                    l.validade,
                    CAST(julianday(l.validade) - julianday(?1) AS INTEGER) as dias_restantes,
                    l.quantidade * COALESCE(l.custo_centavos, p.custo_centavos, 0) as valor_risco
             FROM lotes l
             JOIN produtos p ON p.id = l.produto_id
             WHERE l.ativo = 1 AND l.quantidade > 0 AND l.validade IS NOT NULL
               AND date(l.validade) <= ?2
             ORDER BY date(l.validade) ASC, valor_risco DESC
             LIMIT 1",
        )?;
        let item_validade = stmt_val
            .query_row(params![hoje_s, em_60_dias], |row| {
                let p_nome: String = row.get(0)?;
                let lote_num: String = row.get(1)?;
                let _validade: String = row.get(2)?;
                let dias_restantes: i64 = row.get(3)?;
                let valor_risco: i64 = row.get(4)?;
                let titulo = if dias_restantes < 0 {
                    "LOTE VENCIDO".to_string()
                } else {
                    format!("VENCE EM {} DIAS", dias_restantes)
                };
                Ok(ItemAtencaoView {
                    tipo: "VALIDADE".to_string(),
                    titulo,
                    subtitulo: format!("{}, lote {}", p_nome, lote_num),
                    valor_risco_centavos: valor_risco,
                    acao_rotulo: "Ver Estoque".to_string(),
                    link_destino: "produtos.html".to_string(),
                })
            })
            .ok();
        if let Some(item) = item_validade {
            atencao_itens.push(item);
        }

        // 5.2 Diferença de caixa (CTL-01)
        let mut stmt_quebra = self.conn.prepare(
            "SELECT sc.numero_caixa, u.username, sc.fechada_em, sc.diferenca_centavos
             FROM sessoes_caixa sc
             JOIN usuarios u ON u.id = sc.operador_id
             WHERE sc.estado = 'FECHADA' AND sc.diferenca_centavos != 0
             ORDER BY sc.fechada_em DESC
             LIMIT 1",
        )?;
        let item_quebra = stmt_quebra
            .query_row([], |row| {
                let numero_caixa: i64 = row.get(0)?;
                let _operador: String = row.get(1)?;
                let _fechada_em: Option<String> = row.get(2)?;
                let diferenca: i64 = row.get(3)?;
                let sinal = if diferenca > 0 { "+" } else { "" };
                Ok(ItemAtencaoView {
                    tipo: "QUEBRA_CAIXA".to_string(),
                    titulo: format!("CAIXA {:02}", numero_caixa),
                    subtitulo: format!("Diferença {}R$ {:.2}", sinal, (diferenca as f64) / 100.0),
                    valor_risco_centavos: diferenca.abs(),
                    acao_rotulo: "Ver Caixas".to_string(),
                    link_destino: "pos-menu.html".to_string(),
                })
            })
            .ok();
        if let Some(item) = item_quebra {
            atencao_itens.push(item);
        }

        // 5.3 Maiores devedores de crediário (FIN-26)
        let mut stmt_dev = self.conn.prepare(
            "SELECT COALESCE(p.nome, 'Cliente') as cliente_nome,
                    SUM(par.valor_centavos) as divida_total,
                    CAST(julianday(?1) - julianday(MIN(date(par.vencimento))) AS INTEGER) as dias_atraso
             FROM parcelas par
             JOIN contas_receber cr ON cr.id = par.conta_id
             LEFT JOIN pessoas p ON p.id = cr.pessoa_id
             WHERE par.conta_tipo = 'RECEBIMENTO' AND par.status = 'ABERTA' AND date(par.vencimento) < ?1
             GROUP BY cr.pessoa_id
             ORDER BY divida_total DESC
             LIMIT 1",
        )?;
        let item_devedor = stmt_dev
            .query_row(params![hoje_s], |row| {
                let cliente_nome: String = row.get(0)?;
                let divida_total: i64 = row.get(1)?;
                let dias_atraso: i64 = row.get(2)?;
                Ok(ItemAtencaoView {
                    tipo: "DEVEDOR".to_string(),
                    titulo: "CREDIÁRIO VENCIDO".to_string(),
                    subtitulo: format!("{}, {} dias de atraso", cliente_nome, dias_atraso),
                    valor_risco_centavos: divida_total,
                    acao_rotulo: "Cobrar".to_string(),
                    link_destino: "financeiro.html?aba=receber".to_string(),
                })
            })
            .ok();
        if let Some(item) = item_devedor {
            atencao_itens.push(item);
        }

        // 5.4 Se couber, status do backup (CTL-05)
        if atencao_itens.len() < 3 {
            let mut stmt_bkp = self.conn.prepare(
                "SELECT created_at FROM auditoria WHERE acao = 'sistema.backup' ORDER BY created_at DESC LIMIT 1"
            )?;
            let ultimo_bkp: Option<String> = stmt_bkp.query_row([], |row| row.get(0)).ok();
            let (titulo, subtitulo) = match ultimo_bkp {
                Some(dt) => (
                    "BACKUP EM DIA".to_string(),
                    format!("Última cópia: {}", &dt[..10.min(dt.len())]),
                ),
                None => (
                    "CÓPIA DE SEGURANÇA".to_string(),
                    "Nenhum backup local gerado ainda".to_string(),
                ),
            };
            atencao_itens.push(ItemAtencaoView {
                tipo: "BACKUP".to_string(),
                titulo,
                subtitulo,
                valor_risco_centavos: 0,
                acao_rotulo: "Gerar Backup".to_string(),
                link_destino: "financeiro.html".to_string(),
            });
        }

        atencao_itens.truncate(3);

        // 6. Fita do dia: últimas movimentações (mais recente em cima)
        let mut stmt_fita = self.conn.prepare(
            "SELECT 'VENDA' as tipo,
                    'Venda #' || SUBSTR(id, 1, 8) as descricao,
                    total_centavos as valor_centavos,
                    forma_pagamento as detalhe,
                    created_at
             FROM vendas
             WHERE estado = 'FINALIZADA'
             UNION ALL
             SELECT 'CAIXA' as tipo,
                    tipo || ' (Caixa)' as descricao,
                    valor_centavos,
                    COALESCE(motivo, '') as detalhe,
                    created_at
             FROM livro_caixa
             WHERE tipo IN ('SANGRIA', 'SUPRIMENTO', 'ABERTURA', 'FECHAMENTO')
             ORDER BY created_at DESC
             LIMIT 15",
        )?;
        let fita_do_dia = stmt_fita
            .query_map([], |row| {
                Ok(MovimentoFitaView {
                    tipo: row.get(0)?,
                    descricao: row.get(1)?,
                    valor_centavos: row.get(2)?,
                    detalhe: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // 7. Breakdown Chokko-Melt por método de pagamento no período
        let sql_pix = "SELECT COALESCE(SUM(vp.valor_centavos), 0)
             FROM venda_pagamentos vp
             JOIN vendas v ON v.id = vp.venda_id
             WHERE v.estado = 'FINALIZADA' AND vp.forma = 'PIX'
               AND v.created_at >= ?1
               AND v.created_at <= ?2";
        let total_pix_centavos: i64 = self
            .conn
            .query_row(sql_pix, params![inicio_utc, fim_utc], |row| row.get(0))
            .unwrap_or(0);

        let sql_cartao = "SELECT COALESCE(SUM(vp.valor_centavos), 0)
             FROM venda_pagamentos vp
             JOIN vendas v ON v.id = vp.venda_id
             WHERE v.estado = 'FINALIZADA' AND vp.forma IN ('DEBITO', 'CREDITO')
               AND v.created_at >= ?1
               AND v.created_at <= ?2";
        let total_cartao_centavos: i64 = self
            .conn
            .query_row(sql_cartao, params![inicio_utc, fim_utc], |row| row.get(0))
            .unwrap_or(0);

        let sql_dinheiro = "SELECT COALESCE(SUM(vp.valor_centavos), 0)
             FROM venda_pagamentos vp
             JOIN vendas v ON v.id = vp.venda_id
             WHERE v.estado = 'FINALIZADA' AND vp.forma = 'DINHEIRO'
               AND v.created_at >= ?1
               AND v.created_at <= ?2";
        let total_dinheiro_centavos: i64 = self
            .conn
            .query_row(sql_dinheiro, params![inicio_utc, fim_utc], |row| row.get(0))
            .unwrap_or(0);

        let sql_crediario = "SELECT COALESCE(SUM(vp.valor_centavos), 0)
             FROM venda_pagamentos vp
             JOIN vendas v ON v.id = vp.venda_id
             WHERE v.estado = 'FINALIZADA' AND vp.forma = 'CREDIARIO'
               AND v.created_at >= ?1
               AND v.created_at <= ?2";
        let total_crediario_centavos: i64 = self
            .conn
            .query_row(sql_crediario, params![inicio_utc, fim_utc], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        let total_outros_centavos = faturamento_centavos.saturating_sub(
            total_pix_centavos
                + total_cartao_centavos
                + total_dinheiro_centavos
                + total_crediario_centavos,
        );

        // 8. Detalhamento de Entradas (Vendas Chokko-Melt)
        let sql_entradas =
            "SELECT v.id, v.created_at, v.forma_pagamento, v.total_centavos,
                    v.total_centavos - COALESCE((
                        SELECT SUM(COALESCE(l.custo_centavos, prod.custo_centavos, 0) * vi.quantidade)
                        FROM venda_itens vi
                        LEFT JOIN lotes l ON l.id = vi.lote_id
                        LEFT JOIN produtos prod ON prod.id = vi.produto_id
                        WHERE vi.venda_id = v.id
                    ), 0) as lucro
             FROM vendas v
             WHERE v.estado = 'FINALIZADA'
               AND v.created_at >= ?1
               AND v.created_at <= ?2
             ORDER BY v.created_at DESC
             LIMIT 50";
        let mut stmt_entradas = self.conn.prepare(sql_entradas)?;
        let entradas_detalhadas = stmt_entradas
            .query_map(params![inicio_utc, fim_utc], |row| {
                let id: String = row.get(0)?;
                let created_at: String = row.get(1)?;
                let forma_pagamento: String = row.get(2)?;
                let valor_bruto_centavos: i64 = row.get(3)?;
                let lucro_liquido_centavos: i64 = row.get(4)?;
                Ok(EntradaDetalhadaView {
                    id,
                    coo: None,
                    created_at,
                    cliente_nome: "Consumidor Final".to_string(),
                    forma_pagamento,
                    valor_bruto_centavos,
                    lucro_liquido_centavos,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_default();

        Ok(ResumoDashboardView {
            periodo: periodo.to_string(),
            faturamento_centavos,
            cmv_centavos,
            lucro_bruto_centavos,
            margem_bruta_pct,
            num_vendas,
            ticket_medio_centavos,
            comparativo_faturamento_pct,
            ponto_equilibrio_centavos,
            cobertura_ponto_equilibrio_pct,
            a_pagar_7d_centavos,
            a_pagar_vencidas_centavos,
            num_contas_pagar_vencidas,
            a_receber_7d_centavos,
            crediario_vencido_centavos,
            saldo_caixa_dinheiro_centavos,
            saldo_projetado_7d_centavos,
            total_pix_centavos,
            total_cartao_centavos,
            total_dinheiro_centavos,
            total_crediario_centavos,
            total_outros_centavos,
            entradas_detalhadas,
            atencao_itens,
            fita_do_dia,
        })
    }

    /// Extrato diário do Fluxo de Caixa (Regime de Caixa).
    pub fn obter_fluxo_caixa(
        &self,
        fuso: &str,
        _inicio: Option<&str>,
        _fim: Option<&str>,
    ) -> InfraResult<FluxoCaixaView> {
        let hoje_s = data_local_iso(fuso);
        let em_30_dias = data_local_mais_dias(fuso, 30);
        let sql = "SELECT substr(data_hora, 1, 10) as dia,
                    SUM(CASE WHEN tipo = 'ENTRADA' THEN valor_centavos ELSE 0 END) as entradas,
                    SUM(CASE WHEN tipo = 'SAIDA' THEN valor_centavos ELSE 0 END) as saidas
             FROM (
                 SELECT created_at as data_hora, 'ENTRADA' as tipo, total_centavos as valor_centavos
                 FROM vendas
                 WHERE estado = 'FINALIZADA' AND forma_pagamento != 'CREDIARIO'
                 UNION ALL
                 SELECT created_at as data_hora, 'ENTRADA' as tipo, valor_centavos
                 FROM livro_caixa
                 WHERE tipo = 'SUPRIMENTO'
                 UNION ALL
                 SELECT created_at as data_hora, 'SAIDA' as tipo, valor_centavos
                 FROM livro_caixa
                 WHERE tipo = 'SANGRIA'
                 UNION ALL
                 SELECT pago_em as data_hora, 'SAIDA' as tipo, valor_centavos
                 FROM parcelas
                 WHERE conta_tipo = 'PAGAMENTO' AND status = 'PAGA' AND pago_em IS NOT NULL
                 UNION ALL
                 SELECT pago_em as data_hora, 'ENTRADA' as tipo, valor_centavos
                 FROM parcelas
                 WHERE conta_tipo = 'RECEBIMENTO' AND status = 'PAGA' AND pago_em IS NOT NULL
             )
             WHERE dia IS NOT NULL
             GROUP BY dia
             ORDER BY dia DESC
             LIMIT 30";

        let mut stmt = self.conn.prepare(sql)?;
        let mut total_entradas = 0i64;
        let mut total_saidas = 0i64;

        let dias = stmt
            .query_map([], |row| {
                let data: String = row.get(0)?;
                let entradas_centavos: i64 = row.get(1)?;
                let saidas_centavos: i64 = row.get(2)?;
                let saldo_dia_centavos = entradas_centavos - saidas_centavos;
                Ok(DiaFluxoCaixaView {
                    data,
                    entradas_centavos,
                    saidas_centavos,
                    saldo_dia_centavos,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        for d in &dias {
            total_entradas += d.entradas_centavos;
            total_saidas += d.saidas_centavos;
        }

        let saldo_atual_centavos: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(saldo_esperado_centavos), 0) FROM sessoes_caixa WHERE estado = 'ABERTA'",
            [],
            |row| row.get(0),
        )?;

        let a_receber_30d: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(valor_centavos), 0)
             FROM parcelas
             WHERE conta_tipo = 'RECEBIMENTO' AND status = 'ABERTA'
               AND date(vencimento) >= ?1
               AND date(vencimento) <= ?2",
            params![hoje_s, em_30_dias],
            |row| row.get(0),
        )?;

        let a_pagar_30d: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(valor_centavos), 0)
             FROM parcelas
             WHERE conta_tipo = 'PAGAMENTO' AND status = 'ABERTA'
               AND date(vencimento) >= ?1
               AND date(vencimento) <= ?2",
            params![hoje_s, em_30_dias],
            |row| row.get(0),
        )?;

        let projecao_30d_centavos = saldo_atual_centavos + a_receber_30d - a_pagar_30d;

        Ok(FluxoCaixaView {
            dias,
            saldo_atual_centavos,
            total_entradas_periodo_centavos: total_entradas,
            total_saidas_periodo_centavos: total_saidas,
            projecao_30d_centavos,
        })
    }

    /// DRE Simplificada (Regime de Competência - 7 linhas) para um mês 'AAAA-MM'.
    pub fn obter_resultado_dre(
        &self,
        fuso: &str,
        mes: Option<&str>,
    ) -> InfraResult<ResultadoDREView> {
        let mes_str = match mes {
            Some(m) if m.len() >= 7 => m[..7].to_string(),
            _ => mes_local(fuso),
        };
        // `created_at`/`pago_em` são UTC; o mês é resolvido no fuso da loja.
        let (mes_inicio_utc, mes_fim_utc) = intervalo_utc_do_mes(fuso, &mes_str)?;

        // 1. Receita Bruta e Descontos
        let (receita_bruta_centavos, descontos_centavos): (i64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(subtotal_centavos), 0), COALESCE(SUM(desconto_centavos), 0)
             FROM vendas
             WHERE estado = 'FINALIZADA' AND created_at >= ?1 AND created_at <= ?2",
            params![mes_inicio_utc, mes_fim_utc],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let receita_liquida_centavos = receita_bruta_centavos.saturating_sub(descontos_centavos);

        // 2. CMV
        let cmv_centavos: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(
                 COALESCE(l.custo_centavos, p.custo_centavos, 0) * vi.quantidade
             ), 0)
             FROM venda_itens vi
             JOIN vendas v ON v.id = vi.venda_id
             LEFT JOIN lotes l ON l.id = vi.lote_id
             LEFT JOIN produtos p ON p.id = vi.produto_id
             WHERE v.estado = 'FINALIZADA' AND v.created_at >= ?1 AND v.created_at <= ?2",
            params![mes_inicio_utc, mes_fim_utc],
            |row| row.get(0),
        )?;
        let lucro_bruto_centavos = receita_liquida_centavos.saturating_sub(cmv_centavos);

        // 3. Custos Variáveis (Comissões + Taxas Estimadas de Cartão)
        let comissao_total: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(vi.comissao_centavos), 0)
             FROM venda_itens vi
             JOIN vendas v ON v.id = vi.venda_id
             WHERE v.estado = 'FINALIZADA' AND v.created_at >= ?1 AND v.created_at <= ?2",
            params![mes_inicio_utc, mes_fim_utc],
            |row| row.get(0),
        )?;

        let taxa_cartao_estimada: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(
                 CASE forma
                     WHEN 'DEBITO' THEN CAST(valor_centavos * 0.0199 AS INTEGER)
                     WHEN 'CREDITO' THEN CAST(valor_centavos * 0.0349 AS INTEGER)
                     ELSE 0
                 END
             ), 0)
             FROM venda_pagamentos vp
             JOIN vendas v ON v.id = vp.venda_id
             WHERE v.estado = 'FINALIZADA' AND v.created_at >= ?1 AND v.created_at <= ?2",
            params![mes_inicio_utc, mes_fim_utc],
            |row| row.get(0),
        )?;

        let custos_variaveis_centavos = comissao_total + taxa_cartao_estimada;
        let margem_contribuicao_centavos =
            lucro_bruto_centavos.saturating_sub(custos_variaveis_centavos);
        let margem_contribuicao_pct = if receita_liquida_centavos > 0 {
            (margem_contribuicao_centavos as f64 / receita_liquida_centavos as f64) * 100.0
        } else {
            0.0
        };

        // 4. Despesas Fixas (contas a pagar pagas no mês)
        let despesas_fixas_centavos: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(p.valor_centavos), 0)
             FROM parcelas p
             JOIN contas_pagar cp ON cp.id = p.conta_id
             WHERE p.conta_tipo = 'PAGAMENTO' AND p.status = 'PAGA'
               AND p.pago_em >= ?1 AND p.pago_em <= ?2",
            params![mes_inicio_utc, mes_fim_utc],
            |row| row.get(0),
        )?;

        let resultado_operacional_centavos = margem_contribuicao_centavos - despesas_fixas_centavos;

        // 5. Impostos (lançados no plano 2.1.03 ou estimativa 4% Simples)
        let impostos_lancados: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(p.valor_centavos), 0)
             FROM parcelas p
             JOIN contas_pagar cp ON cp.id = p.conta_id
             WHERE p.conta_tipo = 'PAGAMENTO' AND cp.plano_codigo = '2.1.03'
               AND strftime('%Y-%m', p.vencimento) = ?1",
            params![mes_str],
            |row| row.get(0),
        )?;

        let impostos_estimados_centavos = if impostos_lancados > 0 {
            impostos_lancados
        } else if receita_liquida_centavos > 0 {
            ((receita_liquida_centavos as f64) * 0.04) as i64 // Simples Nacional estimado
        } else {
            0
        };

        let resultado_periodo_centavos =
            resultado_operacional_centavos - impostos_estimados_centavos;

        // 6. Retiradas do dono (sangrias com motivo de retirada)
        let retiradas_dono_centavos: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(valor_centavos), 0)
             FROM livro_caixa
             WHERE tipo = 'SANGRIA'
               AND (motivo LIKE '%RETIRADA%' OR motivo LIKE '%PRO-LABORE%' OR motivo LIKE '%DONO%')
               AND created_at >= ?1 AND created_at <= ?2",
            params![mes_inicio_utc, mes_fim_utc],
            |row| row.get(0),
        )?;

        Ok(ResultadoDREView {
            periodo: mes_str,
            receita_bruta_centavos,
            descontos_centavos,
            receita_liquida_centavos,
            cmv_centavos,
            lucro_bruto_centavos,
            custos_variaveis_centavos,
            margem_contribuicao_centavos,
            margem_contribuicao_pct,
            despesas_fixas_centavos,
            resultado_operacional_centavos,
            impostos_estimados_centavos,
            resultado_periodo_centavos,
            retiradas_dono_centavos,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    #[test]
    fn dashboard_obter_resumo_banco_novo() {
        let db = Database::em_memoria().unwrap();
        let repo = FinanceiroDashboardRepo::novo(db.conn());

        let resumo = repo
            .obter_resumo("hoje", None, None, "America/Sao_Paulo")
            .unwrap();
        assert_eq!(resumo.faturamento_centavos, 0);
        assert_eq!(resumo.num_vendas, 0);
        assert_eq!(resumo.ticket_medio_centavos, 0);
        assert_eq!(resumo.total_pix_centavos, 0);
        assert_eq!(resumo.total_cartao_centavos, 0);
        assert_eq!(resumo.total_dinheiro_centavos, 0);
        assert!(resumo.atencao_itens.len() <= 3);
    }

    #[test]
    fn dashboard_fluxo_caixa_banco_novo() {
        let db = Database::em_memoria().unwrap();
        let repo = FinanceiroDashboardRepo::novo(db.conn());

        let fluxo = repo
            .obter_fluxo_caixa("America/Sao_Paulo", None, None)
            .unwrap();
        assert_eq!(fluxo.saldo_atual_centavos, 0);
        assert_eq!(fluxo.total_entradas_periodo_centavos, 0);
        assert_eq!(fluxo.total_saidas_periodo_centavos, 0);
    }

    #[test]
    fn dashboard_resultado_dre_banco_novo() {
        let db = Database::em_memoria().unwrap();
        let repo = FinanceiroDashboardRepo::novo(db.conn());

        let dre = repo
            .obter_resultado_dre("America/Sao_Paulo", Some("2026-11"))
            .unwrap();
        assert_eq!(dre.periodo, "2026-11");
        assert_eq!(dre.receita_bruta_centavos, 0);
        assert_eq!(dre.cmv_centavos, 0);
        assert_eq!(dre.resultado_periodo_centavos, 0);
    }
}
