//! Handlers de operações de sistema: Cópia de segurança local (RES-030) e Diagnóstico (OPS-001).

use std::fs;
use std::path::PathBuf;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;

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

    let destino: PathBuf = match payload.and_then(|p| p.caminho_destino.clone()) {
        Some(custom) if !custom.trim().is_empty() => PathBuf::from(custom.trim()),
        _ => {
            let agora = Utc::now().format("%Y%m%d_%H%M%S");
            PathBuf::from("backups").join(format!("loja_backup_{}.db", agora))
        }
    };

    // Cria diretório pai caso não exista
    if let Some(parent) = destino.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = fs::create_dir_all(parent) {
                tracing::error!(erro = %e, pasta = ?parent, "falha ao criar pasta para backup");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha ao criar diretório de destino do backup" })),
                );
            }
        }
    }

    // Se o arquivo de destino já existir, remove antes do VACUUM INTO
    if destino.exists() {
        let _ = fs::remove_file(&destino);
    }

    let destino_str = destino.to_string_lossy().to_string();

    let db = state.db.lock().await;

    // Executa VACUUM INTO com escape de aspas
    let sql = format!("VACUUM INTO '{}'", destino_str.replace('\'', "''"));
    if let Err(e) = db.conn().execute(&sql, []) {
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
                Json(serde_json::json!({ "erro": "backup gerado mas falhou verificação de integridade" })),
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

    // 1. Integridade SQLite
    let integrity: String = db
        .conn()
        .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
        .unwrap_or_else(|_| "erro_ao_verificar".into());

    // 2. Versão SQLite
    let sqlite_ver: String = db
        .conn()
        .query_row("SELECT sqlite_version();", [], |r| r.get(0))
        .unwrap_or_else(|_| "desconhecido".into());

    // 3. Totais estruturais (métricas volumétricas sem PII)
    let total_produtos: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM produtos;", [], |r| r.get(0))
        .unwrap_or(0);

    let total_vendas: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM vendas;", [], |r| r.get(0))
        .unwrap_or(0);

    let total_mov_estoque: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM movimentacoes_estoque;", [], |r| r.get(0))
        .unwrap_or(0);

    let total_entradas_caixa: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM livro_caixa;", [], |r| r.get(0))
        .unwrap_or(0);

    // 4. Verificação da cadeia de hash do Livro Caixa (FRD-053)
    let cadeia_livro_ok = match db.conn().prepare(
        "SELECT prev_hash, hash FROM livro_caixa ORDER BY created_at ASC, rowid ASC;"
    ) {
        Ok(mut stmt) => {
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            });

            match rows {
                Ok(entries) => {
                    let mut prev = "GENESIS".to_string();
                    let mut valida = true;
                    for res in entries {
                        match res {
                            Ok((p, h)) => {
                                if p != prev {
                                    valida = false;
                                    break;
                                }
                                prev = h;
                            }
                            Err(_) => {
                                valida = false;
                                break;
                            }
                        }
                    }
                    valida
                }
                Err(_) => false,
            }
        }
        Err(_) => false,
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
                "estatisticas": {
                    "total_produtos": total_produtos,
                    "total_vendas": total_vendas,
                    "total_movimentacoes_estoque": total_mov_estoque,
                    "total_entradas_livro_caixa": total_entradas_caixa,
                }
            },
            "timestamp": Utc::now()
        })),
    )
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

        // 1. Testa diagnóstico diretamente na conexão
        let conn = db.conn();
        let integrity: String = conn
            .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");

        // 2. Testa VACUUM INTO para gerar backup
        let backup_path = temp_dir.join("backup_teste.db");
        let sql = format!("VACUUM INTO '{}'", backup_path.to_string_lossy().replace('\'', "''"));
        conn.execute(&sql, []).unwrap();

        assert!(backup_path.exists());
        let bytes = fs::read(&backup_path).unwrap();
        assert!(!bytes.is_empty());
        let hash = blake3::hash(&bytes);
        assert!(!hash.to_hex().is_empty());

        // O backup deve ser um banco SQLite válido
        let db_backup = Database::abrir(&backup_path).unwrap();
        let int_backup: String = db_backup
            .conn()
            .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(int_backup, "ok");

        // Limpeza
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
