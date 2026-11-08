//! Handlers de contas a pagar / receber (RF-030).
//!
//! Rotas:
//! - `POST /v1/finance/contas-pagar` — registrar conta a pagar
//! - `POST /v1/finance/contas-receber` — registrar conta a receber
//! - `POST /v1/finance/contas-pagar/:id/parcelas/:idx/liquidar`
//! - `POST /v1/finance/contas-receber/:id/parcelas/:idx/liquidar`
//! - `GET /v1/finance/contas-abertas?tipo=pagamento|recebimento`
//!
//! Toda escrita roda em uma única transação SQLite: conta + parcelas são
//! gravadas juntas ou não são gravadas. O ator vem do token, nunca do corpo.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use chrono_tz::Tz;

use mikella_app_finance::contas::{
    listar_contas as listar_contas_uc, liquidar_parcela_pagar as liquidar_pagar_uc,
    liquidar_parcela_receber as liquidar_receber_uc, registrar_conta_pagar as registrar_pagar_uc,
    registrar_conta_receber as registrar_receber_uc, ContasRepos, ListaContasQuery,
    LiquidarParcelaPagarCmd, LiquidarParcelaReceberCmd, RegistrarContaPagarCmd,
    RegistrarContaReceberCmd,
};
use mikella_infra_sqlite::{
    ContaPagarRepo, ContaReceberRepo, DiagnosticoRepo, FinanceiroDashboardRepo, ParcelaRepo,
};
use mikella_shared_kernel::ids::{ContaPagarId, ContaReceberId};
use mikella_shared_kernel::money::Money;

use crate::dto::{
    FluxoCaixaQuery, ListarContasQuery, ParcelaForm, RegistrarContaPagarForm,
    RegistrarContaReceberForm, ResultadoDREQuery, ResumoDashboardQuery,
};
use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::util::{bad_request, error_para_http};

/// `POST /v1/finance/contas-pagar`
/// Registra uma conta a pagar com suas parcelas.
pub async fn registrar_conta_pagar_handler(
    State(state): State<AppState>,
    ator: Ator,
    Json(form): Json<RegistrarContaPagarForm>,
) -> impl IntoResponse {
    exigir!(ator, "financeiro.gerenciar");
    tracing::debug!(operador = %ator.0.username, "registrando conta a pagar");

    let parcelas = match validar_form(&form.descricao, &form.plano_codigo, &form.plano_nome, &form.parcelas) {
        Ok(p) => p,
        Err(msg) => return bad_request(msg),
    };

    let cmd = RegistrarContaPagarCmd {
        descricao: form.descricao,
        plano_codigo: form.plano_codigo,
        plano_nome: form.plano_nome,
        parcelas,
        usuario_id: ator.id(),
    };

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao abrir transação de conta a pagar");
            return erro_interno();
        }
    };

    let resultado = {
        let pagar_repo = ContaPagarRepo::novo(&tx);
        let receber_repo = ContaReceberRepo::novo(&tx);
        let parcela_repo = ParcelaRepo::novo(&tx);
        let repos = ContasRepos {
            contas_a_pagar: &pagar_repo,
            contas_a_receber: &receber_repo,
            parcelas: &parcela_repo,
        };
        registrar_pagar_uc(&repos, cmd, Utc::now())
    };

    match resultado {
        Ok(id) => match tx.commit() {
            Ok(()) => (
                StatusCode::CREATED,
                Json(serde_json::json!({ "id": id.to_string(), "tipo": "pagamento" })),
            ),
            Err(e) => {
                tracing::error!(erro = %e, "falha ao confirmar conta a pagar");
                erro_interno()
            }
        },
        Err(e) => {
            let _ = tx.rollback();
            tracing::warn!(erro = %e, "registro de conta a pagar rejeitado");
            error_para_http(e)
        }
    }
}

/// `POST /v1/finance/contas-receber`
/// Registra uma conta a receber com suas parcelas.
pub async fn registrar_conta_receber_handler(
    State(state): State<AppState>,
    ator: Ator,
    Json(form): Json<RegistrarContaReceberForm>,
) -> impl IntoResponse {
    exigir!(ator, "financeiro.gerenciar");
    tracing::debug!(operador = %ator.0.username, "registrando conta a receber");

    let parcelas = match validar_form(&form.descricao, &form.plano_codigo, &form.plano_nome, &form.parcelas) {
        Ok(p) => p,
        Err(msg) => return bad_request(msg),
    };

    let cmd = RegistrarContaReceberCmd {
        descricao: form.descricao,
        plano_codigo: form.plano_codigo,
        plano_nome: form.plano_nome,
        parcelas,
        usuario_id: ator.id(),
    };

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao abrir transação de conta a receber");
            return erro_interno();
        }
    };

    let resultado = {
        let pagar_repo = ContaPagarRepo::novo(&tx);
        let receber_repo = ContaReceberRepo::novo(&tx);
        let parcela_repo = ParcelaRepo::novo(&tx);
        let repos = ContasRepos {
            contas_a_pagar: &pagar_repo,
            contas_a_receber: &receber_repo,
            parcelas: &parcela_repo,
        };
        registrar_receber_uc(&repos, cmd, Utc::now())
    };

    match resultado {
        Ok(id) => match tx.commit() {
            Ok(()) => (
                StatusCode::CREATED,
                Json(serde_json::json!({ "id": id.to_string(), "tipo": "recebimento" })),
            ),
            Err(e) => {
                tracing::error!(erro = %e, "falha ao confirmar conta a receber");
                erro_interno()
            }
        },
        Err(e) => {
            let _ = tx.rollback();
            tracing::warn!(erro = %e, "registro de conta a receber rejeitado");
            error_para_http(e)
        }
    }
}

/// `POST /v1/finance/contas-pagar/:id/parcelas/:idx/liquidar`
pub async fn liquidar_parcela_handler(
    State(state): State<AppState>,
    Path((id_str, idx_str)): Path<(String, String)>,
    ator: Ator,
) -> impl IntoResponse {
    exigir!(ator, "financeiro.gerenciar");
    tracing::debug!(operador = %ator.0.username, conta_id = %id_str, "liquidando parcela a pagar");

    let conta_id: ContaPagarId = match id_str.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("id inválido (UUID esperado)"),
    };
    let idx: usize = match idx_str.parse() {
        Ok(i) => i,
        Err(_) => return bad_request("índice de parcela inválido"),
    };

    let cmd = LiquidarParcelaPagarCmd {
        conta_id,
        parcela_indice: idx,
        pago_em: Utc::now(),
    };

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao abrir transação de liquidação");
            return erro_interno();
        }
    };

    let resultado = {
        let pagar_repo = ContaPagarRepo::novo(&tx);
        let receber_repo = ContaReceberRepo::novo(&tx);
        let parcela_repo = ParcelaRepo::novo(&tx);
        let repos = ContasRepos {
            contas_a_pagar: &pagar_repo,
            contas_a_receber: &receber_repo,
            parcelas: &parcela_repo,
        };
        liquidar_pagar_uc(&repos, cmd)
    };

    match resultado {
        Ok(()) => match tx.commit() {
            Ok(()) => (
                StatusCode::OK,
                Json(serde_json::json!({
                    "liquidado": true,
                    "conta_id": id_str,
                    "parcela_indice": idx
                })),
            ),
            Err(e) => {
                tracing::error!(erro = %e, "falha ao confirmar liquidação");
                erro_interno()
            }
        },
        Err(e) => {
            let _ = tx.rollback();
            tracing::warn!(erro = %e, "liquidação rejeitada");
            error_para_http(e)
        }
    }
}

/// `POST /v1/finance/contas-receber/:id/parcelas/:idx/liquidar`
pub async fn liquidar_parcela_receber_handler(
    State(state): State<AppState>,
    Path((id_str, idx_str)): Path<(String, String)>,
    ator: Ator,
) -> impl IntoResponse {
    exigir!(ator, "financeiro.gerenciar");
    tracing::debug!(operador = %ator.0.username, conta_id = %id_str, "liquidando parcela a receber");

    let conta_id: ContaReceberId = match id_str.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("id inválido (UUID esperado)"),
    };
    let idx: usize = match idx_str.parse() {
        Ok(i) => i,
        Err(_) => return bad_request("índice de parcela inválido"),
    };

    let cmd = LiquidarParcelaReceberCmd {
        conta_id,
        parcela_indice: idx,
        pago_em: Utc::now(),
    };

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao abrir transação de liquidação");
            return erro_interno();
        }
    };

    let resultado = {
        let pagar_repo = ContaPagarRepo::novo(&tx);
        let receber_repo = ContaReceberRepo::novo(&tx);
        let parcela_repo = ParcelaRepo::novo(&tx);
        let repos = ContasRepos {
            contas_a_pagar: &pagar_repo,
            contas_a_receber: &receber_repo,
            parcelas: &parcela_repo,
        };
        liquidar_receber_uc(&repos, cmd)
    };

    match resultado {
        Ok(()) => match tx.commit() {
            Ok(()) => (
                StatusCode::OK,
                Json(serde_json::json!({
                    "liquidado": true,
                    "conta_id": id_str,
                    "parcela_indice": idx
                })),
            ),
            Err(e) => {
                tracing::error!(erro = %e, "falha ao confirmar liquidação");
                erro_interno()
            }
        },
        Err(e) => {
            let _ = tx.rollback();
            tracing::warn!(erro = %e, "liquidação rejeitada");
            error_para_http(e)
        }
    }
}

/// `GET /v1/finance/contas-abertas?tipo=pagamento|recebimento`
pub async fn listar_contas_abertas_handler(
    State(state): State<AppState>,
    ator: Ator,
    Query(query): Query<ListarContasQuery>,
) -> impl IntoResponse {
    exigir!(ator, "financeiro.ver");
    tracing::debug!(operador = %ator.0.username, tipo = ?query.tipo, "listando contas abertas");

    let db = state.db.lock().await;
    let pagar_repo = ContaPagarRepo::novo(db.conn());
    let receber_repo = ContaReceberRepo::novo(db.conn());
    let parcela_repo = ParcelaRepo::novo(db.conn());
    let repos = ContasRepos {
        contas_a_pagar: &pagar_repo,
        contas_a_receber: &receber_repo,
        parcelas: &parcela_repo,
    };

    match listar_contas_uc(
        &repos,
        ListaContasQuery {
            tipo: query.tipo,
        },
    ) {
        Ok(contas) => (
            StatusCode::OK,
            Json(serde_json::json!({ "contas": contas })),
        ),
        Err(e) => {
            tracing::error!(erro = %e, "erro ao listar contas");
            error_para_http(e)
        }
    }
}

fn validar_form(
    descricao: &str,
    plano_codigo: &str,
    plano_nome: &str,
    parcelas: &[ParcelaForm],
) -> Result<Vec<(Money, DateTime<Utc>)>, &'static str> {
    if descricao.trim().is_empty() {
        return Err("descrição não pode ser vazia");
    }
    if plano_codigo.trim().is_empty() || plano_nome.trim().is_empty() {
        return Err("plano de contas é obrigatório");
    }
    if parcelas.is_empty() {
        return Err("pelo menos uma parcela é obrigatória");
    }
    if parcelas.len() > 12 {
        return Err("máximo de 12 parcelas");
    }

    let mut resultado = Vec::with_capacity(parcelas.len());
    for p in parcelas {
        if p.valor_centavos <= 0 {
            return Err("valor da parcela deve ser maior que zero");
        }
        let vencimento = DateTime::parse_from_rfc3339(&p.vencimento)
            .map_err(|_| "vencimento inválido (RFC3339 esperado)")?
            .with_timezone(&Utc);
        resultado.push((Money::from_centavos(p.valor_centavos), vencimento));
    }
    Ok(resultado)
}

/// Calcula a janela do resumo no fuso IANA da loja.
///
/// O `date('now')` do SQLite é UTC: entre 21h e 0h em `America/Sao_Paulo` ele
/// apontaria para o dia seguinte. Aqui a data é resolvida no fuso da loja e
/// entregue ao repositório já como `AAAA-MM-DD` (validada, sem interpolação
/// de entrada crua no SQL).
fn resolver_janela_resumo(
    fuso_iana: &str,
    periodo: &str,
    data_inicio: Option<&str>,
    data_fim: Option<&str>,
) -> Result<(String, String), (StatusCode, Json<serde_json::Value>)> {
    fn validar(data: &str) -> Result<NaiveDate, (StatusCode, Json<serde_json::Value>)> {
        NaiveDate::parse_from_str(data.trim(), "%Y-%m-%d")
            .map_err(|_| bad_request("data inválida (use AAAA-MM-DD)"))
    }

    if let Some(ini) = data_inicio.map(str::trim).filter(|d| !d.is_empty()) {
        let ini = validar(ini)?;
        let fim = match data_fim.map(str::trim).filter(|d| !d.is_empty()) {
            Some(fim) => validar(fim)?,
            None => ini,
        };
        if fim < ini {
            return Err(bad_request("data final anterior à inicial"));
        }
        return Ok((ini.to_string(), fim.to_string()));
    }

    let tz: Tz = fuso_iana.parse().unwrap_or(chrono_tz::America::Sao_Paulo);
    let hoje = Utc::now().with_timezone(&tz).date_naive();
    let (inicio, fim) = match periodo {
        "semana" => (hoje - chrono::Duration::days(6), hoje),
        "mes" => (
            NaiveDate::from_ymd_opt(hoje.year(), hoje.month(), 1).unwrap_or(hoje),
            hoje,
        ),
        _ => (hoje, hoje),
    };
    Ok((inicio.to_string(), fim.to_string()))
}

/// `GET /v1/finance/resumo?periodo=hoje|semana|mes`
pub async fn obter_resumo_dashboard_handler(
    State(state): State<AppState>,
    ator: Ator,
    Query(query): Query<ResumoDashboardQuery>,
) -> impl IntoResponse {
    exigir!(ator, "financeiro.ver");
    let periodo = query.periodo.as_deref().unwrap_or("hoje");
    tracing::debug!(operador = %ator.0.username, periodo, "obtendo resumo financeiro do dashboard");

    let db = state.db.lock().await;
    let fuso = DiagnosticoRepo::novo(db.conn()).fuso_horario();
    let (data_inicio, data_fim) = match resolver_janela_resumo(
        &fuso,
        periodo,
        query.data_inicio.as_deref(),
        query.data_fim.as_deref(),
    ) {
        Ok(janela) => janela,
        Err(resposta) => return resposta,
    };

    let repo = FinanceiroDashboardRepo::novo(db.conn());

    match repo.obter_resumo(periodo, Some(&data_inicio), Some(&data_fim), &fuso) {
        Ok(resumo) => (
            StatusCode::OK,
            Json(serde_json::to_value(resumo).unwrap_or_default()),
        ),
        Err(e) => {
            tracing::error!(erro = %e, "erro ao obter resumo financeiro");
            erro_interno()
        }
    }
}

/// `GET /v1/finance/fluxo-caixa?inicio=...&fim=...`
pub async fn obter_fluxo_caixa_handler(
    State(state): State<AppState>,
    ator: Ator,
    Query(query): Query<FluxoCaixaQuery>,
) -> impl IntoResponse {
    exigir!(ator, "financeiro.ver");
    tracing::debug!(operador = %ator.0.username, "obtendo extrato de fluxo de caixa");

    let db = state.db.lock().await;
    let repo = FinanceiroDashboardRepo::novo(db.conn());

    match repo.obter_fluxo_caixa(query.inicio.as_deref(), query.fim.as_deref()) {
        Ok(fluxo) => (
            StatusCode::OK,
            Json(serde_json::to_value(fluxo).unwrap_or_default()),
        ),
        Err(e) => {
            tracing::error!(erro = %e, "erro ao obter fluxo de caixa");
            erro_interno()
        }
    }
}

/// `GET /v1/finance/resultado?mes=AAAA-MM`
pub async fn obter_resultado_dre_handler(
    State(state): State<AppState>,
    ator: Ator,
    Query(query): Query<ResultadoDREQuery>,
) -> impl IntoResponse {
    exigir!(ator, "financeiro.ver");
    tracing::debug!(operador = %ator.0.username, mes = ?query.mes, "obtendo DRE simplificada");

    let db = state.db.lock().await;
    let repo = FinanceiroDashboardRepo::novo(db.conn());

    match repo.obter_resultado_dre(query.mes.as_deref()) {
        Ok(dre) => (
            StatusCode::OK,
            Json(serde_json::to_value(dre).unwrap_or_default()),
        ),
        Err(e) => {
            tracing::error!(erro = %e, "erro ao obter DRE simplificada");
            erro_interno()
        }
    }
}

fn erro_interno() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "erro": "falha de banco" })),
    )
}
