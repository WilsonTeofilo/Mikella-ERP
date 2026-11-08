//! Handlers de caixa (MVP-1): status, abertura e fechamento.
//!
//! Todos os handlers aqui usam transações SQLite e o caso de uso do domínio
//! (`app_cash::abrir_caixa` / `app_cash::fechar_caixa`).
//! `operador_id` é sempre derivado do usuário autenticado — nunca do cliente.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use mikella_app_cash::{abrir_caixa, fechar_caixa, AbrirCaixaCmd, CaixaRepos, FecharCaixaCmd};
use mikella_infra_sqlite::ports::SessaoRepository;
use mikella_infra_sqlite::{Blake3HashChain, LivroRepo, SessaoRepo};
use mikella_shared_kernel::clock::SystemClock;
use mikella_shared_kernel::ids::{DispositivoId, SessaoId};
use mikella_shared_kernel::money::Money;

use crate::dto::{AbrirCaixaForm, FecharCaixaForm, SangriaForm};
use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::util::{bad_request, error_para_http, garantir_dispositivo};

/// `GET /v1/caixa/status` — status da sessão de caixa aberta (se houver).
///
/// Usa `SessaoRepo` (infra) implementando a porta `SessaoRepository`
/// (domínio cash). Apenas leitura — não altera estado.
pub async fn caixa_status(State(state): State<AppState>, ator: Ator) -> impl IntoResponse {
    exigir!(ator, "caixa.ver");
    tracing::debug!(usuario = %ator.0.username, "consultando status da caixa");

    let db = state.db.lock().await;
    let sessoes = SessaoRepo::novo(db.conn());

    let abertas = match sessoes.buscar_sessoes_abertas() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(erro = %e, "erro ao buscar sessões abertas");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"erro": "falha ao consultar caixa"})),
            );
        }
    };

    match abertas.first() {
        Some(sessao) => {
            let saldo_oculto = sessao.fechamento_cego && ator.papel().nivel() < 2;
            let (saldo_centavos, saldo_display) = if saldo_oculto {
                (0, "Oculto (Fechamento Cego)".to_string())
            } else {
                (
                    sessao.saldo_esperado_centavos.centavos(),
                    sessao.saldo_esperado_centavos.to_string(),
                )
            };
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "aberta": true,
                    "sessao_id": sessao.id.to_string(),
                    "numero_caixa": sessao.numero_caixa,
                    "saldo_esperado_centavos": saldo_centavos,
                    "saldo_esperado": saldo_display,
                    "fechamento_cego": sessao.fechamento_cego,
                    "saldo_oculto": saldo_oculto,
                    "estado": sessao.estado.as_str(),
                    "aberta_em": sessao.aberta_em.to_rfc3339(),
                    "operador_id": sessao.operador_id.to_string(),
                })),
            )
        }
        None => (
            StatusCode::OK,
            Json(serde_json::json!({
                "aberta": false,
                "sessao_id": null,
                "numero_caixa": null,
                "saldo_esperado_centavos": 0,
                "saldo_esperado": Money::ZERO.to_string(),
                "estado": "FECHADA",
                "aberta_em": null,
                "operador_id": null,
            })),
        ),
    }
}

/// `POST /v1/caixa/abrir` — abre uma sessão de caixa.
///
/// Usa `app_cash::abrir_caixa` com repositórios SQLite dentro de uma transação.
/// Idempotente: reenviar o mesmo `sessao_id` devolve a sessão existente.
/// Se já houver uma sessão aberta para o caixa, devolve a sessão existente com sucesso.
#[allow(clippy::too_many_lines)]
pub async fn abrir_caixa_handler(
    State(state): State<AppState>,
    ator: Ator,
    req: axum::extract::Request,
) -> impl IntoResponse {
    exigir!(ator, "caixa.abrir");

    let bytes = match axum::body::to_bytes(req.into_body(), 64 * 1024).await {
        Ok(b) => b,
        Err(_) => return bad_request("corpo de requisição inválido"),
    };

    let form: AbrirCaixaForm = if let Ok(f) = serde_json::from_slice(&bytes) {
        f
    } else if let Ok(f) = serde_urlencoded::from_bytes(&bytes) {
        f
    } else {
        return bad_request("formato inválido (esperado JSON ou URL-encoded)");
    };

    let sessao_id: SessaoId = match form.sessao_id.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("sessao_id inválido (UUID esperado)"),
    };
    let dispositivo_id: DispositivoId = match form.dispositivo_id.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("dispositivo_id inválido (UUID esperado)"),
    };

    // operador_id é derivado do usuário autenticado, nunca do cliente.
    let operador_id = ator.id();

    let saldo_inicial = Money::from_centavos(form.saldo_inicial_centavos);

    let mut db = state.db.lock().await;

    // MVP-1: auto-provisioning do dispositivo. Em produção (MVP-2), o
    // dispositivo é criado durante o pareamento TLS e já existe aqui.
    let dispositivo_id_str = form.dispositivo_id.clone();
    if let Err(e) = garantir_dispositivo(&db, &dispositivo_id_str) {
        tracing::error!(erro = %e, "falha ao garantir dispositivo");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"erro": "falha de banco"})),
        );
    }

    let tx = match db.transacao() {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao iniciar transação");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"erro": "falha de banco"})),
            );
        }
    };

    let sessoes = SessaoRepo::novo(&tx);

    // Se já existir uma sessão aberta para este número de caixa, retorna a sessão existente de forma idempotente
    if let Ok(abertas) = sessoes.buscar_sessoes_abertas() {
        if let Some(conflito) = abertas.iter().find(|s| s.numero_caixa == form.numero_caixa) {
            let _ = tx.rollback();
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "sessao_id": conflito.id.to_string(),
                    "numero_caixa": conflito.numero_caixa,
                    "estado": conflito.estado.as_str(),
                    "saldo_inicial": conflito.saldo_inicial_centavos.to_string(),
                })),
            );
        }
    }

    let livro = LivroRepo::novo(&tx);
    let hash = Blake3HashChain;
    let clock = SystemClock;
    let repos = CaixaRepos {
        sessoes: &sessoes,
        livro: &livro,
        hash: &hash,
        clock: &clock,
    };

    let cmd = AbrirCaixaCmd {
        sessao_id,
        numero_caixa: form.numero_caixa,
        dispositivo_id,
        operador_id,
        saldo_inicial,
        fechamento_cego: form.fechamento_cego,
    };

    match abrir_caixa(&repos, cmd) {
        Ok(sessao) => {
            let audit = mikella_infra_sqlite::AuditoriaRepo::novo(&tx);
            let _ = audit.registrar(
                "sessao_caixa",
                &sessao.id.to_string(),
                "ABERTURA_CAIXA",
                Some(&ator.id().to_string()),
                Some(&dispositivo_id.to_string()),
                None,
                Some(&format!(
                    "caixa: {}, saldo_inicial: {}",
                    sessao.numero_caixa, sessao.saldo_inicial_centavos
                )),
            );

            if tx.commit().is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"erro": "falha ao confirmar caixa"})),
                );
            }
            tracing::info!(
                usuario = %ator.0.username,
                sessao_id = %sessao.id,
                numero_caixa = sessao.numero_caixa,
                "caixa aberto"
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "sessao_id": sessao.id.to_string(),
                    "numero_caixa": sessao.numero_caixa,
                    "estado": sessao.estado.as_str(),
                    "saldo_inicial": sessao.saldo_inicial_centavos.to_string(),
                })),
            )
        }
        Err(e) => {
            let _ = tx.rollback();
            error_para_http(e)
        }
    }
}

/// `POST /v1/caixa/fechar` — fecha uma sessão de caixa aberta.
///
/// Usa `app_cash::fechar_caixa` com transação atômica. Verifica a cadeia de
/// hashes do livro antes de permitir o fechamento — se alguém alterou o banco
/// diretamente, o fechamento é recusado.
pub async fn fechar_caixa_handler(
    State(state): State<AppState>,
    ator: Ator,
    req: axum::extract::Request,
) -> impl IntoResponse {
    exigir!(ator, "caixa.fechar");

    let bytes = match axum::body::to_bytes(req.into_body(), 64 * 1024).await {
        Ok(b) => b,
        Err(_) => return bad_request("corpo de requisição inválido"),
    };

    let form: FecharCaixaForm = if let Ok(f) = serde_json::from_slice(&bytes) {
        f
    } else if let Ok(f) = serde_urlencoded::from_bytes(&bytes) {
        f
    } else {
        return bad_request("formato inválido (esperado JSON ou URL-encoded)");
    };

    let sessao_id: SessaoId = match form.sessao_id.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("sessao_id inválido (UUID esperado)"),
    };
    let saldo_contado = Money::from_centavos(form.saldo_contado_centavos);
    if saldo_contado.is_negative() {
        return bad_request("saldo contado não pode ser negativo");
    }

    let observacao = form
        .observacao
        .as_deref()
        .map(str::trim)
        .filter(|o| !o.is_empty())
        .map(|o| {
            if o.chars().count() > 512 {
                o.chars().take(512).collect::<String>()
            } else {
                o.to_string()
            }
        });

    let operador_id = ator.id();

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao iniciar transação");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"erro": "falha de banco"})),
            );
        }
    };

    let sessoes = SessaoRepo::novo(&tx);
    let livro = LivroRepo::novo(&tx);
    let hash = Blake3HashChain;
    let clock = SystemClock;

    // RF-012/RF-014: a sessão pertence a um operador. O próprio operador
    // fecha a sessão; apenas gerente/dono fecham sessão de terceiros.
    match sessoes.buscar_por_id(sessao_id) {
        Ok(Some(sessao)) => {
            if sessao.operador_id != ator.id() && ator.papel().nivel() < 2 {
                let _ = tx.rollback();
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({ "erro": "sessão pertence a outro operador" })),
                );
            }
        }
        Ok(None) => {
            let _ = tx.rollback();
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "erro": "sessão de caixa não encontrada" })),
            );
        }
        Err(e) => {
            let _ = tx.rollback();
            return error_para_http(e);
        }
    }

    let repos = CaixaRepos {
        sessoes: &sessoes,
        livro: &livro,
        hash: &hash,
        clock: &clock,
    };

    let cmd = FecharCaixaCmd {
        sessao_id,
        usuario_id: operador_id,
        saldo_contado,
        observacao,
    };

    match fechar_caixa(&repos, cmd) {
        Ok(diferenca) => {
            let audit = mikella_infra_sqlite::AuditoriaRepo::novo(&tx);
            let _ = audit.registrar(
                "sessao_caixa",
                &sessao_id.to_string(),
                "FECHAMENTO_CAIXA",
                Some(&ator.id().to_string()),
                None,
                Some(&format!("esperado: {}", diferenca.esperado)),
                Some(&format!(
                    "contado: {}, diferenca: {}",
                    diferenca.contado, diferenca.diferenca
                )),
            );

            if tx.commit().is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"erro": "falha ao confirmar fechamento"})),
                );
            }
            tracing::info!(
                usuario = %ator.0.username,
                sessao_id = %sessao_id,
                "caixa fechado"
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "esperado": diferenca.esperado.to_string(),
                    "contado": diferenca.contado.to_string(),
                    "diferenca": diferenca.diferenca.to_string(),
                    "tem_diferenca": diferenca.tem_diferenca(),
                })),
            )
        }
        Err(e) => {
            let _ = tx.rollback();
            error_para_http(e)
        }
    }
}

/// `POST /v1/caixa/sangria` — realiza retirada de dinheiro da gaveta com lançamento criptográfico.
pub async fn realizar_sangria_handler(
    State(state): State<AppState>,
    ator: Ator,
    req: axum::extract::Request,
) -> impl IntoResponse {
    exigir!(ator, "venda.sangria");

    let bytes = match axum::body::to_bytes(req.into_body(), 64 * 1024).await {
        Ok(b) => b,
        Err(_) => return bad_request("corpo de requisição inválido"),
    };

    let form: SangriaForm = if let Ok(f) = serde_json::from_slice(&bytes) {
        f
    } else if let Ok(f) = serde_urlencoded::from_bytes(&bytes) {
        f
    } else {
        return bad_request("formato inválido (esperado JSON ou URL-encoded)");
    };

    let sessao_id: SessaoId = match form.sessao_id.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("sessao_id inválido (UUID esperado)"),
    };

    if form.valor_centavos <= 0 {
        return bad_request("Valor da sangria deve ser maior que zero");
    }

    let operador_id = ator.id();
    let motivo = form
        .motivo
        .clone()
        .unwrap_or_else(|| "Sangria de caixa".to_string());
    let motivo_truncado: String = motivo.trim().chars().take(256).collect();

    // SEC-006: sangria exige aprovador real e diferente do operador.
    let aprovacao =
        match crate::util::exigir_aprovador(&state, &ator, form.credenciais_aprovador(), "sangria")
            .await
        {
            Ok(a) => a,
            Err(resposta) => return resposta,
        };

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao abrir transação de sangria");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha de banco" })),
            );
        }
    };

    use mikella_app_cash::caixa::registrar_sangria;

    let sessoes = SessaoRepo::novo(&tx);
    let livro = LivroRepo::novo(&tx);

    // SEC-006: operador só movimenta o caixa em que é o operador da sessão.
    if ator.papel().nivel() < 2 {
        use mikella_domain_cash::ports::SessaoRepository as _;
        match sessoes.buscar_por_id(sessao_id) {
            Ok(Some(sessao)) if sessao.operador_id == operador_id => {}
            Ok(_) => {
                let _ = tx.rollback();
                return crate::middleware::proibido(
                    "operador só pode fazer sangria no próprio caixa",
                );
            }
            Err(e) => {
                let _ = tx.rollback();
                tracing::error!(erro = %e, "falha ao validar sessão da sangria");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha de banco" })),
                );
            }
        }
    }

    let hash = Blake3HashChain;
    let clock = SystemClock;
    let repos = CaixaRepos {
        sessoes: &sessoes,
        livro: &livro,
        hash: &hash,
        clock: &clock,
    };

    match registrar_sangria(
        &repos,
        sessao_id,
        Money::from_centavos(form.valor_centavos),
        operador_id,
        aprovacao.id,
        motivo_truncado.clone(),
    ) {
        Ok(()) => {
            let audit = mikella_infra_sqlite::AuditoriaRepo::novo(&tx);
            let _ = audit.registrar(
                "sangria",
                &sessao_id.to_string(),
                "SANGRIA",
                Some(&operador_id.to_string()),
                None,
                None,
                Some(&format!(
                    "valor_centavos: {}, motivo: {}, aprovador: {}",
                    form.valor_centavos, motivo_truncado, aprovacao.id
                )),
            );

            if tx.commit().is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "Erro ao confirmar sangria" })),
                );
            }

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "sucesso": true,
                    "mensagem": "Sangria realizada com sucesso!",
                    "valor": Money::from_centavos(form.valor_centavos).to_string(),
                    "motivo": motivo_truncado,
                })),
            )
        }
        Err(e) => {
            let _ = tx.rollback();
            error_para_http(e)
        }
    }
}
