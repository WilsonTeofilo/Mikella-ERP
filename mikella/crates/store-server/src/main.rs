//! Mikella Store Server — processo do servidor da loja.
//!
//! Expõe `/v1/health` (loopback), `/v1/login` (loopback) e rotas de dados
//! protegidas por token Bearer.
//!
//! **Rotas de dados só entram depois do pareamento autenticado e do TLS local.**
//! Hoje o servidor escuta apenas em loopback; expor dados antes disso seria uma
//! porta aberta na rede da loja.
//!
//! Veja `docs/fases.md` para o roadmap completo (MVP-0 a MVP-6).

mod dto;
mod handlers;
mod middleware;
mod state;
mod util;

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderValue, Method};
use axum::middleware::from_fn_with_state;
use axum::routing::{delete, get, post, put};
use axum::Router;
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::middleware::exigir_token;
use crate::state::{AppState, ConsoleEmailBackend};

// Re-exporta handlers no crate root para routing mais limpo.
use handlers::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ─── Logging / tracing ──────────────────────────────────────────────
    let filtro = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "mikella_server=debug,tower_http=info".to_string()
        } else {
            "mikella_server=info,tower_http=warn".to_string()
        }
    });
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filtro))
        .with_ansi(false)
        .init();
    tracing::info!("Mikella Store Server v{}", env!("CARGO_PKG_VERSION"));

    // ─── Resolve caminho do DB a partir de MIKELLA_DB_PATH (default: dados/loja.db)
    let db_path = std::env::var("MIKELLA_DB_PATH").unwrap_or_else(|_| "dados/loja.db".into());
    tracing::info!(db_path = %db_path, "abrindo banco de dados");

    // SEC-014/LGPD-003: a chave é resolvida antes do boot para que a migração
    // de blind index rode de forma consistente entre reinícios.
    let chave_loja = state::resolver_chave_loja(&db_path)?;
    let db = mikella_infra_sqlite::Database::abrir(&db_path)?;
    let migrados = mikella_infra_sqlite::backfill_blind_index(db.conn(), &chave_loja)?;
    if migrados > 0 {
        tracing::info!(migrados, "blind indexes pendentes convertidos no boot");
    }

    let email: Arc<dyn crate::state::EmailBackend> = Arc::new(ConsoleEmailBackend::new());
    let state = AppState::com_chave_loja(db, email, chave_loja);

    // ─── Rotas públicas (sem auth): health + login + onboarding + 2FA
    let rotas_publicas = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/login", post(login))
        .route("/v1/onboarding/status", get(onboarding_status))
        .route("/v1/onboarding", post(onboarding_create))
        .route("/v1/2fa/solicitar", post(solicitar_2fa))
        .route("/v1/2fa/verificar", post(verificar_2fa));

    // ─── Rotas protegidas: require token Bearer
    let rotas_protegidas = Router::new()
        .route("/v1/auth/me", get(auth_me))
        .route("/v1/auth/logout", post(logout))
        .route("/v1/produtos", get(listar_produtos))
        .route("/v1/produtos", post(criar_produto_handler))
        .route("/v1/produtos/:id", get(buscar_produto_handler))
        .route("/v1/produtos/:id", put(atualizar_produto_handler))
        .route("/v1/produtos/:id/arquivar", post(arquivar_produto_handler))
        .route("/v1/usuarios", get(listar_usuarios_handler))
        .route("/v1/usuarios", post(criar_usuario_handler))
        .route("/v1/usuarios/:id", get(buscar_usuario_handler))
        .route("/v1/usuarios/:id", put(atualizar_usuario_handler))
        .route(
            "/v1/usuarios/:id/desativar",
            post(desativar_usuario_handler),
        )
        .route(
            "/v1/usuarios/:id/capacidades",
            post(gerenciar_capacidades_handler),
        )
        .route("/v1/usuarios/:id/senha", put(atualizar_senha_handler))
        .route("/v1/caixa/status", get(caixa_status))
        .route("/v1/caixa/abrir", post(abrir_caixa_handler))
        .route("/v1/caixa/sangria", post(realizar_sangria_handler))
        .route("/v1/caixa/fechar", post(fechar_caixa_handler))
        .route(
            "/v1/caixa/fechamento/:id/comprovante",
            get(obter_comprovante_fechamento),
        )
        .route(
            "/v1/caixa/fechamento/:id/imprimir",
            post(imprimir_comprovante_fechamento),
        )
        .route("/v1/livro-caixa", get(livro_caixa))
        .route("/v1/vendas", get(vendas_listar))
        .route("/v1/vendas", post(finalizar_venda_handler))
        .route("/v1/vendas/simular-parcelas", get(simular_parcelas))
        .route(
            "/v1/vendas/autorizar-cancelamento",
            post(autorizar_cancelamento_handler),
        )
        .route("/v1/vendas/:id/cancelar", post(cancelar_venda_handler))
        .route("/v1/vendas/:id/estornar", post(cancelar_venda_handler))
        .route(
            "/v1/vendas/:id/estorno/comprovante",
            get(obter_comprovante_estorno),
        )
        .route(
            "/v1/vendas/:id/estorno/imprimir",
            post(imprimir_comprovante_estorno),
        )
        .route("/v1/vendas/:id/comprovante", get(obter_comprovante))
        .route("/v1/vendas/:id/imprimir", post(imprimir_comprovante))
        .route("/v1/clientes", get(listar_clientes_handler))
        .route("/v1/clientes", post(salvar_cliente_handler))
        .route("/v1/clientes/:id/exportar", get(exportar_titular_handler))
        .route("/v1/clientes/:id", delete(excluir_ou_anonimizar_cliente_handler))
        .route("/v1/crediario/devedores", get(listar_devedores_handler))
        .route("/v1/crediario/liquidar", post(liquidar_divida_handler))
        .route("/v1/sistema/backup-local", post(backup_local))
        .route("/v1/sistema/diagnostico", get(diagnostico))
        .route("/v1/sistema/teste-impressora", post(teste_impressora_handler))
        .route("/v1/sistema/cep/:cep", get(consultar_cep))
        .route("/v1/donation/prompt", get(donation_prompt))
        .route("/v1/donation/ack", post(donation_ack))
        // Finance (RF-030 / DSH-001) — contas a pagar/receber e Dashboard.
        .route("/v1/finance/resumo", get(obter_resumo_dashboard_handler))
        .route("/v1/finance/fluxo-caixa", get(obter_fluxo_caixa_handler))
        .route("/v1/finance/resultado", get(obter_resultado_dre_handler))
        .route("/v1/finance/contas-pagar", post(registrar_conta_pagar_handler))
        .route("/v1/finance/contas-receber", post(registrar_conta_receber_handler))
        .route(
            "/v1/finance/contas-pagar/:id/parcelas/:idx/liquidar",
            post(liquidar_parcela_handler),
        )
        .route(
            "/v1/finance/contas-receber/:id/parcelas/:idx/liquidar",
            post(liquidar_parcela_receber_handler),
        )
        .route("/v1/finance/contas-abertas", get(listar_contas_abertas_handler))
        // CSV import (PROD-010).
        .route("/v1/import/csv", post(import_csv_handler))
        // Aplica middleware de token em todas as rotas protegidas.
        .route_layer(from_fn_with_state(state.clone(), exigir_token));

    // ─── CORS: allowlist explícita (SEC-003).
    // `permissive()` permitia que qualquer site aberto no PC chamasse a API
    // loopback e lesse a resposta do login. Só o WebView do Tauri e o preview
    // de desenvolvimento falam com o servidor.
    let mut origens: Vec<HeaderValue> = [
        "http://tauri.localhost",
        "https://tauri.localhost",
        "tauri://localhost",
        "http://127.0.0.1:3000",
        "http://localhost:3000",
    ]
    .iter()
    .filter_map(|o| o.parse().ok())
    .collect();
    if let Ok(extra) = std::env::var("MIKELLA_CORS_ORIGIN") {
        if let Ok(valor) = extra.parse() {
            origens.push(valor);
        }
    }

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origens))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
        .allow_credentials(false);

    // ─── Combina e adiciona camadas: tracing, CORS, timeout e limite de corpo.
    let app = Router::new()
        .merge(rotas_publicas)
        .merge(rotas_protegidas)
        .with_state(state.clone())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors)
                .layer(TimeoutLayer::new(Duration::from_secs(30)))
                // 8 MB cobre a importação de CSV de produtos sem deixar a API
                // aberta a corpos ilimitados. Limites por rota ficam para o
                // SEC-016 (P1).
                .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
                .into_inner(),
        );

    // ─── Bind: loopback apenas (127.0.0.1)
    let porta: u16 = std::env::var("MIKELLA_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(47823);
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, porta));
    tracing::info!(addr = %addr, "servidor escutando em loopback");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
