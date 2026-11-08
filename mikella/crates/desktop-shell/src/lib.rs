//! mikella-desktop-shell
//!
//! Fronteira de apresentação/IPC do aplicativo desktop (Tauri 2).
//!
//! Regras de negócio pertencem aos crates `domain-*` e `app-*`; infraestrutura
//! fica em `infra-*` e o servidor HTTP local em `store-server`. Este crate
//! apenas monta a janela e expõe comandos IPC finos e de leitura.

pub mod app;
pub mod commands;

/// Compõe e executa o aplicativo desktop.
pub fn executar() -> tauri::Result<()> {
    app::setup::iniciar_logging();
    app::setup::construir().run(tauri::generate_context!())
}
