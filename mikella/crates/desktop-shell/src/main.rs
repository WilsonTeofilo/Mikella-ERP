use std::sync::Arc;

/// Estado compartilhado entre os comandos Tauri.
#[allow(dead_code)]
struct AppState {
    /// Placeholder — não usado neste esqueleto mínimo.
    _placeholder: Arc<()>,
}

/// Endpoint de saúde para a UI verificar que o backend Tauri está ativo.
#[tauri::command]
fn health() -> Result<String, String> {
    Ok("ok".to_string())
}

/// Status da aplicação — versão e estado do banco.
#[tauri::command]
fn app_info() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "status": "ok",
        "versao": env!("CARGO_PKG_VERSION"),
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ─── Logging / tracing ──────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new("mikella=debug"))
        .with_ansi(false)
        .init();

    tracing::info!(
        "Iniciando Mikella ERP Desktop Shell (Tauri) v{}",
        env!("CARGO_PKG_VERSION")
    );

    // ─── Estado — apenas um placeholder ────────────────────────────────────
    let state = Arc::new(AppState { _placeholder: Arc::new(()) });

    // ─── Tauri ───────────────────────────────────────────────────────────
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            health,
            app_info,
        ])
        .run(tauri::generate_context!())?;

    Ok(())
}

// Re-export para uso em testes
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_ok() {
        assert_eq!(health().unwrap(), "ok");
    }
}
