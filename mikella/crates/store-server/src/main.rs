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

use axum::middleware::from_fn_with_state;
use axum::routing::{get, post, put};
use axum::Router;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

use crate::middleware::exigir_token;
use crate::state::{AppState, ConsoleEmailBackend};

// Re-exporta handlers no crate root para routing mais limpo.
use handlers::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ─── Logging / tracing ──────────────────────────────────────────────
    let filtro = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "mikella_server=debug,tower_http=info".to_string());
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filtro))
        .with_ansi(false)
        .init();
    tracing::info!("Mikella Store Server v{}", env!("CARGO_PKG_VERSION"));

    // ─── Resolve caminho do DB a partir de MIKELLA_DB_PATH (default: dados/loja.db)
    let db_path = std::env::var("MIKELLA_DB_PATH").unwrap_or_else(|_| "dados/loja.db".into());
    tracing::info!(db_path = %db_path, "abrindo banco de dados");

    let db = mikella_infra_sqlite::Database::abrir(&db_path)?;
    let email: Arc<dyn crate::state::EmailBackend> = Arc::new(ConsoleEmailBackend::new());
    let state = AppState::new(db, email);

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
        .route(
            "/v1/vendas/autorizar-cancelamento",
            post(autorizar_cancelamento_handler),
        )
        .route("/v1/vendas/:id/cancelar", post(cancelar_venda_handler))
        .route("/v1/vendas/:id/comprovante", get(obter_comprovante))
        .route("/v1/vendas/:id/imprimir", post(imprimir_comprovante))
        .route("/v1/clientes", get(listar_clientes_handler))
        .route("/v1/clientes", post(salvar_cliente_handler))
        .route("/v1/crediario/devedores", get(listar_devedores_handler))
        .route("/v1/crediario/liquidar", post(liquidar_divida_handler))
        .route("/v1/sistema/backup-local", post(backup_local))
        .route("/v1/sistema/diagnostico", get(diagnostico))
        .route("/v1/donation/prompt", get(donation_prompt))
        .route("/v1/donation/ack", post(donation_ack))
        // Finance (RF-030) — contas a pagar/receber.
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

    // ─── Camada CORS para permitir chamadas do Tauri / WebView2 (tauri://localhost)
    let cors = tower_http::cors::CorsLayer::permissive();

    // ─── Combina e adiciona camada de tracing e cors
    let app = Router::new()
        .merge(rotas_publicas)
        .merge(rotas_protegidas)
        .with_state(state.clone())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors)
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
