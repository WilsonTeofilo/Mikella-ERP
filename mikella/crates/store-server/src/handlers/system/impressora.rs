//! Cupom de teste da impressora térmica (ESC/POS).

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use mikella_infra_hardware::PrinterPort;

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use mikella_infra_sqlite::DiagnosticoRepo;

/// `POST /v1/sistema/teste-impressora`
/// Emite um cupom de teste para validação de comunicação, acentuação e corte de papel.
pub async fn teste_impressora_handler(
    State(state): State<AppState>,
    ator: Ator,
) -> impl IntoResponse {
    exigir!(ator, "sistema.configurar");
    let fuso = {
        let db = state.db.lock().await;
        DiagnosticoRepo::novo(db.conn()).fuso_horario()
    };
    let agora = mikella_infra_hardware::formatar_data_no_fuso(
        &chrono::Utc::now(),
        &fuso,
        "%d/%m/%Y %H:%M:%S",
    );
    let mut buf = Vec::new();
    buf.extend_from_slice(b"\x1b@"); // Inicializa
    buf.extend_from_slice(b"\x1ba\x01"); // Centraliza
    buf.extend_from_slice(b"MIKELLA ERP\nTESTE DE IMPRESSAO\n");
    buf.extend_from_slice(format!("Data/Hora: {}\n", agora).as_bytes());
    buf.extend_from_slice(b"--------------------------------\n");
    buf.extend_from_slice(b"\x1ba\x00"); // Alinha à esquerda
    buf.extend_from_slice(b"Status da comunicacao: OK\n");
    buf.extend_from_slice(b"Acentuacao: A E I O U c C\n");
    buf.extend_from_slice(b"Largura: 58mm / 80mm ESC/POS\n");
    buf.extend_from_slice(b"--------------------------------\n");
    buf.extend_from_slice(b"\x1ba\x01");
    buf.extend_from_slice(b"Comprovante emitido com sucesso!\n\n\n\n");
    buf.extend_from_slice(b"\x1dm"); // Corte de papel

    let impressora = mikella_infra_hardware::DispositivoLocalPrinter::novo("padrao");
    match impressora.imprimir(&buf) {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "sucesso": true,
                "mensagem": "Comprovante de teste enviado para a impressora local."
            })),
        ),
        Err(e) => {
            // SEC-017: o detalhe do hardware fica no log, nunca na resposta.
            tracing::error!(erro = %e, "falha ao imprimir cupom de teste");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "sucesso": false,
                    "erro": "Não foi possível falar com a impressora. Verifique o cabo e a configuração."
                })),
            )
        }
    }
}
