//! Handlers para clientes, crediário e devedores da loja.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use mikella_shared_kernel::money::Money;

#[derive(Deserialize)]
pub struct ClientesQuery {
    pub busca: Option<String>,
}

#[derive(Serialize)]
pub struct ClienteResponse {
    pub id: String,
    pub nome: String,
    pub cpf: Option<String>,
    pub telefone: Option<String>,
    pub email: Option<String>,
    pub endereco: Option<String>,
    pub saldo_devedor_centavos: i64,
    pub saldo_devedor_fmt: String,
    pub tem_vencido: bool,
    pub ativo: bool,
}

#[derive(Deserialize)]
pub struct SalvarClientePayload {
    pub id: Option<String>,
    pub nome: String,
    pub cpf: Option<String>,
    pub telefone: Option<String>,
    pub email: Option<String>,
    pub endereco: Option<String>,
}

#[derive(Serialize)]
pub struct DevedorItemResponse {
    pub cliente_id: String,
    pub cliente_nome: String,
    pub cliente_cpf: Option<String>,
    pub cliente_telefone: Option<String>,
    pub conta_id: String,
    pub parcela_numero: i64,
    pub descricao: String,
    pub vencimento: String,
    pub valor_centavos: i64,
    pub valor_fmt: String,
    pub dias_atraso: i64,
    pub vencido: bool,
}

#[derive(Deserialize)]
pub struct LiquidarDividaPayload {
    pub conta_id: String,
    pub parcela_numero: i64,
    pub forma_pagamento: String,
    pub sessao_id: Option<String>,
}

/// `GET /v1/clientes` — busca clientes por nome, CPF ou celular, com status de crediário.
pub async fn listar_clientes_handler(
    State(state): State<AppState>,
    ator: Ator,
    Query(query): Query<ClientesQuery>,
) -> impl IntoResponse {
    exigir!(ator, "caixa.ver");

    let db = state.db.lock().await;
    let termo = query.busca.unwrap_or_default().trim().to_lowercase();

    let sql = if termo.is_empty() {
        "SELECT id, nome, documento, telefone, email, endereco, ativo FROM pessoas \
         WHERE tipo IN ('CLIENTE', 'AMBOS') AND ativo = 1 ORDER BY nome LIMIT 50".to_string()
    } else {
        format!(
            "SELECT id, nome, documento, telefone, email, endereco, ativo FROM pessoas \
             WHERE tipo IN ('CLIENTE', 'AMBOS') AND ativo = 1 \
               AND (LOWER(nome) LIKE '%{termo}%' OR documento LIKE '%{termo}%' OR telefone LIKE '%{termo}%') \
             ORDER BY nome LIMIT 50"
        )
    };

    let clientes_res = (|| -> rusqlite::Result<Vec<ClienteResponse>> {
        let mut stmt = db.conn().prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let nome: String = row.get(1)?;
            let cpf: Option<String> = row.get(2)?;
            let telefone: Option<String> = row.get(3)?;
            let email: Option<String> = row.get(4)?;
            let endereco: Option<String> = row.get(5)?;
            let ativo: bool = row.get::<_, i64>(6)? == 1;

            Ok((id, nome, cpf, telefone, email, endereco, ativo))
        })?;

        let mut lista = Vec::new();
        for r in rows {
            let (id, nome, cpf, telefone, email, endereco, ativo) = r?;

            // Calcula saldo devedor e se há parcelas vencidas no crediário
            let saldo_e_vencido: (i64, i64) = db
                .conn()
                .query_row(
                    "SELECT COALESCE(SUM(pr.valor_centavos), 0), \
                            COALESCE(SUM(CASE WHEN date(pr.vencimento) < date('now') THEN 1 ELSE 0 END), 0) \
                     FROM parcelas_receber pr \
                     JOIN contas_receber cr ON cr.id = pr.conta_id \
                     WHERE cr.cliente_id = ?1 AND pr.estado = 'PENDENTE'",
                    [&id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap_or((0, 0));

            let saldo_devedor_centavos = saldo_e_vencido.0;
            let tem_vencido = saldo_e_vencido.1 > 0;

            lista.push(ClienteResponse {
                id,
                nome,
                cpf,
                telefone,
                email,
                endereco,
                saldo_devedor_centavos,
                saldo_devedor_fmt: Money::from_centavos(saldo_devedor_centavos).to_string(),
                tem_vencido,
                ativo,
            });
        }
        Ok(lista)
    })();

    match clientes_res {
        Ok(clientes) => (StatusCode::OK, Json(serde_json::json!({ "clientes": clientes }))),
        Err(e) => {
            tracing::error!(erro = %e, "falha ao listar clientes");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha ao consultar clientes" })),
            )
        }
    }
}

/// `POST /v1/clientes` — cadastra ou atualiza um cliente na loja.
pub async fn salvar_cliente_handler(
    State(state): State<AppState>,
    ator: Ator,
    Json(payload): Json<SalvarClientePayload>,
) -> impl IntoResponse {
    exigir!(ator, "caixa.ver");

    let nome = payload.nome.trim();
    if nome.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "erro": "Nome do cliente é obrigatório" })),
        );
    }

    let db = state.db.lock().await;

    let res = if let Some(id) = payload.id.filter(|s| !s.trim().is_empty()) {
        db.conn().execute(
            "UPDATE pessoas SET nome = ?1, documento = ?2, telefone = ?3, email = ?4, endereco = ?5, \
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
             WHERE id = ?6",
            params![
                nome,
                payload.cpf.as_deref().map(str::trim),
                payload.telefone.as_deref().map(str::trim),
                payload.email.as_deref().map(str::trim),
                payload.endereco.as_deref().map(str::trim),
                id,
            ],
        )
    } else {
        let novo_id = Uuid::new_v4().to_string();
        db.conn().execute(
            "INSERT INTO pessoas (id, tipo, nome, documento, telefone, email, endereco, ativo) \
             VALUES (?1, 'CLIENTE', ?2, ?3, ?4, ?5, ?6, 1)",
            params![
                novo_id,
                nome,
                payload.cpf.as_deref().map(str::trim),
                payload.telefone.as_deref().map(str::trim),
                payload.email.as_deref().map(str::trim),
                payload.endereco.as_deref().map(str::trim),
            ],
        )
    };

    match res {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({ "sucesso": true, "mensagem": "Cliente salvo com sucesso" })),
        ),
        Err(e) => {
            tracing::error!(erro = %e, "erro ao salvar cliente");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "Falha de banco ao salvar cliente" })),
            )
        }
    }
}

/// `GET /v1/crediario/devedores` — lista clientes com dívidas em aberto em ordem alfabética.
pub async fn listar_devedores_handler(
    State(state): State<AppState>,
    ator: Ator,
) -> impl IntoResponse {
    exigir!(ator, "caixa.ver");

    let db = state.db.lock().await;

    let sql = "\
        SELECT p.id, p.nome, p.documento, p.telefone, \
               cr.id, pr.numero, cr.descricao, pr.vencimento, pr.valor_centavos, \
               CAST((julianday('now') - julianday(pr.vencimento)) AS INTEGER) as dias_atraso \
        FROM parcelas_receber pr \
        JOIN contas_receber cr ON cr.id = pr.conta_id \
        JOIN pessoas p ON p.id = cr.cliente_id \
        WHERE pr.estado = 'PENDENTE' \
        ORDER BY p.nome ASC, pr.vencimento ASC";

    let devedores_res = (|| -> rusqlite::Result<Vec<DevedorItemResponse>> {
        let mut stmt = db.conn().prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            let valor_c: i64 = row.get(8)?;
            let dias_atraso: i64 = row.get(9).unwrap_or(0);
            Ok(DevedorItemResponse {
                cliente_id: row.get(0)?,
                cliente_nome: row.get(1)?,
                cliente_cpf: row.get(2)?,
                cliente_telefone: row.get(3)?,
                conta_id: row.get(4)?,
                parcela_numero: row.get(5)?,
                descricao: row.get(6)?,
                vencimento: row.get(7)?,
                valor_centavos: valor_c,
                valor_fmt: Money::from_centavos(valor_c).to_string(),
                dias_atraso: dias_atraso.max(0),
                vencido: dias_atraso > 0,
            })
        })?;
        rows.collect()
    })();

    match devedores_res {
        Ok(devedores) => (
            StatusCode::OK,
            Json(serde_json::json!({ "devedores": devedores })),
        ),
        Err(e) => {
            tracing::error!(erro = %e, "erro ao listar devedores");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha ao consultar crediário" })),
            )
        }
    }
}

/// `POST /v1/crediario/liquidar` — baixa uma parcela de dívida de crediário.
pub async fn liquidar_divida_handler(
    State(state): State<AppState>,
    ator: Ator,
    Json(payload): Json<LiquidarDividaPayload>,
) -> impl IntoResponse {
    exigir!(ator, "caixa.criar");

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": e.to_string() })),
            )
        }
    };

    // 1. Busca valor da parcela
    let parcela_info: Option<(i64, String, String)> = tx
        .query_row(
            "SELECT pr.valor_centavos, cr.descricao, cr.cliente_id \
             FROM parcelas_receber pr \
             JOIN contas_receber cr ON cr.id = pr.conta_id \
             WHERE pr.conta_id = ?1 AND pr.numero = ?2 AND pr.estado = 'PENDENTE'",
            params![&payload.conta_id, payload.parcela_numero],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();

    let (valor_centavos, descricao, _cliente_id) = match parcela_info {
        Some(info) => info,
        None => {
            let _ = tx.rollback();
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "erro": "Parcela de dívida não encontrada ou já liquidada" })),
            );
        }
    };

    // 2. Atualiza estado da parcela para RECEBIDO
    let updated = tx.execute(
        "UPDATE parcelas_receber \
         SET estado = 'RECEBIDO', recebido_em = strftime('%Y-%m-%dT%H:%M:%fZ','now'), recebido_por = ?1 \
         WHERE conta_id = ?2 AND numero = ?3",
        params![ator.id().to_string(), &payload.conta_id, payload.parcela_numero],
    );

    if let Err(e) = updated {
        let _ = tx.rollback();
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": format!("Falha ao liquidar parcela: {}", e) })),
        );
    }

    // 3. Se foi pago em dinheiro e houver caixa aberto, lança como SUPRIMENTO / RECEBIMENTO no livro-caixa
    if payload.forma_pagamento.to_uppercase() == "DINHEIRO" {
        if let Some(sessao_id_str) = payload.sessao_id.filter(|s| !s.trim().is_empty()) {
            let livro = mikella_infra_sqlite::LivroRepo::novo(&tx);
            if let Ok(sessao_id) = sessao_id_str.parse() {
                use mikella_domain_cash::ports::LivroCaixaRepository;
                if let Ok(posicao) = livro.posicao_atual(sessao_id) {
                    let entrada_id = Uuid::new_v4().to_string();
                    let motivo = format!("Recebimento Crediário: {} (Parc {})", descricao, payload.parcela_numero);
                    let mut hasher = blake3::Hasher::new();
                    hasher.update(posicao.ultimo_hash.as_bytes());
                    hasher.update(entrada_id.as_bytes());
                    hasher.update(b"SUPRIMENTO");
                    hasher.update(&valor_centavos.to_le_bytes());
                    let novo_hash = hasher.finalize().to_hex().to_string();

                    let _ = tx.execute(
                        "INSERT INTO livro_caixa \
                         (id, sessao_id, sequencia, tipo, valor_centavos, troco_centavos, \
                          referencia_id, referencia_tipo, usuario_id, motivo, prev_hash, hash) \
                         VALUES (?1, ?2, ?3, 'SUPRIMENTO', ?4, 0, ?5, 'CREDIARIO', ?6, ?7, ?8, ?9)",
                        params![
                            entrada_id,
                            sessao_id_str,
                            posicao.proxima_sequencia,
                            valor_centavos,
                            payload.conta_id,
                            ator.id().to_string(),
                            motivo,
                            posicao.ultimo_hash,
                            novo_hash,
                        ],
                    );
                }
            }
        }
    }

    if let Err(e) = tx.commit() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": format!("Falha ao confirmar liquidação: {}", e) })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "sucesso": true,
            "mensagem": "Dívida quitada com sucesso!",
            "valor_pago": Money::from_centavos(valor_centavos).to_string(),
        })),
    )
}
