//! Handler para consulta e emissão de comprovantes térmicos ESC/POS de vendas.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Utc};
use mikella_infra_hardware::{
    ComprovanteEstorno, ComprovanteFechamentoCaixa, ComprovanteVenda, DispositivoLocalPrinter,
    EscPosGerador, ItemComprovante, LarguraPapel, PagamentoComprovante, PrinterPort,
};
use mikella_infra_sqlite::{
    carregar_estorno, carregar_fechamento, carregar_venda, DiagnosticoRepo, ReciboEstorno,
};
use mikella_shared_kernel::money::Money;

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;

fn carregar_comprovante_venda(
    conn: &rusqlite::Connection,
    venda_id: &str,
) -> Result<ComprovanteVenda, (StatusCode, &'static str)> {
    let view = match carregar_venda(conn, venda_id) {
        Ok(Some(view)) => view,
        Ok(None) => return Err((StatusCode::NOT_FOUND, "venda não encontrada")),
        Err(e) => {
            tracing::error!(erro = %e, venda_id = %venda_id, "erro ao buscar venda");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "falha de banco"));
        }
    };

    let data_hora = DateTime::parse_from_rfc3339(&view.created_at)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let itens = view
        .itens
        .into_iter()
        .map(|item| ItemComprovante {
            nome: item.nome,
            quantidade: item.quantidade,
            preco_unitario: Money::from_centavos(item.preco_unitario_centavos),
            subtotal: Money::from_centavos(item.subtotal_centavos),
        })
        .collect();

    let pagamentos = view
        .pagamentos
        .into_iter()
        .map(|pagamento| PagamentoComprovante {
            forma: pagamento.forma,
            valor: Money::from_centavos(pagamento.valor_centavos),
        })
        .collect();

    // Hash de integridade derivado da própria venda
    let hash_verificacao = blake3::hash(
        format!(
            "{}:{}:{}",
            view.venda_id, view.total_centavos, view.created_at
        )
        .as_bytes(),
    )
    .to_hex()
    .to_string();

    Ok(ComprovanteVenda {
        loja_nome: "Mikella Loja".into(),
        loja_documento: None,
        venda_id: view.venda_id,
        sessao_id: view.sessao_id,
        operador_nome: view.operador_nome,
        data_hora,
        itens,
        subtotal: Money::from_centavos(view.subtotal_centavos),
        desconto: Money::from_centavos(view.desconto_centavos),
        total: Money::from_centavos(view.total_centavos),
        pagamentos,
        troco: if view.troco_centavos > 0 {
            Some(Money::from_centavos(view.troco_centavos))
        } else {
            None
        },
        hash_verificacao: Some(hash_verificacao),
    })
}

/// Monta o comprovante interno de estorno a partir da venda e do livro-caixa.
///
/// O COO é o `rowid` da venda (mesmo número usado na busca do histórico).
/// Motivo e aprovador vêm da entrada `CANCELAMENTO` gravada pelo caso de uso.
pub(crate) fn carregar_comprovante_estorno(
    conn: &rusqlite::Connection,
    venda_id: &str,
) -> Result<ComprovanteEstorno, (StatusCode, &'static str)> {
    let view = match carregar_estorno(conn, venda_id) {
        Ok(ReciboEstorno::Encontrado(view)) => *view,
        Ok(ReciboEstorno::VendaNaoEncontrada) => {
            return Err((StatusCode::NOT_FOUND, "venda não encontrada"));
        }
        Ok(ReciboEstorno::EstornoNaoEncontrado) => {
            return Err((
                StatusCode::NOT_FOUND,
                "registro de estorno não encontrado para esta venda",
            ));
        }
        Err(e) => {
            tracing::error!(erro = %e, venda_id = %venda_id, "erro ao buscar estorno no livro");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "falha de banco"));
        }
    };

    let data_venda = DateTime::parse_from_rfc3339(&view.data_venda)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let data_estorno = DateTime::parse_from_rfc3339(&view.data_estorno)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let formas = view
        .formas
        .into_iter()
        .map(|forma| PagamentoComprovante {
            forma: forma.forma,
            valor: Money::from_centavos(forma.valor_centavos),
        })
        .collect();

    let hash_verificacao = blake3::hash(
        format!(
            "{}:{}:{}:{}",
            view.venda_id, view.coo, view.total_centavos, view.data_estorno
        )
        .as_bytes(),
    )
    .to_hex()
    .to_string();

    Ok(ComprovanteEstorno {
        loja_nome: "Mikella Loja".into(),
        coo: view.coo,
        venda_id: view.venda_id,
        data_venda,
        operador_nome: view.operador_nome,
        total: Money::from_centavos(view.total_centavos),
        valor_estornado: Money::from_centavos(view.total_centavos),
        formas,
        motivo: view.motivo,
        aprovador_nome: view.aprovador_nome,
        aprovador_papel: view.aprovador_papel,
        data_estorno,
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
    let fuso = DiagnosticoRepo::novo(db.conn()).fuso_horario();
    drop(db);

    let texto_58mm = EscPosGerador::gerar_texto_puro(&comprovante, LarguraPapel::Colunas32, &fuso);
    let texto_80mm = EscPosGerador::gerar_texto_puro(&comprovante, LarguraPapel::Colunas48, &fuso);
    let bytes_58mm =
        EscPosGerador::gerar_bytes(&comprovante, LarguraPapel::Colunas32, true, false, &fuso);
    let bytes_80mm =
        EscPosGerador::gerar_bytes(&comprovante, LarguraPapel::Colunas48, true, false, &fuso);

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
    let fuso = DiagnosticoRepo::novo(db.conn()).fuso_horario();
    drop(db);

    let raw_bytes =
        EscPosGerador::gerar_bytes(&comprovante, LarguraPapel::Colunas48, true, false, &fuso);

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
                Json(serde_json::json!({ "erro": "falha na impressora" })),
            )
        }
    }
}

/// `GET /v1/vendas/:id/estorno/comprovante` — comprovante interno do estorno.
pub async fn obter_comprovante_estorno(
    State(state): State<AppState>,
    ator: Ator,
    Path(venda_id): Path<String>,
) -> impl IntoResponse {
    exigir!(ator, "venda.ver");

    let db = state.db.lock().await;
    let c = match carregar_comprovante_estorno(db.conn(), &venda_id) {
        Ok(c) => c,
        Err((status, msg)) => return (status, Json(serde_json::json!({ "erro": msg }))),
    };
    let fuso = DiagnosticoRepo::novo(db.conn()).fuso_horario();
    drop(db);

    let texto_58mm = EscPosGerador::gerar_texto_estorno(&c, LarguraPapel::Colunas32, &fuso);
    let texto_80mm = EscPosGerador::gerar_texto_estorno(&c, LarguraPapel::Colunas48, &fuso);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "venda_id": c.venda_id,
            "coo": c.coo,
            "valor_estornado_centavos": c.valor_estornado.centavos(),
            "texto_58mm": texto_58mm,
            "texto_80mm": texto_80mm,
        })),
    )
}

/// `POST /v1/vendas/:id/estorno/imprimir` — reimprime o comprovante de estorno.
pub async fn imprimir_comprovante_estorno(
    State(state): State<AppState>,
    ator: Ator,
    Path(venda_id): Path<String>,
) -> impl IntoResponse {
    exigir!(ator, "venda.ver");

    let db = state.db.lock().await;
    let c = match carregar_comprovante_estorno(db.conn(), &venda_id) {
        Ok(c) => c,
        Err((status, msg)) => return (status, Json(serde_json::json!({ "erro": msg }))),
    };
    let fuso = DiagnosticoRepo::novo(db.conn()).fuso_horario();
    drop(db);

    let raw_bytes = EscPosGerador::gerar_bytes_estorno(&c, LarguraPapel::Colunas48, true, &fuso);
    let impressora = DispositivoLocalPrinter::novo("padrao");
    match impressora.imprimir(&raw_bytes) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "impresso_com_sucesso" })),
        ),
        Err(e) => {
            tracing::error!(erro = %e, "erro ao imprimir comprovante de estorno");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha na impressora" })),
            )
        }
    }
}

fn carregar_comprovante_fechamento(
    conn: &rusqlite::Connection,
    sessao_id: &str,
) -> Result<ComprovanteFechamentoCaixa, (StatusCode, &'static str)> {
    let view = match carregar_fechamento(conn, sessao_id) {
        Ok(Some(view)) => view,
        Ok(None) => return Err((StatusCode::NOT_FOUND, "sessão de caixa não encontrada")),
        Err(e) => {
            tracing::error!(erro = %e, sessao_id = %sessao_id, "erro ao buscar sessão");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "falha de banco"));
        }
    };

    let aberta_em = DateTime::parse_from_rfc3339(&view.aberta_em)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let fechada_em = view
        .fechada_em
        .as_deref()
        .and_then(|f| DateTime::parse_from_rfc3339(f).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    let mut total_dinheiro = 0i64;
    let mut total_eletronico = 0i64;
    let mut total_suprimentos = 0i64;
    let mut total_sangrias = 0i64;
    let mut ultimo_hash = "GENESIS".to_string();
    let mut contado: Option<i64> = None;
    let mut observacao: Option<String> = None;

    for movimento in &view.movimentos {
        ultimo_hash = movimento.hash.clone();
        match movimento.tipo.as_str() {
            "VENDA_DINHEIRO" => {
                total_dinheiro += movimento.valor_centavos - movimento.troco_centavos;
            }
            "VENDA_DEBITO" | "VENDA_CREDITO" | "VENDA_PIX" | "VENDA_OUTRO" => {
                total_eletronico += movimento.valor_centavos;
            }
            "SUPRIMENTO" => total_suprimentos += movimento.valor_centavos,
            "SANGRIA" => total_sangrias += movimento.valor_centavos,
            "FECHAMENTO" => {
                contado = Some(movimento.valor_centavos);
                observacao = movimento
                    .motivo
                    .as_deref()
                    .and_then(|m| m.split_once("| Obs: "))
                    .map(|(_, obs)| obs.trim().to_string())
                    .filter(|obs| !obs.is_empty());
            }
            _ => {}
        }
    }

    let saldo_esperado_dinheiro = Money::from_centavos(
        view.saldo_inicial_centavos + total_dinheiro + total_suprimentos - total_sangrias,
    );
    // O valor contado é o lançamento FECHAMENTO (o que o operador digitou no
    // fechamento). O saldo "esperado" da sessão é o valor calculado pelo
    // servidor no momento do fechamento, não o contado.
    let saldo_informado_dinheiro =
        Money::from_centavos(contado.unwrap_or(saldo_esperado_dinheiro.centavos()));
    let diferenca = saldo_informado_dinheiro - saldo_esperado_dinheiro;

    Ok(ComprovanteFechamentoCaixa {
        loja_nome: "Mikella Loja".into(),
        loja_documento: None,
        sessao_id: view.sessao_id,
        numero_caixa: view.numero_caixa,
        operador_nome: view.operador_nome,
        aberta_em,
        fechada_em,
        saldo_inicial: Money::from_centavos(view.saldo_inicial_centavos),
        total_vendas_dinheiro: Money::from_centavos(total_dinheiro),
        total_vendas_eletronico: Money::from_centavos(total_eletronico),
        total_suprimentos: Money::from_centavos(total_suprimentos),
        total_sangrias: Money::from_centavos(total_sangrias),
        saldo_esperado_dinheiro,
        saldo_informado_dinheiro,
        diferenca,
        quantidade_vendas: u32::try_from(view.quantidade_vendas).unwrap_or(0),
        quantidade_cancelamentos: u32::try_from(view.quantidade_cancelamentos).unwrap_or(0),
        quantidade_estornos: u32::try_from(view.quantidade_estornos).unwrap_or(0),
        observacao,
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
    let fuso = DiagnosticoRepo::novo(db.conn()).fuso_horario();
    drop(db);

    let texto_58mm =
        EscPosGerador::gerar_texto_fechamento(&comprovante, LarguraPapel::Colunas32, &fuso);
    let texto_80mm =
        EscPosGerador::gerar_texto_fechamento(&comprovante, LarguraPapel::Colunas48, &fuso);
    let bytes_58mm =
        EscPosGerador::gerar_bytes_fechamento(&comprovante, LarguraPapel::Colunas32, true, &fuso);
    let bytes_80mm =
        EscPosGerador::gerar_bytes_fechamento(&comprovante, LarguraPapel::Colunas48, true, &fuso);

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
    let fuso = DiagnosticoRepo::novo(db.conn()).fuso_horario();
    drop(db);

    let raw_bytes =
        EscPosGerador::gerar_bytes_fechamento(&comprovante, LarguraPapel::Colunas48, true, &fuso);

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
                Json(serde_json::json!({ "erro": "falha na impressora" })),
            )
        }
    }
}
