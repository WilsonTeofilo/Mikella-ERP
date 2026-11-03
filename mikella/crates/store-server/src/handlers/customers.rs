//! Handlers HTTP para clientes, crediário e devedores da loja (Clean Architecture).

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use mikella_domain_people::ports::PessoaRepository;
use mikella_domain_people::Pessoa;
use mikella_infra_sqlite::PessoaRepo;
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
    #[allow(dead_code)]
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
    let repo = PessoaRepo::novo(db.conn());

    match repo.buscar_clientes_com_saldo(query.busca.as_deref()) {
        Ok(clientes) => {
            let lista: Vec<ClienteResponse> = clientes
                .into_iter()
                .map(|c| ClienteResponse {
                    id: c.pessoa.id,
                    nome: c.pessoa.nome,
                    cpf: c.pessoa.documento,
                    telefone: c.pessoa.telefone,
                    email: c.pessoa.email,
                    endereco: c.pessoa.endereco,
                    saldo_devedor_centavos: c.saldo_devedor_centavos,
                    saldo_devedor_fmt: Money::from_centavos(c.saldo_devedor_centavos).to_string(),
                    tem_vencido: c.tem_vencido,
                    ativo: c.pessoa.ativo,
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!({ "clientes": lista })))
        }
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

    let id_existente = payload.id.as_deref().filter(|s| !s.trim().is_empty());
    let id = id_existente
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let pessoa = match Pessoa::novo_cliente(
        id,
        payload.nome,
        payload.telefone,
        payload.email,
        payload.cpf,
        payload.endereco,
        chrono::Utc::now(),
    ) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "erro": e.to_string() })),
            );
        }
    };

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": format!("Erro ao abrir transação: {e}") })),
            );
        }
    };

    let repo = PessoaRepo::novo(&tx);
    let resultado = if id_existente.is_some() {
        repo.atualizar(&pessoa)
    } else {
        repo.salvar(&pessoa)
    };

    match resultado {
        Ok(_) => {
            if let Err(e) = tx.commit() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": format!("Erro ao confirmar transação: {e}") })),
                );
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({ "sucesso": true, "mensagem": "Cliente salvo com sucesso" })),
            )
        }
        Err(e) => {
            tracing::error!(erro = %e, "erro ao salvar cliente");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": format!("Falha ao salvar cliente: {e}") })),
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
    let repo = PessoaRepo::novo(db.conn());

    match repo.listar_devedores() {
        Ok(devedores) => {
            let mut lista_resp = Vec::new();
            for dev in devedores {
                for parc in dev.parcelas {
                    lista_resp.push(DevedorItemResponse {
                        cliente_id: dev.cliente_id.clone(),
                        cliente_nome: dev.nome.clone(),
                        cliente_cpf: dev.documento.clone(),
                        cliente_telefone: dev.telefone.clone(),
                        conta_id: parc.parcela_id.clone(),
                        parcela_numero: parc.numero as i64,
                        descricao: format!("Parcela {}/{}", parc.numero, parc.numero),
                        vencimento: parc.vencimento,
                        valor_centavos: parc.valor_centavos,
                        valor_fmt: Money::from_centavos(parc.valor_centavos).to_string(),
                        dias_atraso: parc.dias_atraso,
                        vencido: parc.dias_atraso > 0,
                    });
                }
            }
            (StatusCode::OK, Json(serde_json::json!({ "devedores": lista_resp })))
        }
        Err(e) => {
            tracing::error!(erro = %e, "falha ao listar devedores");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha ao consultar devedores" })),
            )
        }
    }
}

/// `POST /v1/crediario/liquidar` — baixa uma parcela de dívida com lançamento no caixa se for dinheiro.
pub async fn liquidar_divida_handler(
    State(state): State<AppState>,
    ator: Ator,
    Json(payload): Json<LiquidarDividaPayload>,
) -> impl IntoResponse {
    exigir!(ator, "caixa.abrir");

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": format!("Erro ao abrir transação: {e}") })),
            );
        }
    };

    let repo = PessoaRepo::novo(&tx);
    let valor_centavos = match repo.liquidar_parcela(&payload.conta_id, &payload.forma_pagamento) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "erro": e.to_string() })),
            );
        }
    };

    // Se o pagamento foi em dinheiro e a sessão do caixa estiver ativa, registra como suprimento no livro caixa
    if payload.forma_pagamento.eq_ignore_ascii_case("DINHEIRO") {
        if let Some(sessao_id) = payload.sessao_id.filter(|s| !s.trim().is_empty()) {
            let sessao_uuid: Result<uuid::Uuid, _> = sessao_id.parse();
            if let Ok(suuid) = sessao_uuid {
                let livro_repo = mikella_infra_sqlite::LivroRepo::novo(&tx);
                let sid = mikella_shared_kernel::ids::SessaoId::from_uuid(suuid);
                if let Ok(posicao) = mikella_domain_cash::ports::LivroCaixaRepository::posicao_atual(&livro_repo, sid) {
                    let hash_chain = mikella_infra_sqlite::Blake3HashChain;
                    let mut entrada = mikella_domain_cash::livro::EntradaLivro {
                        id: mikella_shared_kernel::ids::LivroCaixaId::new(),
                        sessao_id: sid,
                        sequencia: posicao.proxima_sequencia,
                        tipo: mikella_domain_cash::livro::TipoMovimentacao::Suprimento,
                        valor_centavos: Money::from_centavos(valor_centavos),
                        troco_centavos: Money::ZERO,
                        referencia: Some(mikella_domain_cash::livro::Referencia {
                            id: payload.conta_id.clone(),
                            tipo: "RECEBIMENTO_CREDIARIO".to_string(),
                        }),
                        usuario_id: ator.id(),
                        aprovador_id: None,
                        motivo: Some("Recebimento de dívida de crediário".to_string()),
                        prev_hash: posicao.ultimo_hash,
                        hash: String::new(),
                        created_at: chrono::Utc::now(),
                    };
                    entrada.hash = mikella_domain_cash::ports::HashChain::hash_entrada(&hash_chain, &entrada);
                    let _ = mikella_domain_cash::ports::LivroCaixaRepository::inserir_entrada(&livro_repo, &entrada);
                }
            }
        }
    }

    if let Err(e) = tx.commit() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": format!("Erro ao confirmar transação: {e}") })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "sucesso": true,
            "mensagem": "Dívida liquidada com sucesso",
            "valor_liquidado_centavos": valor_centavos
        })),
    )
}
