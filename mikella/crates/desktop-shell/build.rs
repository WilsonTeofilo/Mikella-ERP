use std::path::Path;

fn main() {
    tauri_build::build();

    // O `frontendDist` é embutido no binário em tempo de compilação. Sem esta
    // declaração, o Cargo não percebe mudanças em HTML/CSS/JS e o executável
    // continua servindo a versão antiga do frontend.
    let frontend = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("frontend")
        .join("src");
    println!("cargo:rerun-if-changed={}", frontend.display());
}
