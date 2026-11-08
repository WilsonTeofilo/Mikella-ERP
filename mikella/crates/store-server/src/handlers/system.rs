//! Handlers de operações de sistema: Cópia de segurança local (RES-030) e Diagnóstico (OPS-001).

use std::fs;
use std::path::PathBuf;
use axum::extract::State;
use axum::http::StatusCode;
use mikella_infra_hardware::PrinterPort;
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use mikella_infra_sqlite::DiagnosticoRepo;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BackupLocalRequest {
    pub caminho_destino: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BackupLocalResponse {
    pub status: String,
    pub arquivo: String,
    pub tamanho_bytes: u64,
    pub blake3_hash: String,
    pub timestamp: chrono::DateTime<Utc>,
}

/// `POST /v1/sistema/backup-local` — gera cópia local limpa e consistente via SQLite VACUUM INTO.
pub async fn backup_local(
    State(state): State<AppState>,
    ator: Ator,
    payload: Option<Json<BackupLocalRequest>>,
) -> impl IntoResponse {
    exigir!(ator, "sistema.backup");
    tracing::info!(usuario = %ator.0.username, "solicitando backup local");

    // SEC-005: o destino nunca é livre. O backup só pode ser gravado dentro de
    // uma pasta de backup conhecida, nunca sobrescreve/apaga arquivo existente.
    let raiz = std::env::var("MIKELLA_BACKUP_DIR").unwrap_or_else(|_| "backups".to_string());
    let raiz = PathBuf::from(raiz);
    if let Err(e) = fs::create_dir_all(&raiz) {
        tracing::error!(erro = %e, pasta = ?raiz, "falha ao criar pasta de backup");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": "falha ao preparar pasta de backup" })),
        );
    }
    let raiz_canon = match raiz.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(erro = %e, pasta = ?raiz, "pasta de backup inválida");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "pasta de backup inválida" })),
            );
        }
    };

    let destino: PathBuf = match payload.and_then(|p| p.caminho_destino.clone()) {
        Some(custom) if !custom.trim().is_empty() => {
            let pedido = PathBuf::from(custom.trim());
            let Some(nome) = pedido.file_name() else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "erro": "caminho de backup inválido" })),
                );
            };
            let pai = pedido.parent().unwrap_or(&raiz);
            let pai = if pai.as_os_str().is_empty() {
                raiz.clone()
            } else {
                pai.to_path_buf()
            };
            let pai_canon = match pai.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "erro": "pasta de destino não existe" })),
                    )
                }
            };
            if !pai_canon.starts_with(&raiz_canon) {
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({
                        "erro": "backup só pode ser gravado na pasta configurada (MIKELLA_BACKUP_DIR)"
                    })),
                );
            }
            pai_canon.join(nome)
        }
        _ => {
            let agora = Utc::now().format("%Y%m%d_%H%M%S");
            raiz_canon.join(format!("loja_backup_{}.db", agora))
        }
    };

    if destino.extension().and_then(|e| e.to_str()) != Some("db") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "erro": "backup deve ter extensão .db" })),
        );
    }

    // Nunca apaga um backup existente: recusa e pede outro nome.
    if destino.exists() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "erro": "já existe um arquivo nesse destino" })),
        );
    }

    let destino_str = destino.to_string_lossy().to_string();

    let db = state.db.lock().await;

    // VACUUM INTO parametrizado: sem interpolar caminho em SQL.
    if let Err(e) = DiagnosticoRepo::novo(db.conn()).backup_vacuum_into(&destino_str) {
        tracing::error!(erro = %e, destino = %destino_str, "falha no VACUUM INTO");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": "falha ao gerar cópia de segurança do banco" })),
        );
    }
    drop(db); // libera o lock do banco o quanto antes

    // Calcula tamanho e hash BLAKE3 do arquivo gerado para garantia de integridade
    let (tamanho, hash_hex) = match fs::read(&destino) {
        Ok(bytes) => {
            let hash = blake3::hash(&bytes);
            (bytes.len() as u64, hash.to_hex().to_string())
        }
        Err(e) => {
            tracing::error!(erro = %e, arquivo = %destino_str, "falha ao ler arquivo de backup gerado");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    serde_json::json!({ "erro": "backup gerado mas falhou verificação de integridade" }),
                ),
            );
        }
    };

    tracing::info!(
        arquivo = %destino_str,
        tamanho = tamanho,
        blake3 = %hash_hex,
        "backup local gerado com sucesso"
    );

    (
        StatusCode::OK,
        Json(serde_json::json!(BackupLocalResponse {
            status: "sucesso".into(),
            arquivo: destino_str,
            tamanho_bytes: tamanho,
            blake3_hash: hash_hex,
            timestamp: Utc::now(),
        })),
    )
}

/// `GET /v1/sistema/diagnostico` — pacote de saúde e diagnóstico sem PII (OPS-001).
pub async fn diagnostico(State(state): State<AppState>, ator: Ator) -> impl IntoResponse {
    exigir!(ator, "sistema.diagnostico");
    tracing::debug!(usuario = %ator.0.username, "gerando pacote de diagnóstico");

    let db = state.db.lock().await;
    let repo = DiagnosticoRepo::novo(db.conn());

    // 1. Integridade SQLite
    let integrity = match repo.integridade() {
        Ok(resultado) => resultado,
        Err(e) => {
            tracing::error!(erro = %e, "falha no integrity_check");
            "erro_ao_verificar".to_string()
        }
    };

    // 2. Versão SQLite
    let sqlite_ver = match repo.sqlite_versao() {
        Ok(versao) => versao,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao ler versão do SQLite");
            "desconhecido".to_string()
        }
    };

    // 3. Totais estruturais (métricas volumétricas sem PII)
    let contagens = match repo.contagens() {
        Ok(contagens) => contagens,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao contar tabelas do diagnóstico");
            Default::default()
        }
    };

    // 4. Verificação da cadeia de hash do Livro Caixa (FRD-053) e Auditoria (LGPD-002)
    let cadeia_livro_ok = match repo.cadeia_livro_caixa_integra() {
        Ok(valida) => valida,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao verificar cadeia do livro-caixa");
            false
        }
    };

    let cadeia_auditoria_ok = match repo.cadeia_auditoria_integra() {
        Ok(valida) => valida,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao verificar cadeia de auditoria");
            false
        }
    };

    drop(db);

    // 5. Informações de ambiente
    let os_nome = std::env::consts::OS;
    let arch_nome = std::env::consts::ARCH;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "app_versao": env!("CARGO_PKG_VERSION"),
            "ambiente": {
                "sistema_operacional": os_nome,
                "arquitetura": arch_nome,
            },
            "banco": {
                "sqlite_versao": sqlite_ver,
                "integrity_check": integrity,
                "cadeia_hash_livro_caixa_integra": cadeia_livro_ok,
                "cadeia_hash_auditoria_integra": cadeia_auditoria_ok,
                "estatisticas": {
                    "total_produtos": contagens.produtos,
                    "total_vendas": contagens.vendas,
                    "total_movimentacoes_estoque": contagens.movimentacoes_estoque,
                    "total_entradas_livro_caixa": contagens.entradas_livro_caixa,
                    "total_registros_auditoria": contagens.registros_auditoria,
                }
            },
            "timestamp": Utc::now()
        })),
    )
}

use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Clone, Serialize, Deserialize)]
pub struct CepInfo {
    pub cep: String,
    pub logradouro: String,
    pub bairro: String,
    pub cidade: String,
    pub uf: String,
}

static CEP_CACHE: std::sync::OnceLock<RwLock<HashMap<String, CepInfo>>> = std::sync::OnceLock::new();

fn obter_cep_cache() -> &'static RwLock<HashMap<String, CepInfo>> {
    CEP_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// `GET /v1/sistema/cep/:cep` — consulta de endereço por CEP (proxy local seguro com fallback e cache local).
///
/// Bypassa restrições de CSP do WebView desktop (SEG-063), usa ViaCEP como primário e BrasilAPI como reserva (CLI-004).
pub async fn consultar_cep(
    axum::extract::Path(cep): axum::extract::Path<String>,
) -> impl IntoResponse {
    let cep_limpo: String = cep.chars().filter(|c| c.is_ascii_digit()).collect();
    if cep_limpo.len() != 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "erro": "CEP deve conter exatamente 8 dígitos" })),
        );
    }

    // 1. Verifica cache local em memória (economiza requisições e funciona offline para repetidos)
    if let Ok(cache) = obter_cep_cache().read() {
        if let Some(item) = cache.get(&cep_limpo) {
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "cep": item.cep,
                    "logradouro": item.logradouro,
                    "bairro": item.bairro,
                    "cidade": item.cidade,
                    "uf": item.uf,
                    "cache": true
                })),
            );
        }
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3)) // timeout estrito de 3s (CLI-004)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao inicializar client HTTP");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha interna de rede" })),
            );
        }
    };

    // 2. Provedor Principal: ViaCEP
    let url_viacep = format!("https://viacep.com.br/ws/{}/json/", cep_limpo);
    if let Ok(res) = client.get(&url_viacep).send().await {
        if res.status().is_success() {
            if let Ok(body) = res.json::<serde_json::Value>().await {
                if !body.get("erro").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let logradouro = body.get("logradouro").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let bairro = body.get("bairro").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let localidade = body.get("localidade").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let uf = body.get("uf").and_then(|v| v.as_str()).unwrap_or("").to_ascii_uppercase();

                    let info = CepInfo {
                        cep: cep_limpo.clone(),
                        logradouro,
                        bairro,
                        cidade: localidade,
                        uf,
                    };

                    if let Ok(mut cache) = obter_cep_cache().write() {
                        cache.insert(cep_limpo, info.clone());
                    }

                    return (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "ok": true,
                            "cep": info.cep,
                            "logradouro": info.logradouro,
                            "bairro": info.bairro,
                            "cidade": info.cidade,
                            "uf": info.uf,
                            "provedor": "viacep"
                        })),
                    );
                }
            }
        }
    }

    // 3. Provedor Reserva: BrasilAPI (CLI-004)
    let url_brasilapi = format!("https://brasilapi.com.br/api/cep/v1/{}", cep_limpo);
    if let Ok(res) = client.get(&url_brasilapi).send().await {
        if res.status().is_success() {
            if let Ok(body) = res.json::<serde_json::Value>().await {
                let street = body.get("street").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let neighborhood = body.get("neighborhood").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let city = body.get("city").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let state = body.get("state").and_then(|v| v.as_str()).unwrap_or("").to_ascii_uppercase();

                let info = CepInfo {
                    cep: cep_limpo.clone(),
                    logradouro: street,
                    bairro: neighborhood,
                    cidade: city,
                    uf: state,
                };

                if let Ok(mut cache) = obter_cep_cache().write() {
                    cache.insert(cep_limpo, info.clone());
                }

                return (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "ok": true,
                        "cep": info.cep,
                        "logradouro": info.logradouro,
                        "bairro": info.bairro,
                        "cidade": info.cidade,
                        "uf": info.uf,
                        "provedor": "brasilapi"
                    })),
                );
            }
        }
    }

    // Se ambos falharam ou não encontraram
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "erro": "CEP não encontrado. Preencha o endereço manualmente." })),
    )
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use mikella_infra_sqlite::Database;
    use uuid::Uuid;

    #[test]
    fn backup_e_diagnostico_banco_temporario() {
        let unique = Uuid::new_v4().to_string();
        let temp_dir = std::env::temp_dir().join(format!("mikella_test_{}", unique));
        let _ = fs::create_dir_all(&temp_dir);

        let db_path = temp_dir.join("teste.db");
        let db = Database::abrir(&db_path).unwrap();

        // 1. Diagnóstico via read model (sem SQL no handler)
        let repo = DiagnosticoRepo::novo(db.conn());
        assert_eq!(repo.integridade().unwrap(), "ok");
        assert!(repo.cadeia_livro_caixa_integra().unwrap());

        // 2. Backup consistente via VACUUM INTO parametrizado
        let backup_path = temp_dir.join("backup_teste.db");
        let backup_str = backup_path.to_string_lossy().to_string();
        repo.backup_vacuum_into(&backup_str).unwrap();

        assert!(backup_path.exists());
        let bytes = fs::read(&backup_path).unwrap();
        assert!(!bytes.is_empty());
        let hash = blake3::hash(&bytes);
        assert!(!hash.to_hex().is_empty());

        // O backup deve ser um banco SQLite válido
        let db_backup = Database::abrir(&backup_path).unwrap();
        let repo_backup = DiagnosticoRepo::novo(db_backup.conn());
        assert_eq!(repo_backup.integridade().unwrap(), "ok");

        // Limpeza
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
