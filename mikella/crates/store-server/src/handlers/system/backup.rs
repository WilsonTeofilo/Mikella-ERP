//! Cópia de segurança local (SEC-005 / RES-030) e verificação de backups.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::suporte;
use mikella_infra_sqlite::{blake3_arquivo, verificar_backup, DiagnosticoRepo, VerificacaoBackup};

type RespostaErro = (StatusCode, Json<serde_json::Value>);

fn erro(status: StatusCode, mensagem: &str) -> RespostaErro {
    (status, Json(serde_json::json!({ "erro": mensagem })))
}

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

/// Resolve a pasta autorizada de backups (SEC-005), criando-a se necessário.
fn raiz_backup_de_arquivos() -> Result<(PathBuf, PathBuf), RespostaErro> {
    let raiz = std::env::var("MIKELLA_BACKUP_DIR").unwrap_or_else(|_| "backups".to_string());
    let raiz = PathBuf::from(raiz);
    if let Err(e) = fs::create_dir_all(&raiz) {
        tracing::error!(erro = %e, pasta = ?raiz, "falha ao criar pasta de backup");
        return Err(erro(
            StatusCode::INTERNAL_SERVER_ERROR,
            "falha ao preparar pasta de backup",
        ));
    }
    let canon = match raiz.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(erro = %e, pasta = ?raiz, "pasta de backup inválida");
            return Err(erro(
                StatusCode::INTERNAL_SERVER_ERROR,
                "pasta de backup inválida",
            ));
        }
    };
    Ok((raiz, canon))
}

/// Extrai o nome do arquivo pedido, exigindo extensão `.db`.
fn nome_de_backup(pedido: &std::path::Path) -> Result<&std::ffi::OsStr, RespostaErro> {
    let nome = pedido
        .file_name()
        .filter(|n| !n.is_empty())
        .ok_or_else(|| erro(StatusCode::BAD_REQUEST, "informe o arquivo de backup"))?;
    if pedido.extension().and_then(|e| e.to_str()) != Some("db") {
        return Err(erro(
            StatusCode::BAD_REQUEST,
            "backup deve ter extensão .db",
        ));
    }
    Ok(nome)
}

/// Devolve o caminho canônico do pai, recusando pasta inexistente.
fn pai_canonico(pedido: &std::path::Path, raiz: &std::path::Path) -> Result<PathBuf, RespostaErro> {
    let pai = pedido
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(raiz);
    pai.canonicalize().map_err(|_| {
        erro(
            StatusCode::BAD_REQUEST,
            "a pasta do backup informada não existe",
        )
    })
}

/// Valida que `arquivo` está dentro da pasta de backup e tem extensão `.db`.
fn resolver_arquivo_backup(arquivo: &str) -> Result<PathBuf, RespostaErro> {
    let (raiz, raiz_canon) = raiz_backup_de_arquivos()?;
    let pedido = PathBuf::from(arquivo.trim());
    let nome = nome_de_backup(&pedido)?;
    let pai_canon = pai_canonico(&pedido, &raiz)?;

    if !pai_canon.starts_with(&raiz_canon) {
        return Err(erro(
            StatusCode::FORBIDDEN,
            "só é possível verificar backups dentro de MIKELLA_BACKUP_DIR",
        ));
    }

    let candidato = pai_canon.join(nome);
    // Um link simbólico dentro da pasta de backup poderia apontar para fora
    // dela. Quando o arquivo existe, o caminho real também precisa continuar
    // dentro da raiz autorizada.
    if !candidato.exists() {
        return Ok(candidato);
    }
    let canon = candidato.canonicalize().map_err(|_| {
        erro(
            StatusCode::BAD_REQUEST,
            "arquivo de backup inválido ou inacessível",
        )
    })?;
    if !canon.starts_with(&raiz_canon) {
        return Err(erro(
            StatusCode::FORBIDDEN,
            "só é possível verificar backups dentro de MIKELLA_BACKUP_DIR",
        ));
    }
    Ok(canon)
}

/// Corta um texto vindo do banco/arquivo em categoria legível.
///
/// O trecho depois do primeiro `:` costuma ser mensagem do SQLite, que pode
/// conter nome de tabela, caminho ou detalhe interno. Esse detalhe fica no log
/// (SEC-017); o cliente recebe só o rótulo estável e sem caractere de controle.
fn rotulo_do_problema(problema: &str) -> String {
    let categoria = problema.split(':').next().unwrap_or(problema);
    let limpo = suporte::sanitizar_logs(categoria, 1);
    let limpo: String = limpo
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect();
    let limpo = limpo.trim();
    if limpo.is_empty() {
        "falha na verificação".to_string()
    } else {
        limpo.to_string()
    }
}

/// Texto curto e sem controle, para campos que vêm de um arquivo externo.
fn texto_seguro(valor: &str, max: usize) -> String {
    valor
        .chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect()
}

/// Converte a verificação em resposta de cliente: sem caminho absoluto e sem
/// detalhe interno do SQLite. O objeto completo continua disponível no log.
fn resumo_verificacao(verificacao: &VerificacaoBackup) -> serde_json::Value {
    let arquivo = PathBuf::from(&verificacao.arquivo)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| texto_seguro(n, 200))
        .unwrap_or_else(|| "backup.db".to_string());

    let problemas: Vec<String> = verificacao
        .problemas
        .iter()
        .take(20)
        .map(|p| rotulo_do_problema(p))
        .collect();

    serde_json::json!({
        "arquivo": arquivo,
        "tamanho_bytes": verificacao.tamanho_bytes,
        "blake3_hash": verificacao.blake3_hash,
        "integridade": if verificacao.integridade == "ok" { "ok" } else { "falha" },
        "foreign_keys_violadas": verificacao.foreign_keys_violadas,
        "cadeia_livro_caixa_integra": verificacao.cadeia_livro_caixa_integra,
        "cadeia_auditoria_integra": verificacao.cadeia_auditoria_integra,
        "versao_schema": texto_seguro(&verificacao.versao_schema, 40),
        "contagens": verificacao.contagens,
        "valido": verificacao.valido,
        "problemas": problemas,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificarBackupRequest {
    pub arquivo: String,
}

/// `POST /v1/sistema/backup/verificar` — valida uma cópia sem alterá-la.
///
/// Devolve 200 com `valido: true/false`; 422 quando o arquivo existe mas não é
/// um backup aceitável (o operador vê o motivo e escolhe outro).
pub async fn verificar_backup_handler(
    ator: Ator,
    Json(payload): Json<VerificarBackupRequest>,
) -> impl IntoResponse {
    exigir!(ator, "sistema.backup");

    let arquivo = match resolver_arquivo_backup(&payload.arquivo) {
        Ok(p) => p,
        Err(resposta) => return resposta,
    };

    if !arquivo.is_file() {
        return erro(StatusCode::NOT_FOUND, "arquivo de backup não encontrado");
    }

    match verificar_backup(&arquivo) {
        Ok(verificacao) => {
            let status = if verificacao.valido {
                StatusCode::OK
            } else {
                // Detalhe técnico fica no log; o cliente recebe só o rótulo.
                tracing::warn!(
                    arquivo = %arquivo.display(),
                    problemas = ?verificacao.problemas,
                    "backup reprovado na verificação"
                );
                StatusCode::UNPROCESSABLE_ENTITY
            };
            (status, Json(resumo_verificacao(&verificacao)))
        }
        Err(e) => {
            // SEC-017: detalhe do arquivo/SQLite só no log.
            tracing::error!(erro = %e, arquivo = %arquivo.display(), "falha ao verificar backup");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "valido": false,
                    "erro": "não foi possível abrir o arquivo como backup do Mikella",
                    "problemas": ["arquivo ilegível ou corrompido"]
                })),
            )
        }
    }
}

/// `GET /v1/sistema/backups` — lista as cópias disponíveis na pasta autorizada.
pub async fn listar_backups_handler(ator: Ator) -> impl IntoResponse {
    exigir!(ator, "sistema.backup");

    let (_, raiz_canon) = match raiz_backup_de_arquivos() {
        Ok(p) => p,
        Err(resposta) => return resposta,
    };

    let mut backups = Vec::new();
    match fs::read_dir(&raiz_canon) {
        Ok(entradas) => {
            for entrada in entradas.flatten() {
                let caminho = entrada.path();
                if caminho.extension().and_then(|e| e.to_str()) != Some("db") {
                    continue;
                }
                let Ok(metadados) = entrada.metadata() else {
                    continue;
                };
                if !metadados.is_file() {
                    continue;
                }
                let modificado = metadados
                    .modified()
                    .ok()
                    .map(chrono::DateTime::<Utc>::from)
                    .map(|t| t.to_rfc3339());
                backups.push(serde_json::json!({
                    "arquivo": caminho.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                    "tamanho_bytes": metadados.len(),
                    "modificado_em": modificado,
                }));
            }
        }
        Err(e) => {
            tracing::error!(erro = %e, "falha ao listar pasta de backups");
            return erro(StatusCode::INTERNAL_SERVER_ERROR, "falha ao listar backups");
        }
    }

    backups.sort_by(|a, b| {
        let data = |item: &serde_json::Value| {
            item.get("modificado_em")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        data(b).cmp(&data(a))
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({ "backups": backups })),
    )
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
    let (raiz, raiz_canon) = match raiz_backup_de_arquivos() {
        Ok(p) => p,
        Err(resposta) => return resposta,
    };

    let destino: PathBuf = match payload.map(|Json(p)| p).and_then(|p| p.caminho_destino) {
        Some(custom) if !custom.trim().is_empty() => {
            let pedido = PathBuf::from(custom.trim());
            let Some(nome) = pedido.file_name() else {
                return erro(StatusCode::BAD_REQUEST, "caminho de backup inválido");
            };
            let Ok(pai_canon) = pai_canonico(&pedido, &raiz) else {
                return erro(StatusCode::BAD_REQUEST, "pasta de destino não existe");
            };
            if !pai_canon.starts_with(&raiz_canon) {
                return erro(
                    StatusCode::FORBIDDEN,
                    "backup só pode ser gravado na pasta configurada (MIKELLA_BACKUP_DIR)",
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
        return erro(StatusCode::BAD_REQUEST, "backup deve ter extensão .db");
    }

    // Nunca apaga um backup existente: recusa e pede outro nome.
    if destino.exists() {
        return erro(StatusCode::CONFLICT, "já existe um arquivo nesse destino");
    }

    let destino_str = destino.to_string_lossy().to_string();

    let db = state.db.lock().await;

    // VACUUM INTO parametrizado: sem interpolar caminho em SQL.
    if let Err(e) = DiagnosticoRepo::novo(db.conn()).backup_vacuum_into(&destino_str) {
        tracing::error!(erro = %e, destino = %destino_str, "falha no VACUUM INTO");
        return erro(
            StatusCode::INTERNAL_SERVER_ERROR,
            "falha ao gerar cópia de segurança do banco",
        );
    }
    drop(db); // libera o lock do banco o quanto antes

    // Tamanho e hash BLAKE3 do arquivo gerado: o hash é lido em blocos para não
    // carregar o banco inteiro na memória (a máquina-piso tem 6 GB).
    let tamanho = match fs::metadata(&destino) {
        Ok(metadados) => metadados.len(),
        Err(e) => {
            tracing::error!(erro = %e, arquivo = %destino_str, "falha ao medir arquivo de backup gerado");
            return erro(
                StatusCode::INTERNAL_SERVER_ERROR,
                "backup gerado mas falhou verificação de integridade",
            );
        }
    };
    let hash_hex = match blake3_arquivo(&destino) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!(erro = %e, arquivo = %destino_str, "falha ao ler arquivo de backup gerado");
            return erro(
                StatusCode::INTERNAL_SERVER_ERROR,
                "backup gerado mas falhou verificação de integridade",
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
            // SEC-017: o cliente recebe só o nome; o caminho completo fica no log.
            arquivo: destino
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| texto_seguro(n, 200))
                .unwrap_or_else(|| "backup.db".to_string()),
            tamanho_bytes: tamanho,
            blake3_hash: hash_hex,
            timestamp: Utc::now(),
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resumo_de_verificacao_nao_expoe_caminho_nem_detalhe_interno() {
        let verificacao = VerificacaoBackup {
            arquivo: r"C:\Users\loja\dados\backup.db".to_string(),
            integridade: "*** in database main *** page 3: btree".to_string(),
            versao_schema: "0012\nmalicioso".to_string(),
            problemas: vec![
                r"integrity_check falhou: unable to open C:\Users\loja\dados\backup.db".to_string(),
                "tabela obrigatória ausente: vendas".to_string(),
                "/home/loja/backup.db corrompido".to_string(),
            ],
            ..Default::default()
        };

        let resumo = resumo_verificacao(&verificacao);

        // SEC-017: nome do arquivo, sem o caminho absoluto.
        assert_eq!(resumo["arquivo"], "backup.db");
        assert_eq!(resumo["integridade"], "falha");
        assert_eq!(resumo["versao_schema"], "0012malicioso");

        let texto = resumo.to_string();
        assert!(
            !texto.contains("C:\\Users"),
            "caminho absoluto vazou: {texto}"
        );
        assert!(!texto.contains("/home/"), "caminho POSIX vazou: {texto}");
        assert!(
            !texto.contains("page 3"),
            "detalhe do SQLite vazou: {texto}"
        );
        assert!(texto.contains("integrity_check falhou"));
        assert!(texto.contains("tabela obrigatória ausente"));
        assert!(texto.contains("<caminho-local>"));
    }
}
