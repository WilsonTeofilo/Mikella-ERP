//! Handler de finalização de venda (MVP-1).
//!
//! Usa `app_sales::finalizar_venda` com repositórios SQLite dentro de uma
//! transação. O cliente envia IDs e quantidades; preços e custos vêm do banco.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use mikella_app_sales::{
    finalizar_venda, CrediarioCmd, FinalizarVendaCmd, ItemCmd, PoliticaEstoque, VendaRepos,
};
use mikella_domain_sales::venda::{FormaPagamento, Pagamento};
use mikella_infra_hardware::{EscPosGerador, LarguraPapel};
use mikella_infra_sqlite::{
    Blake3HashChain, ContaReceberRepo, DiagnosticoRepo, LivroRepo, LoteRepo, MovimentacaoRepo,
    ProdutoRepo, SessaoRepo, VendaRepo,
};
use mikella_shared_kernel::clock::SystemClock;
use mikella_shared_kernel::ids::{DispositivoId, LoteId, ProdutoId, SessaoId, VendaId};
use mikella_shared_kernel::money::Money;

use crate::dto::FinalizarVendaForm;
use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::util::{bad_request, error_para_http, garantir_dispositivo};

/// `POST /v1/vendas` — finaliza uma venda.
///
/// Idempotente: reenvio com o mesmo `venda_id` devolve a venda já finalizada.
/// Preço e estoque vêm do servidor — a UI só envia IDs, quantidades e forma de pagamento.
///
/// Exemplo de request JSON:
/// ```json
/// {
///   "venda_id": "uuid-v4",
///   "sessao_id": "uuid",
///   "dispositivo_id": "uuid",
///   "itens": [{"produto_id": "uuid", "lote_id": null, "quantidade": 2}],
///   "pagamentos": [{"forma": "PIX", "valor_centavos": 25980}],
///   "desconto_centavos": 0,
///   "politica_estoque": "Bloquear"
/// }
/// ```
#[allow(clippy::too_many_lines)]
pub async fn finalizar_venda_handler(
    State(state): State<AppState>,
    ator: Ator,
    Json(form): Json<FinalizarVendaForm>,
) -> impl IntoResponse {
    exigir!(ator, "venda.criar");

    if form.desconto_centavos < 0 {
        return bad_request("desconto não pode ser negativo");
    }
    if form.desconto_centavos > 0 {
        // SEG-022: desconto é decisão do servidor, não um valor livre da UI.
        exigir!(ator, "venda.desconto");
    }

    if form.itens.is_empty() {
        return bad_request("pelo menos um item é obrigatório");
    }
    if form.pagamentos.is_empty() {
        return bad_request("pelo menos um pagamento é obrigatório");
    }

    let venda_id: VendaId = match form.venda_id.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("venda_id inválido (UUID esperado)"),
    };
    let sessao_id: SessaoId = match form.sessao_id.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("sessao_id inválido (UUID esperado)"),
    };
    let dispositivo_id: DispositivoId = match form.dispositivo_id.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("dispositivo_id inválido (UUID esperado)"),
    };

    // operador_id derivado do auth, nunca do cliente
    let operador_id = ator.id();

    // garantir dispositivo
    let dev_str = form.dispositivo_id.clone();
    {
        let db_guard = state.db.lock().await;
        if let Err(e) = garantir_dispositivo(&db_guard, &dev_str) {
            tracing::error!(erro = %e, "falha ao garantir dispositivo");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"erro": "falha de banco"})),
            );
        }
    }

    // itens
    let itens: Vec<ItemCmd> = {
        let mut acc = Vec::with_capacity(form.itens.len());
        for item in &form.itens {
            let produto_id: ProdutoId = match item.produto_id.parse() {
                Ok(id) => id,
                Err(_) => return bad_request("produto_id inválido"),
            };
            let lote_id: Option<LoteId> = match &item.lote_id {
                Some(s) => match s.parse() {
                    Ok(id) => Some(id),
                    Err(_) => return bad_request("lote_id inválido"),
                },
                None => None,
            };
            acc.push(ItemCmd {
                produto_id,
                lote_id,
                quantidade: item.quantidade,
            });
        }
        acc
    };

    // pagamentos
    let pagamentos: Vec<Pagamento> = {
        let mut acc = Vec::with_capacity(form.pagamentos.len());
        for p in &form.pagamentos {
            let forma = match p.forma.to_uppercase().as_str() {
                "DINHEIRO" => FormaPagamento::Dinheiro,
                "DEBITO" => FormaPagamento::Debito,
                "CREDITO" => FormaPagamento::Credito,
                "PIX" => FormaPagamento::Pix,
                "VOUCHER" => FormaPagamento::Voucher,
                "CREDIARIO" => FormaPagamento::Crediario,
                "OUTRO" => FormaPagamento::Outro,
                _ => return bad_request(&format!("forma desconhecida: {}", p.forma)),
            };
            let valor = Money::from_centavos(p.valor_centavos);
            let num_parcelas = p.parcelas.unwrap_or(1).max(1);
            if num_parcelas > 1 && forma != FormaPagamento::Credito {
                return bad_request("Apenas pagamento em cartão de crédito permite parcelamento");
            }
            let taxa = Money::from_centavos(p.taxa_centavos.unwrap_or(0));
            let pag = if forma == FormaPagamento::Dinheiro {
                let recebido_centavos = p.recebido_centavos.unwrap_or(p.valor_centavos);
                let recebido = Money::from_centavos(recebido_centavos);
                match Pagamento::dinheiro(valor, recebido) {
                    Ok(pag_criado) => pag_criado,
                    Err(e) => return bad_request(&e.to_string()),
                }
            } else {
                match Pagamento::eletronico(forma, valor) {
                    Ok(pag_criado) => pag_criado.com_detalhes_cartao(
                        num_parcelas,
                        p.maquininha.clone(),
                        p.modalidade.clone(),
                        taxa,
                    ),
                    Err(e) => return bad_request(&e.to_string()),
                }
            };
            acc.push(pag);
        }
        acc
    };

    let politica = match form.politica_estoque.to_uppercase().as_str() {
        "BLOQUEAR" => PoliticaEstoque::Bloquear,
        "PERMITIR_NEGATIVO" | "PERMITIRNEGATIVO" => PoliticaEstoque::PermitirNegativo,
        _ => return bad_request("politica_estoque inválida (Bloquear|PermitirNegativo)"),
    };

    let crediario = if pagamentos.iter().any(|p| p.forma.e_crediario()) {
        let Some(cliente_id) = form
            .cliente_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        else {
            return bad_request("venda a crediário exige um cliente cadastrado");
        };
        Some(CrediarioCmd {
            cliente_id: cliente_id.to_string(),
            vencimento_dias: 30,
        })
    } else {
        None
    };

    let cmd = FinalizarVendaCmd {
        venda_id,
        sessao_id,
        dispositivo_id,
        operador_id,
        vendedor_id: None,
        comanda_id: form.comanda_id.clone(),
        itens,
        desconto: Money::from_centavos(form.desconto_centavos),
        pagamentos,
        politica_estoque: politica,
        crediario,
    };

    // Transação única: venda + estoque + livro-caixa + crediário.
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

    let produtos = ProdutoRepo::novo(&tx);
    let lotes = LoteRepo::novo(&tx);
    let vendas = VendaRepo::novo(&tx);
    let sessoes = SessaoRepo::novo(&tx);
    let livro = LivroRepo::novo(&tx);
    let estoque = MovimentacaoRepo::novo(&tx);
    let contas_receber = ContaReceberRepo::novo(&tx);
    let hash = Blake3HashChain;
    let clock = SystemClock;
    let repos = VendaRepos {
        produtos: &produtos,
        lotes: &lotes,
        vendas: &vendas,
        sessoes: &sessoes,
        livro: &livro,
        estoque: &estoque,
        hash: &hash,
        clock: &clock,
        contas_receber: Some(&contas_receber),
    };

    match finalizar_venda(&repos, cmd) {
        Ok(venda) => {
            if tx.commit().is_err() {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"erro": "falha ao confirmar venda"})),
                );
            }
            tracing::info!(venda_id = %venda.id, total = %venda.total, "venda finalizada");
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "venda_id": venda.id.to_string(),
                    "sessao_id": venda.sessao_id.to_string(),
                    "operador_id": venda.operador_id.to_string(),
                    "estado": venda.estado.as_str(),
                    "forma_pagamento": venda.forma_pagamento.as_str(),
                    "subtotal": venda.subtotal.to_string(),
                    "desconto": venda.desconto_total.to_string(),
                    "total": venda.total.to_string(),
                    "recebido": venda.recebido.to_string(),
                    "troco": venda.troco.to_string(),
                    "itens": venda.itens.iter().map(|i| serde_json::json!({
                        "produto_id": i.produto_id.to_string(),
                        "quantidade": i.quantidade,
                        "subtotal": i.preco_unitario.mul_qty(i.quantidade).map(|m| m.to_string()).unwrap_or_else(|_| "R$ 0,00".to_string()),
                    })).collect::<Vec<_>>(),
                    "pagamentos": venda.pagamentos.iter().map(|p| serde_json::json!({
                        "forma": p.forma.as_str(),
                        "valor": p.valor.to_string(),
                        "recebido": p.recebido.map(|v| v.to_string()).unwrap_or_else(|| p.valor.to_string()),
                    })).collect::<Vec<_>>(),
                })),
            )
        }
        Err(e) => {
            let _ = tx.rollback();
            error_para_http(e)
        }
    }
}

/// `POST /v1/vendas/autorizar-cancelamento`
///
/// Valida se o usuário e senha têm permissão para cancelar compra no caixa atual:
/// - Dono e Gerente podem cancelar em qualquer caixa.
/// - Operador de caixa comum só pode cancelar no seu próprio caixa (onde é o operador da sessão).
pub async fn autorizar_cancelamento_handler(
    State(state): State<AppState>,
    ator: Ator,
    Json(payload): Json<crate::dto::sales::AutorizarCancelamentoPayload>,
) -> impl IntoResponse {
    exigir!(ator, "venda.cancelar");

    let username = payload.username.trim();
    if username.is_empty() || payload.senha.is_empty() {
        return bad_request("usuário e senha de autorização são obrigatórios");
    }

    // SEG-006: senha de autorização também é alvo de força bruta.
    let chave_rate = format!(
        "autorizar-cancelamento:{}",
        AppState::chave_usuario(username)
    );
    {
        let mut rl = state.rate_limit.lock().await;
        if !rl.permitir(&chave_rate) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "erro": "Muitas tentativas de autorização. Aguarde alguns minutos."
                })),
            );
        }
    }

    let db = state.db.lock().await;

    use mikella_domain_people::ports::UsuarioRepository;
    let resultado_usuario = {
        let usuarios = mikella_infra_sqlite::UsuarioRepo::novo(db.conn());
        usuarios.buscar_por_username(username)
    };
    let usuario = match resultado_usuario {
        Ok(Some(u)) if u.ativo => u,
        Ok(_) => {
            drop(db);
            state.rate_limit.lock().await.registrar_falha(&chave_rate);
            return (
                StatusCode::UNAUTHORIZED,
                Json(
                    serde_json::json!({ "erro": "Usuário de autorização não encontrado ou inativo." }),
                ),
            );
        }
        Err(e) => {
            drop(db);
            tracing::error!(erro = %e, "falha ao buscar usuário autorizador");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha de banco" })),
            );
        }
    };

    if !crate::util::verificar_senha(&payload.senha, &usuario.senha_hash) {
        drop(db);
        state.rate_limit.lock().await.registrar_falha(&chave_rate);
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "erro": "Senha de autorização incorreta." })),
        );
    }

    // Dono ou Gerente podem cancelar em qualquer caixa.
    if usuario.papel.nivel() >= 2 {
        drop(db);
        state.rate_limit.lock().await.limpar(&chave_rate);
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "autorizado": true,
                "usuario_id": usuario.id.to_string(),
                "papel": usuario.papel.as_db(),
                "motivo": "Autorizado por perfil gerencial"
            })),
        );
    }

    // Operador comum: só pode cancelar no próprio caixa.
    let sessao_id = match payload.sessao_id.parse::<SessaoId>() {
        Ok(id) => id,
        Err(_) => return bad_request("sessao_id inválido (UUID esperado)"),
    };
    use mikella_domain_cash::ports::SessaoRepository;
    let resultado_sessao = {
        let sessoes = mikella_infra_sqlite::SessaoRepo::novo(db.conn());
        sessoes.buscar_por_id(sessao_id)
    };
    let sessao = match resultado_sessao {
        Ok(Some(s)) => s,
        Ok(None) => {
            drop(db);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "erro": "Sessão de caixa não encontrada." })),
            );
        }
        Err(e) => {
            drop(db);
            tracing::error!(erro = %e, "falha ao buscar sessão de caixa");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha de banco" })),
            );
        }
    };
    drop(db);

    if sessao.operador_id == usuario.id {
        state.rate_limit.lock().await.limpar(&chave_rate);
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "autorizado": true,
                "usuario_id": usuario.id.to_string(),
                "papel": usuario.papel.as_db(),
                "motivo": "Autorizado pelo próprio operador do caixa"
            })),
        )
    } else {
        (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "erro": "Permissão negada: Um operador só pode cancelar compras no seu próprio caixa. Chame o gerente."
            })),
        )
    }
}

/// `POST /v1/vendas/:id/cancelar` e `POST /v1/vendas/:id/estornar`.
///
/// O estorno é total, atômico e exige aprovador quando quem opera não é
/// gerente/dono (SEC-004). Devolve o comprovante interno para impressão.
pub async fn cancelar_venda_handler(
    State(state): State<AppState>,
    ator: Ator,
    axum::extract::Path(venda_id): axum::extract::Path<String>,
    Json(payload): Json<crate::dto::sales::CancelarVendaPayload>,
) -> impl IntoResponse {
    exigir!(ator, "venda.cancelar");

    // O motivo é obrigatório: o estorno precisa de justificativa no livro e no
    // comprovante. Não confiamos apenas na validação do cliente.
    if payload
        .motivo
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .is_none()
    {
        return crate::util::bad_request("informe o motivo do estorno");
    }

    // SEC-004: a aprovação é consumida aqui, não em um endpoint separado.
    let aprovacao = match crate::util::exigir_aprovador(
        &state,
        &ator,
        payload.credenciais_aprovador(),
        "estorno",
    )
    .await
    {
        Ok(a) => a,
        Err(resposta) => return resposta,
    };
    tracing::info!(
        aprovador = %aprovacao.nome,
        papel = %aprovacao.papel,
        venda_id = %venda_id,
        "estorno autorizado"
    );

    let mut db = state.db.lock().await;
    let tx = match db.transacao() {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao abrir transação de estorno");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "erro interno" })),
            );
        }
    };

    let vid: mikella_shared_kernel::ids::VendaId = match venda_id.parse() {
        Ok(id) => id,
        Err(_) => {
            let _ = tx.rollback();
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "erro": "venda_id inválido (UUID esperado)" })),
            );
        }
    };

    // Valida dono do caixa e sessão aberta antes de qualquer escrita.
    {
        use mikella_domain_cash::ports::SessaoRepository as _;
        use mikella_domain_sales::ports::VendaRepository as _;

        let vendas_leitura = mikella_infra_sqlite::VendaRepo::novo(&tx);
        let venda = match vendas_leitura.buscar_por_id(vid) {
            Ok(Some(v)) => v,
            Ok(None) => {
                let _ = tx.rollback();
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "erro": "venda não encontrada" })),
                );
            }
            Err(e) => {
                let _ = tx.rollback();
                tracing::error!(erro = %e, "falha ao buscar venda para estorno");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha de banco" })),
                );
            }
        };

        let sessoes = mikella_infra_sqlite::SessaoRepo::novo(&tx);
        let sessao = match sessoes.buscar_por_id(venda.sessao_id) {
            Ok(Some(s)) => s,
            Ok(None) => {
                let _ = tx.rollback();
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "erro": "sessão da venda não encontrada" })),
                );
            }
            Err(e) => {
                let _ = tx.rollback();
                tracing::error!(erro = %e, "falha ao buscar sessão da venda");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha de banco" })),
                );
            }
        };

        if ator.papel().nivel() < 2 && sessao.operador_id != ator.id() {
            let _ = tx.rollback();
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "erro": "operador só pode estornar vendas do próprio caixa; chame o gerente"
                })),
            );
        }

        // DAT-008: estorno não pode reescrever um fechamento já feito.
        if !sessao.is_aberta() {
            let _ = tx.rollback();
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "erro": "sessão do caixa já foi fechada; estorno exige caixa aberto"
                })),
            );
        }
    }

    let vendas = mikella_infra_sqlite::VendaRepo::novo(&tx);
    let produtos = mikella_infra_sqlite::ProdutoRepo::novo(&tx);
    let estoque = mikella_infra_sqlite::MovimentacaoRepo::novo(&tx);
    let livro = mikella_infra_sqlite::LivroRepo::novo(&tx);
    let contas_receber = mikella_infra_sqlite::ContaReceberRepo::novo(&tx);
    let hash = mikella_infra_sqlite::Blake3HashChain;
    let repos = mikella_app_sales::CancelarVendaRepos {
        vendas: &vendas,
        produtos: &produtos,
        estoque: &estoque,
        livro: &livro,
        hash: &hash,
        contas_receber: Some(&contas_receber),
    };

    let motivo_audit = payload.motivo.as_deref().unwrap_or("-").to_string();
    let cmd = mikella_app_sales::CancelarVendaCmd {
        venda_id: vid,
        usuario_id: ator.id(),
        aprovador_id: Some(aprovacao.id),
        motivo: payload.motivo,
        observacao: payload.observacao,
    };

    match mikella_app_sales::cancelar_venda(&repos, cmd) {
        Ok(()) => {
            let audit = mikella_infra_sqlite::AuditoriaRepo::novo(&tx);
            let _ = audit.registrar(
                "venda",
                &vid.to_string(),
                "ESTORNO_VENDA",
                Some(&ator.id().to_string()),
                None,
                None,
                Some(&format!(
                    "motivo: {}, aprovador: {}",
                    motivo_audit, aprovacao.id
                )),
            );

            if let Err(e) = tx.commit() {
                tracing::error!(erro = %e, venda_id = %venda_id, "falha ao confirmar estorno");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "erro interno" })),
                );
            }

            // O comprovante é gerado depois do commit. Se falhar, o estorno
            // continua válido e a UI permite reimprimir depois.
            // Usa o id normalizado do domínio (UUID canônico), não o texto cru
            // da URL, para casar com a referência gravada no livro-caixa.
            let venda_id_canonico = vid.to_string();
            match crate::handlers::receipt::carregar_comprovante_estorno(
                db.conn(),
                &venda_id_canonico,
            ) {
                Ok(c) => {
                    let fuso = DiagnosticoRepo::novo(db.conn()).fuso_horario();
                    let texto_58mm =
                        EscPosGerador::gerar_texto_estorno(&c, LarguraPapel::Colunas32, &fuso);
                    let texto_80mm =
                        EscPosGerador::gerar_texto_estorno(&c, LarguraPapel::Colunas48, &fuso);
                    let bytes_58mm =
                        EscPosGerador::gerar_bytes_estorno(&c, LarguraPapel::Colunas32, true, &fuso);
                    let bytes_80mm =
                        EscPosGerador::gerar_bytes_estorno(&c, LarguraPapel::Colunas48, true, &fuso);
                    let hex_58mm: String =
                        bytes_58mm.iter().map(|b| format!("{:02x}", b)).collect();
                    let hex_80mm: String =
                        bytes_80mm.iter().map(|b| format!("{:02x}", b)).collect();

                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "sucesso": true,
                            "mensagem": "Venda estornada com sucesso.",
                            "coo": c.coo,
                            "valor_estornado_centavos": c.valor_estornado.centavos(),
                            "texto_58mm": texto_58mm,
                            "texto_80mm": texto_80mm,
                            "escpos_hex_58mm": hex_58mm,
                            "escpos_hex_80mm": hex_80mm,
                        })),
                    )
                }
                Err((_, msg)) => {
                    tracing::warn!(
                        venda_id = %venda_id,
                        erro = msg,
                        "estorno concluído, mas comprovante não pôde ser montado"
                    );
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "sucesso": true,
                            "mensagem": "Venda estornada com sucesso.",
                            "aviso": "comprovante indisponível; use a reimpressão do histórico",
                        })),
                    )
                }
            }
        }
        Err(e) => {
            let _ = tx.rollback();
            crate::util::error_para_http(e)
        }
    }
}
