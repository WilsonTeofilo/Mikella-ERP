//! Read models dos comprovantes térmicos (ARQ-001).
//!
//! Estes tipos carregam apenas dados crus do SQLite. A montagem dos structs
//! de `infra-hardware` e a formatação ESC/POS continuam na camada HTTP.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::InfraResult;

/// Item de venda exibido no comprovante.
#[derive(Debug, Clone)]
pub struct ItemReciboView {
    pub nome: String,
    pub quantidade: u32,
    pub preco_unitario_centavos: i64,
    pub subtotal_centavos: i64,
}

/// Pagamento exibido no comprovante.
#[derive(Debug, Clone)]
pub struct PagamentoReciboView {
    pub forma: String,
    pub valor_centavos: i64,
}

/// Dados completos de uma venda para reimpressão do cupom.
#[derive(Debug, Clone)]
pub struct VendaReciboView {
    pub venda_id: String,
    pub sessao_id: String,
    pub subtotal_centavos: i64,
    pub desconto_centavos: i64,
    pub total_centavos: i64,
    pub created_at: String,
    pub operador_nome: String,
    pub itens: Vec<ItemReciboView>,
    pub pagamentos: Vec<PagamentoReciboView>,
    pub troco_centavos: i64,
}

/// Dados do estorno para o comprovante interno.
#[derive(Debug, Clone)]
pub struct EstornoReciboView {
    pub coo: i64,
    pub venda_id: String,
    pub total_centavos: i64,
    pub data_venda: String,
    pub operador_nome: String,
    pub formas: Vec<PagamentoReciboView>,
    pub motivo: String,
    pub data_estorno: String,
    pub aprovador_nome: String,
    pub aprovador_papel: String,
}

/// Resultado da busca do comprovante de estorno.
///
/// A variante de sucesso é `Box`ada para não inflar o tamanho do enum nas
/// variantes vazias (clippy::large_enum_variant).
#[derive(Debug, Clone)]
pub enum ReciboEstorno {
    VendaNaoEncontrada,
    EstornoNaoEncontrado,
    Encontrado(Box<EstornoReciboView>),
}

/// Movimento do livro-caixa usado no fechamento.
#[derive(Debug, Clone)]
pub struct MovimentoReciboView {
    pub tipo: String,
    pub valor_centavos: i64,
    pub troco_centavos: i64,
    pub hash: String,
    pub motivo: Option<String>,
}

/// Dados do fechamento de caixa para o comprovante.
#[derive(Debug, Clone)]
pub struct FechamentoReciboView {
    pub sessao_id: String,
    pub numero_caixa: u8,
    pub aberta_em: String,
    pub fechada_em: Option<String>,
    pub saldo_inicial_centavos: i64,
    pub operador_nome: String,
    pub movimentos: Vec<MovimentoReciboView>,
    pub quantidade_vendas: i64,
    pub quantidade_cancelamentos: i64,
    pub quantidade_estornos: i64,
}

/// Carrega os dados crus de uma venda para o comprovante.
pub fn carregar_venda(conn: &Connection, venda_id: &str) -> InfraResult<Option<VendaReciboView>> {
    let cabecalho = conn
        .query_row(
            "SELECT v.id, v.sessao_id, v.subtotal_centavos, v.desconto_centavos,
                    v.total_centavos, v.created_at, u.nome
             FROM vendas v
             JOIN usuarios u ON u.id = v.operador_id
             WHERE v.id = ?1",
            params![venda_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()?;

    let Some((
        venda_id,
        sessao_id,
        subtotal_centavos,
        desconto_centavos,
        total_centavos,
        created_at,
        operador_nome,
    )) = cabecalho
    else {
        return Ok(None);
    };

    let mut stmt_itens = conn.prepare(
        "SELECT p.nome, vi.quantidade, vi.preco_unitario_centavos, vi.subtotal_centavos
         FROM venda_itens vi
         JOIN produtos p ON p.id = vi.produto_id
         WHERE vi.venda_id = ?1
         ORDER BY vi.rowid ASC",
    )?;
    let rows_itens = stmt_itens.query_map(params![venda_id], |row| {
        Ok(ItemReciboView {
            nome: row.get(0)?,
            quantidade: row.get(1)?,
            preco_unitario_centavos: row.get(2)?,
            subtotal_centavos: row.get(3)?,
        })
    })?;
    let mut itens = Vec::new();
    for item in rows_itens {
        itens.push(item?);
    }

    let mut stmt_pagamentos = conn.prepare(
        "SELECT forma, valor_centavos, troco_centavos
         FROM venda_pagamentos
         WHERE venda_id = ?1
         ORDER BY rowid ASC",
    )?;
    let rows_pagamentos = stmt_pagamentos.query_map(params![venda_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    let mut pagamentos = Vec::new();
    let mut troco_centavos = 0i64;
    for pagamento in rows_pagamentos {
        let (forma, valor_centavos, troco) = pagamento?;
        if troco > 0 {
            troco_centavos += troco;
        }
        pagamentos.push(PagamentoReciboView {
            forma,
            valor_centavos,
        });
    }

    Ok(Some(VendaReciboView {
        venda_id,
        sessao_id,
        subtotal_centavos,
        desconto_centavos,
        total_centavos,
        created_at,
        operador_nome,
        itens,
        pagamentos,
        troco_centavos,
    }))
}

/// Carrega os dados crus do comprovante de estorno.
pub fn carregar_estorno(conn: &Connection, venda_id: &str) -> InfraResult<ReciboEstorno> {
    let venda = conn
        .query_row(
            "SELECT v.rowid, v.id, v.total_centavos, v.created_at, u.nome
             FROM vendas v
             JOIN usuarios u ON u.id = v.operador_id
             WHERE v.id = ?1",
            params![venda_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?;

    let Some((coo, venda_id, total_centavos, data_venda, operador_nome)) = venda else {
        return Ok(ReciboEstorno::VendaNaoEncontrada);
    };

    let mut stmt = conn.prepare(
        "SELECT forma, SUM(valor_centavos)
         FROM venda_pagamentos
         WHERE venda_id = ?1
         GROUP BY forma
         ORDER BY MIN(rowid)",
    )?;
    let rows = stmt.query_map(params![venda_id], |row| {
        Ok(PagamentoReciboView {
            forma: row.get(0)?,
            valor_centavos: row.get(1)?,
        })
    })?;
    let mut formas = Vec::new();
    for forma in rows {
        formas.push(forma?);
    }

    let estorno = conn
        .query_row(
            "SELECT COALESCE(lc.motivo, 'Estorno de venda'),
                    lc.created_at,
                    COALESCE(u.nome, '—'),
                    COALESCE(u.papel, '—')
             FROM livro_caixa lc
             LEFT JOIN usuarios u ON u.id = lc.aprovador_id
             WHERE lc.referencia_id = ?1 AND lc.tipo = 'CANCELAMENTO'
             ORDER BY lc.created_at DESC, lc.rowid DESC
             LIMIT 1",
            params![venda_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;

    let Some((motivo, data_estorno, aprovador_nome, aprovador_papel)) = estorno else {
        return Ok(ReciboEstorno::EstornoNaoEncontrado);
    };

    Ok(ReciboEstorno::Encontrado(Box::new(EstornoReciboView {
        coo,
        venda_id,
        total_centavos,
        data_venda,
        operador_nome,
        formas,
        motivo,
        data_estorno,
        aprovador_nome,
        aprovador_papel,
    })))
}

/// Carrega os dados crus do fechamento de caixa.
pub fn carregar_fechamento(
    conn: &Connection,
    sessao_id: &str,
) -> InfraResult<Option<FechamentoReciboView>> {
    let sessao = conn
        .query_row(
            "SELECT s.id, s.numero_caixa, s.aberta_em, s.fechada_em,
                    s.saldo_inicial_centavos, u.nome
             FROM sessoes_caixa s
             JOIN usuarios u ON u.id = s.operador_id
             WHERE s.id = ?1",
            params![sessao_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u8>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?;

    let Some((
        sessao_id,
        numero_caixa,
        aberta_em,
        fechada_em,
        saldo_inicial_centavos,
        operador_nome,
    )) = sessao
    else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT tipo, valor_centavos, troco_centavos, hash, motivo
         FROM livro_caixa
         WHERE sessao_id = ?1
         ORDER BY created_at ASC, rowid ASC",
    )?;
    let rows = stmt.query_map(params![sessao_id], |row| {
        Ok(MovimentoReciboView {
            tipo: row.get(0)?,
            valor_centavos: row.get(1)?,
            troco_centavos: row.get(2)?,
            hash: row.get(3)?,
            motivo: row.get(4)?,
        })
    })?;
    let mut movimentos = Vec::new();
    for movimento in rows {
        movimentos.push(movimento?);
    }

    let (quantidade_vendas, quantidade_cancelamentos, quantidade_estornos): (i64, i64, i64) = conn
        .query_row(
            "SELECT
                 COALESCE(SUM(CASE WHEN estado = 'FINALIZADA' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN estado = 'CANCELADA' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN estado = 'ESTORNADA' THEN 1 ELSE 0 END), 0)
             FROM vendas WHERE sessao_id = ?1",
            params![sessao_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

    Ok(Some(FechamentoReciboView {
        sessao_id,
        numero_caixa,
        aberta_em,
        fechada_em,
        saldo_inicial_centavos,
        operador_nome,
        movimentos,
        quantidade_vendas,
        quantidade_cancelamentos,
        quantidade_estornos,
    }))
}
