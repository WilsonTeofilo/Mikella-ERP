//! Handler para consulta e emissão de comprovantes térmicos ESC/POS de vendas.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Utc};
use mikella_infra_hardware::{
    ComprovanteFechamentoCaixa, ComprovanteVenda, DispositivoLocalPrinter, EscPosGerador,
    ItemComprovante, LarguraPapel, PagamentoComprovante, PrinterPort,
};
use mikella_shared_kernel::money::Money;

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;

fn carregar_comprovante_venda(
    conn: &rusqlite::Connection,
    venda_id: &str,
) -> Result<ComprovanteVenda, (StatusCode, &'static str)> {
    // 1. Busca os dados da venda
    let venda_row = conn.query_row(
        "SELECT v.id, v.sessao_id, v.subtotal_centavos, v.desconto_centavos, v.total_centavos, \
                v.created_at, u.nome \
         FROM vendas v \
         JOIN usuarios u ON u.id = v.operador_id \
         WHERE v.id = ?1",
        [venda_id],
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
    );

    let (v_id, sessao_id, subtotal_c, desconto_c, total_c, created_at_str, operador_nome) =
        match venda_row {
            Ok(v) => v,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err((StatusCode::NOT_FOUND, "venda não encontrada"));
            }
            Err(e) => {
                tracing::error!(erro = %e, venda_id = %venda_id, "erro ao buscar venda");
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "falha de banco"));
            }
        };

    let data_hora = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    // 2. Busca itens
    let itens = {
        let mut itens_stmt = conn
            .prepare(
                "SELECT p.nome, vi.quantidade, vi.preco_unitario_centavos, vi.subtotal_centavos \
                 FROM venda_itens vi \
                 JOIN produtos p ON p.id = vi.produto_id \
                 WHERE vi.venda_id = ?1 \
                 ORDER BY vi.rowid ASC",
            )
            .map_err(|e| {
                tracing::error!(erro = %e, "erro ao preparar query de itens");
                (StatusCode::INTERNAL_SERVER_ERROR, "falha de banco")
            })?;

        let itens_iter = itens_stmt.query_map([&v_id], |row| {
            Ok(ItemComprovante {
                nome: row.get(0)?,
                quantidade: row.get(1)?,
                preco_unitario: Money::from_centavos(row.get(2)?),
                subtotal: Money::from_centavos(row.get(3)?),
            })
        });

        match itens_iter {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    };

    // 3. Busca pagamentos
    let (pagamentos, troco_acumulado) = {
        let mut pag_stmt = conn
            .prepare(
                "SELECT forma, valor_centavos, troco_centavos \
                 FROM venda_pagamentos \
                 WHERE venda_id = ?1 \
                 ORDER BY rowid ASC",
            )
            .map_err(|e| {
                tracing::error!(erro = %e, "erro ao preparar query de pagamentos");
                (StatusCode::INTERNAL_SERVER_ERROR, "falha de banco")
            })?;

        let mut troco_total = 0i64;
        let mut pags = Vec::new();

        let pag_iter = pag_stmt.query_map([&v_id], |row| {
            let forma: String = row.get(0)?;
            let valor: i64 = row.get(1)?;
            let troco: i64 = row.get(2)?;
            Ok((forma, valor, troco))
        });

        if let Ok(rows) = pag_iter {
            for res in rows.flatten() {
                if res.2 > 0 {
                    troco_total += res.2;
                }
                pags.push(PagamentoComprovante {
                    forma: res.0,
                    valor: Money::from_centavos(res.1),
                });
            }
        }

        (pags, troco_total)
    };

    // Hash de integridade derivado da própria venda
    let hash_verificacao = blake3::hash(format!("{}:{}:{}", v_id, total_c, created_at_str).as_bytes())
        .to_hex()
        .to_string();

    Ok(ComprovanteVenda {
        loja_nome: "Mikella Loja".into(),
        loja_documento: None,
        venda_id: v_id,
        sessao_id,
        operador_nome,
        data_hora,
        itens,
        subtotal: Money::from_centavos(subtotal_c),
        desconto: Money::from_centavos(desconto_c),
        total: Money::from_centavos(total_c),
        pagamentos,
        troco: if troco_acumulado > 0 {
            Some(Money::from_centavos(troco_acumulado))
        } else {
            None
        },
        hash_verificacao: Some(hash_verificacao),
    })
}

/// `GET /v1/vendas/:id/comprovante` — gera o comprovante formatado (texto puro e ESC/POS hex).
pub async fn obter_comprovante(
    State(state): State<AppState>,
    ator: Ator,
    Path(venda_id): Path<String>,
) -> impl IntoResponse {
    exigir!(ator, "venda.ver");

    let db = state.db.lock().await;
    let comprovante = match carregar_comprovante_venda(db.conn(), &venda_id) {
        Ok(c) => c,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "erro": msg })));
        }
    };
    drop(db);

    let texto_58mm = EscPosGerador::gerar_texto_puro(&comprovante, LarguraPapel::Colunas32);
    let texto_80mm = EscPosGerador::gerar_texto_puro(&comprovante, LarguraPapel::Colunas48);
    let bytes_58mm = EscPosGerador::gerar_bytes(&comprovante, LarguraPapel::Colunas32, true, false);
    let bytes_80mm = EscPosGerador::gerar_bytes(&comprovante, LarguraPapel::Colunas48, true, false);

    let hex_58mm: String = bytes_58mm.iter().map(|b| format!("{:02x}", b)).collect();
    let hex_80mm: String = bytes_80mm.iter().map(|b| format!("{:02x}", b)).collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "venda_id": comprovante.venda_id,
            "texto_58mm": texto_58mm,
            "texto_80mm": texto_80mm,
            "escpos_hex_58mm": hex_58mm,
            "escpos_hex_80mm": hex_80mm,
        })),
    )
}

/// `POST /v1/vendas/:id/imprimir` — envia o comprovante diretamente para a impressora configurada.
pub async fn imprimir_comprovante(
    State(state): State<AppState>,
    ator: Ator,
    Path(venda_id): Path<String>,
) -> impl IntoResponse {
    exigir!(ator, "venda.ver");

    let db = state.db.lock().await;
    let comprovante = match carregar_comprovante_venda(db.conn(), &venda_id) {
        Ok(c) => c,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "erro": msg })));
        }
    };
    drop(db);

    let raw_bytes = EscPosGerador::gerar_bytes(&comprovante, LarguraPapel::Colunas48, true, false);

    // Imprime via dispositivo padrão
    let impressora = DispositivoLocalPrinter::novo("padrao");
    match impressora.imprimir(&raw_bytes) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "impresso_com_sucesso" })),
        ),
        Err(e) => {
            tracing::error!(erro = %e, "erro ao imprimir comprovante");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": format!("falha na impressora: {}", e) })),
            )
        }
    }
}

fn carregar_comprovante_fechamento(
    conn: &rusqlite::Connection,
    sessao_id: &str,
) -> Result<ComprovanteFechamentoCaixa, (StatusCode, &'static str)> {
    // 1. Busca os dados da sessão
    let sessao_row = conn.query_row(
        "SELECT s.id, s.numero_caixa, s.aberta_em, s.fechada_em, s.saldo_inicial_centavos, \
                s.saldo_esperado_centavos, u.nome \
         FROM sessoes_caixa s \
         JOIN usuarios u ON u.id = s.operador_id \
         WHERE s.id = ?1",
        [sessao_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u8>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, String>(6)?,
            ))
        },
    );

    let (s_id, num_caixa, aberta_str, fechada_str, saldo_ini, saldo_esp, operador_nome) =
        match sessao_row {
            Ok(s) => s,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err((StatusCode::NOT_FOUND, "sessão de caixa não encontrada"));
            }
            Err(e) => {
                tracing::error!(erro = %e, sessao_id = %sessao_id, "erro ao buscar sessão");
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "falha de banco"));
            }
        };

    let aberta_em = DateTime::parse_from_rfc3339(&aberta_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let fechada_em = fechada_str
        .as_deref()
        .and_then(|f| DateTime::parse_from_rfc3339(f).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    // 2. Busca movimentações do livro_caixa
    let mut stmt = conn
        .prepare(
            "SELECT tipo, valor_centavos, troco_centavos, hash \
             FROM livro_caixa \
             WHERE sessao_id = ?1 \
             ORDER BY created_at ASC, rowid ASC",
        )
        .map_err(|e| {
            tracing::error!(erro = %e, "erro ao preparar query do livro");
            (StatusCode::INTERNAL_SERVER_ERROR, "falha de banco")
        })?;

    let mut total_dinheiro = 0i64;
    let mut total_eletronico = 0i64;
    let mut total_suprimentos = 0i64;
    let mut total_sangrias = 0i64;
    let mut ultimo_hash = "GENESIS".to_string();

    let rows = stmt.query_map([&s_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    });

    if let Ok(entries) = rows {
        for res in entries.flatten() {
            let (tipo, valor, troco, hash) = res;
            ultimo_hash = hash;
            match tipo.as_str() {
                "VENDA_DINHEIRO" => total_dinheiro += valor - troco,
                "VENDA_DEBITO" | "VENDA_CREDITO" | "VENDA_PIX" | "VENDA_OUTRO" => {
                    total_eletronico += valor;
                }
                "SUPRIMENTO" => total_suprimentos += valor,
                "SANGRIA" => total_sangrias += valor,
                _ => {}
            }
        }
    }

    let saldo_esperado_dinheiro =
        Money::from_centavos(saldo_ini + total_dinheiro + total_suprimentos - total_sangrias);
    let saldo_informado_dinheiro =
        Money::from_centavos(saldo_esp.unwrap_or(saldo_esperado_dinheiro.centavos()));
    let diferenca = saldo_informado_dinheiro - saldo_esperado_dinheiro;

    Ok(ComprovanteFechamentoCaixa {
        loja_nome: "Mikella Loja".into(),
        loja_documento: None,
        sessao_id: s_id,
        numero_caixa: num_caixa,
        operador_nome,
        aberta_em,
        fechada_em,
        saldo_inicial: Money::from_centavos(saldo_ini),
        total_vendas_dinheiro: Money::from_centavos(total_dinheiro),
        total_vendas_eletronico: Money::from_centavos(total_eletronico),
        total_suprimentos: Money::from_centavos(total_suprimentos),
        total_sangrias: Money::from_centavos(total_sangrias),
        saldo_esperado_dinheiro,
        saldo_informado_dinheiro,
        diferenca,
        ultimo_hash,
    })
}

/// `GET /v1/caixa/fechamento/:sessao_id/comprovante` — gera comprovante térmico do fechamento de caixa.
pub async fn obter_comprovante_fechamento(
    State(state): State<AppState>,
    ator: Ator,
    Path(sessao_id): Path<String>,
) -> impl IntoResponse {
    exigir!(ator, "caixa.ver");

    let db = state.db.lock().await;
    let comprovante = match carregar_comprovante_fechamento(db.conn(), &sessao_id) {
        Ok(c) => c,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "erro": msg })));
        }
    };
    drop(db);

    let texto_58mm =
        EscPosGerador::gerar_texto_fechamento(&comprovante, LarguraPapel::Colunas32);
    let texto_80mm =
        EscPosGerador::gerar_texto_fechamento(&comprovante, LarguraPapel::Colunas48);
    let bytes_58mm =
        EscPosGerador::gerar_bytes_fechamento(&comprovante, LarguraPapel::Colunas32, true);
    let bytes_80mm =
        EscPosGerador::gerar_bytes_fechamento(&comprovante, LarguraPapel::Colunas48, true);

    let hex_58mm: String = bytes_58mm.iter().map(|b| format!("{:02x}", b)).collect();
    let hex_80mm: String = bytes_80mm.iter().map(|b| format!("{:02x}", b)).collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "sessao_id": comprovante.sessao_id,
            "numero_caixa": comprovante.numero_caixa,
            "texto_58mm": texto_58mm,
            "texto_80mm": texto_80mm,
            "escpos_hex_58mm": hex_58mm,
            "escpos_hex_80mm": hex_80mm,
        })),
    )
}

/// `POST /v1/caixa/fechamento/:sessao_id/imprimir` — imprime diretamente o fechamento de caixa.
pub async fn imprimir_comprovante_fechamento(
    State(state): State<AppState>,
    ator: Ator,
    Path(sessao_id): Path<String>,
) -> impl IntoResponse {
    exigir!(ator, "caixa.ver");

    let db = state.db.lock().await;
    let comprovante = match carregar_comprovante_fechamento(db.conn(), &sessao_id) {
        Ok(c) => c,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "erro": msg })));
        }
    };
    drop(db);

    let raw_bytes =
        EscPosGerador::gerar_bytes_fechamento(&comprovante, LarguraPapel::Colunas48, true);

    let impressora = DispositivoLocalPrinter::novo("padrao");
    match impressora.imprimir(&raw_bytes) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "impresso_com_sucesso" })),
        ),
        Err(e) => {
            tracing::error!(erro = %e, "erro ao imprimir fechamento de caixa");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": format!("falha na impressora: {}", e) })),
            )
        }
    }
}
