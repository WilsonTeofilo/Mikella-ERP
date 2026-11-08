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
use mikella_shared_kernel::documento::mascarar_documento;
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
    pub parcela_id: String,
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
    /// ID da parcela em aberto (nome antigo `conta_id` aceito por compatibilidade).
    #[serde(alias = "conta_id")]
    pub parcela_id: String,
    pub forma_pagamento: String,
    pub sessao_id: Option<String>,
}

/// `GET /v1/clientes` — busca clientes por nome, CPF ou celular, com status de crediário.
///
/// SEC-013 / LGPD: se o operador tiver nível < 2 (caixa/vendedor), o CPF é mascarado.
pub async fn listar_clientes_handler(
    State(state): State<AppState>,
    ator: Ator,
    Query(query): Query<ClientesQuery>,
) -> impl IntoResponse {
    exigir!(ator, "caixa.ver");

    let db = state.db.lock().await;
    let repo = PessoaRepo::com_chave(db.conn(), &state.chave_loja);

    match repo.buscar_clientes_com_saldo(query.busca.as_deref()) {
        Ok(clientes) => {
            let lista: Vec<ClienteResponse> = clientes
                .into_iter()
                .map(|c| {
                    let cpf_exibicao = if ator.papel().nivel() >= 2 {
                        c.pessoa.documento
                    } else {
                        c.pessoa.documento.as_deref().map(mascarar_documento)
                    };
                    ClienteResponse {
                        id: c.pessoa.id,
                        nome: c.pessoa.nome,
                        cpf: cpf_exibicao,
                        telefone: c.pessoa.telefone,
                        email: c.pessoa.email,
                        endereco: c.pessoa.endereco,
                        saldo_devedor_centavos: c.saldo_devedor_centavos,
                        saldo_devedor_fmt: Money::from_centavos(c.saldo_devedor_centavos).to_string(),
                        tem_vencido: c.tem_vencido,
                        ativo: c.pessoa.ativo,
                    }
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
        Err(mikella_shared_kernel::error::DomainError::Invalido(msg)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "erro": msg })),
            );
        }
        Err(e) => {
            tracing::error!(erro = %e, "erro ao validar cliente");
            return crate::util::error_para_http(e);
        }
    };

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao abrir transação de cliente");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha de banco" })),
            );
        }
    };

    let repo = PessoaRepo::com_chave(&tx, &state.chave_loja);

    if let Some(tel) = pessoa.telefone.as_deref() {
        match repo.telefone_em_uso(tel, id_existente) {
            Ok(true) => {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({ "erro": "Telefone já cadastrado para outro cliente" })),
                );
            }
            Ok(false) => {}
            Err(e) => {
                tracing::error!(erro = %e, "falha ao verificar telefone do cliente");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha ao validar telefone" })),
                );
            }
        }
    }

    if let Some(doc) = pessoa.documento.as_deref() {
        match repo.documento_em_uso(doc, id_existente) {
            Ok(true) => {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({ "erro": "CPF/CNPJ já cadastrado para outro cliente" })),
                );
            }
            Ok(false) => {}
            Err(e) => {
                tracing::error!(erro = %e, "falha ao verificar documento do cliente");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha ao validar documento" })),
                );
            }
        }
    }

    let resultado = if id_existente.is_some() {
        repo.atualizar(&pessoa)
    } else {
        repo.salvar(&pessoa)
    };

    match resultado {
        Ok(_) => {
            if let Err(e) = tx.commit() {
                tracing::error!(erro = %e, "falha ao confirmar transação de cliente");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha de banco" })),
                );
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({ "sucesso": true, "mensagem": "Cliente salvo com sucesso" })),
            )
        }
        Err(mikella_shared_kernel::error::DomainError::NaoEncontrado { .. }) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "erro": "Cliente não encontrado" })),
        ),
        Err(mikella_shared_kernel::error::DomainError::Invalido(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "erro": msg })),
        ),
        Err(e) => {
            tracing::error!(erro = %e, "erro ao salvar cliente");
            crate::util::error_para_http(e)
        }
    }
}

/// `GET /v1/crediario/devedores` — lista clientes com dívidas em aberto em ordem alfabética.
///
/// SEC-013 / LGPD: se o operador tiver nível < 2 (caixa/vendedor), o CPF é mascarado.
pub async fn listar_devedores_handler(
    State(state): State<AppState>,
    ator: Ator,
) -> impl IntoResponse {
    exigir!(ator, "caixa.ver");

    let db = state.db.lock().await;
    let repo = PessoaRepo::com_chave(db.conn(), &state.chave_loja);

    match repo.listar_devedores() {
        Ok(devedores) => {
            let mut lista_resp = Vec::new();
            for dev in devedores {
                let cpf_exibicao = if ator.papel().nivel() >= 2 {
                    dev.documento.clone()
                } else {
                    dev.documento.as_deref().map(mascarar_documento)
                };

                for parc in dev.parcelas {
                    lista_resp.push(DevedorItemResponse {
                        cliente_id: dev.cliente_id.clone(),
                        cliente_nome: dev.nome.clone(),
                        cliente_cpf: cpf_exibicao.clone(),
                        cliente_telefone: dev.telefone.clone(),
                        parcela_id: parc.parcela_id.clone(),
                        parcela_numero: i64::from(parc.numero),
                        descricao: format!("Parcela {}", parc.numero),
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

/// `POST /v1/crediario/liquidar` — baixa uma parcela de dívida.
///
/// Recebimento em dinheiro entra no livro-caixa como venda à vista; as demais
/// formas apenas liquidam a parcela (sem inflar o dinheiro esperado da gaveta).
pub async fn liquidar_divida_handler(
    State(state): State<AppState>,
    ator: Ator,
    Json(payload): Json<LiquidarDividaPayload>,
) -> impl IntoResponse {
    exigir!(ator, "financeiro.gerenciar");

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao abrir transação de liquidação");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha de banco" })),
            );
        }
    };

    let repo = PessoaRepo::com_chave(&tx, &state.chave_loja);
    let valor_centavos = match repo.liquidar_parcela(&payload.parcela_id, &payload.forma_pagamento) {
        Ok(v) => v,
        Err(mikella_shared_kernel::error::DomainError::EstadoInvalido(msg)) => {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({ "erro": msg })),
            );
        }
        Err(mikella_shared_kernel::error::DomainError::Invalido(msg)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "erro": msg })),
            );
        }
        Err(mikella_shared_kernel::error::DomainError::NaoEncontrado { .. }) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "erro": "Parcela não encontrada" })),
            );
        }
        Err(e) => {
            tracing::error!(erro = %e, "erro ao liquidar parcela");
            return crate::util::error_para_http(e);
        }
    };

    // Toda liquidação realizada no caixa deve gerar lançamento no livro-caixa
    // para conciliação atômica com o meio de pagamento utilizado.
    let tipo_mov = match payload.forma_pagamento.trim().to_ascii_uppercase().as_str() {
        "DINHEIRO" => mikella_domain_cash::livro::TipoMovimentacao::VendaDinheiro,
        "DEBITO" => mikella_domain_cash::livro::TipoMovimentacao::VendaDebito,
        "CREDITO" => mikella_domain_cash::livro::TipoMovimentacao::VendaCredito,
        "PIX" => mikella_domain_cash::livro::TipoMovimentacao::VendaPix,
        _ => mikella_domain_cash::livro::TipoMovimentacao::VendaOutro,
    };

    if let Some(sessao_id) = payload.sessao_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let Ok(suuid) = sessao_id.parse::<Uuid>() else {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "erro": "sessao_id inválido (UUID esperado)" })),
            );
        };
        let sid = mikella_shared_kernel::ids::SessaoId::from_uuid(suuid);
        let livro_repo = mikella_infra_sqlite::LivroRepo::novo(&tx);

        let posicao = match mikella_domain_cash::ports::LivroCaixaRepository::posicao_atual(
            &livro_repo,
            sid,
        ) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(erro = %e, "falha ao ler posição do livro-caixa");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha ao registrar recebimento no caixa" })),
                );
            }
        };

        let hash_chain = mikella_infra_sqlite::Blake3HashChain;
        let mut entrada = mikella_domain_cash::livro::EntradaLivro {
            id: mikella_shared_kernel::ids::LivroCaixaId::new(),
            sessao_id: sid,
            sequencia: posicao.proxima_sequencia,
            tipo: tipo_mov,
            valor_centavos: Money::from_centavos(valor_centavos),
            troco_centavos: Money::ZERO,
            referencia: Some(mikella_domain_cash::livro::Referencia {
                id: payload.parcela_id.clone(),
                tipo: "RECEBIMENTO_CREDIARIO".to_string(),
            }),
            usuario_id: ator.id(),
            aprovador_id: None,
            motivo: Some(format!("Recebimento de dívida ({})", payload.forma_pagamento)),
            prev_hash: posicao.ultimo_hash,
            hash: String::new(),
            created_at: chrono::Utc::now(),
        };
        entrada.hash =
            mikella_domain_cash::ports::HashChain::hash_entrada(&hash_chain, &entrada);

        if let Err(e) =
            mikella_domain_cash::ports::LivroCaixaRepository::inserir_entrada(&livro_repo, &entrada)
        {
            tracing::error!(erro = %e, "falha ao inserir recebimento no livro-caixa");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha ao registrar recebimento no caixa" })),
            );
        }
    }

    // Carrega dados adicionais para emissão do comprovante térmico de liquidação
    let info_recibo = tx
        .query_row(
            "SELECT pe.nome, pe.documento, pa.numero
             FROM parcelas pa
             INNER JOIN contas_receber c ON c.id = pa.conta_id
             INNER JOIN pessoas pe ON pe.id = c.pessoa_id
             WHERE pa.id = ?1",
            [&payload.parcela_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, u32>(2)?,
                ))
            },
        )
        .ok();

    let recibo_json = info_recibo.map(|(nome, doc, num)| {
        let doc_dec = repo.decifrar_campo(doc);
        let doc_exibicao = if ator.papel().nivel() >= 2 {
            doc_dec
        } else {
            doc_dec.as_deref().map(mascarar_documento)
        };
        serde_json::json!({
            "cliente_nome": nome,
            "cliente_documento": doc_exibicao,
            "parcela_numero": num,
            "valor_centavos": valor_centavos,
            "valor_fmt": Money::from_centavos(valor_centavos).to_string(),
            "forma_pagamento": payload.forma_pagamento,
            "operador": ator.0.username,
            "data_hora": chrono::Utc::now().to_rfc3339()
        })
    });
    drop(repo);

    if let Err(e) = tx.commit() {
        tracing::error!(erro = %e, "falha ao confirmar liquidação");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": "falha ao confirmar liquidação" })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "sucesso": true,
            "mensagem": "Dívida liquidada com sucesso",
            "valor_liquidado_centavos": valor_centavos,
            "recibo": recibo_json
        })),
    )
}

/// `GET /v1/clientes/:id/exportar` — exporta todos os dados do titular em JSON estruturado (LGPD-003 / Art. 18 LGPD).
///
/// Exige papel de Dono (autorização do controlador).
pub async fn exportar_titular_handler(
    State(state): State<AppState>,
    ator: Ator,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if ator.papel().nivel() < 3 {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "erro": "Apenas o Dono da loja pode autorizar a exportação de dados do titular (LGPD Art. 18)"
            })),
        );
    }

    let db = state.db.lock().await;
    let repo = PessoaRepo::com_chave(db.conn(), &state.chave_loja);

    let pessoa = match repo.buscar_por_id(&id) {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "erro": "Titular não encontrado" })),
            );
        }
        Err(e) => {
            tracing::error!(erro = %e, "falha ao buscar titular para exportação");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha ao consultar dados do titular" })),
            );
        }
    };

    // Auditoria obrigatória (LGPD-002 / FRD-053)
    let audit = mikella_infra_sqlite::AuditoriaRepo::novo(db.conn());
    if let Err(e) = audit.registrar(
        "cliente",
        &id,
        "EXPORTACAO_DADOS_TITULAR",
        Some(&ator.id().to_string()),
        None,
        None,
        Some("exportacao_json_lgpd_art18"),
    ) {
        tracing::error!(erro = %e, "falha ao registrar auditoria de exportação LGPD");
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "versao_schema": "mikella.lgpd.export.v1",
            "base_legal": "Art. 18, incisos II e V da Lei nº 13.709/2018 (LGPD)",
            "controlador": "Mikella ERP Local Store",
            "exportado_em": chrono::Utc::now().to_rfc3339(),
            "autorizado_por": {
                "usuario_id": ator.id().to_string(),
                "username": ator.0.username,
                "papel": ator.papel().as_db(),
            },
            "titular": {
                "id": pessoa.id,
                "tipo": pessoa.tipo.as_db(),
                "nome": pessoa.nome,
                "cpf_cnpj": pessoa.documento,
                "telefone": pessoa.telefone,
                "email": pessoa.email,
                "endereco": pessoa.endereco,
                "ativo": pessoa.ativo,
                "criado_em": pessoa.criado_em.to_rfc3339(),
                "atualizado_em": pessoa.atualizado_em.to_rfc3339(),
            }
        })),
    )
}

/// `DELETE /v1/clientes/:id`
/// Anonimiza os dados pessoais do cliente em cumprimento ao direito de eliminação (Art. 16/18 LGPD).
/// Os registros fiscais de vendas e livro-caixa são preservados conforme exigência legal.
pub async fn excluir_ou_anonimizar_cliente_handler(
    State(state): State<AppState>,
    ator: Ator,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    exigir!(ator, "clientes.gerenciar");
    let db = state.db.lock().await;
    let repo = PessoaRepo::com_chave(db.conn(), &state.chave_loja);

    match repo.anonimizar_cliente(&id) {
        Ok(true) => {
            let audit = mikella_infra_sqlite::AuditoriaRepo::novo(db.conn());
            let _ = audit.registrar(
                "cliente",
                &id,
                "ANONIMIZACAO_TITULAR_LGPD",
                Some(&ator.id().to_string()),
                None,
                None,
                Some("direito_eliminacao_art16_lgpd"),
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "sucesso": true,
                    "mensagem": "Dados pessoais do cliente anonimizados com sucesso. Registros históricos de vendas preservados para cumprimento de obrigação fiscal e legal (Art. 16, I da LGPD)."
                })),
            )
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "sucesso": false, "erro": "Cliente não encontrado" })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "sucesso": false, "erro": e.to_string() })),
        ),
    }
}
