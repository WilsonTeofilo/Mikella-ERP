//! Entrada do binário do shell desktop.
//!
//! Toda a composição da aplicação está em `mikella_desktop_shell::executar`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    mikella_desktop_shell::executar()?;
    Ok(())
}
