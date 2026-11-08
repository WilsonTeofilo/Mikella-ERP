use tauri::Builder;

use crate::commands;

/// Configura o tracing do processo. SEM PII: apenas identificação da aplicação.
pub fn iniciar_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new("mikella=debug"))
        .with_ansi(false)
        .init();

    tracing::info!(
        "Iniciando Mikella ERP Desktop Shell (Tauri) v{}",
        env!("CARGO_PKG_VERSION")
    );
}

/// Monta o aplicativo com o conjunto mínimo de comandos expostos ao WebView.
pub fn construir() -> Builder<tauri::Wry> {
    Builder::default().invoke_handler(tauri::generate_handler![
        commands::sistema::health,
        commands::sistema::app_info,
    ])
}
