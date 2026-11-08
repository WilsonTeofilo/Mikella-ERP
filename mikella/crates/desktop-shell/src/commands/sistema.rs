/// Verificação de saúde usada pelo frontend/diagnóstico.
#[tauri::command]
pub fn health() -> Result<String, String> {
    Ok("ok".to_string())
}

/// Informações públicas da aplicação (sem caminhos, sem dados de cliente).
#[tauri::command]
pub fn app_info() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "status": "ok",
        "versao": env!("CARGO_PKG_VERSION"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_responde_ok() {
        assert_eq!(health().expect("health não deve falhar"), "ok");
    }

    #[test]
    fn app_info_expoe_apenas_status_e_versao() {
        let info = app_info().expect("app_info não deve falhar");
        assert_eq!(info["status"], "ok");
        assert_eq!(info["versao"], env!("CARGO_PKG_VERSION"));
        assert_eq!(info.as_object().expect("objeto").len(), 2);
    }
}
