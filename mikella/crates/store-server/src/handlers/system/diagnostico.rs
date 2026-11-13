//! Diagnóstico sem PII (OPS-001) e pacote de suporte (ZIP *stored*).

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::suporte::{self, EntradaZip};
use mikella_infra_sqlite::DiagnosticoRepo;

/// Monta o diagnóstico de saúde sem PII.
///
/// Compartilhado pelo endpoint `/v1/sistema/diagnostico` e pelo pacote de
/// suporte (OPS-001), para que os dois nunca divirjam. Não retorna SQL, caminho
/// absoluto, token, CPF ou qualquer dado de cliente.
fn montar_diagnostico(db: &mikella_infra_sqlite::Database) -> serde_json::Value {
    let repo = DiagnosticoRepo::novo(db.conn());

    // 1. Integridade SQLite. O resultado bruto pode listar páginas, tabelas e
    //    índices internos: fica no log; o cliente recebe só o veredito.
    let integrity = match repo.integridade() {
        Ok(resultado) if resultado == "ok" => "ok".to_string(),
        Ok(resultado) => {
            tracing::error!(resultado = %resultado, "integrity_check reprovado");
            "falha".to_string()
        }
        Err(e) => {
            tracing::error!(erro = %e, "falha no integrity_check");
            "erro_ao_verificar".to_string()
        }
    };

    // 2. Versão SQLite
    let sqlite_ver = repo.sqlite_versao().unwrap_or_else(|e| {
        tracing::error!(erro = %e, "falha ao ler versão do SQLite");
        "desconhecido".to_string()
    });

    // 3. Totais estruturais (métricas volumétricas sem PII)
    let contagens = repo.contagens().unwrap_or_else(|e| {
        tracing::error!(erro = %e, "falha ao contar tabelas do diagnóstico");
        Default::default()
    });

    // 4. Verificação da cadeia de hash do Livro Caixa (FRD-053) e Auditoria (LGPD-002)
    let cadeia_livro_ok = repo.cadeia_livro_caixa_integra().unwrap_or_else(|e| {
        tracing::error!(erro = %e, "falha ao verificar cadeia do livro-caixa");
        false
    });

    let cadeia_auditoria_ok = repo.cadeia_auditoria_integra().unwrap_or_else(|e| {
        tracing::error!(erro = %e, "falha ao verificar cadeia de auditoria");
        false
    });

    // 5. Schema aplicado: prova em qual versão o banco estava quando o
    //    problema aconteceu. Só nomes de arquivo e checksums, sem dados.
    let migracoes = repo.migracoes().unwrap_or_else(|e| {
        tracing::error!(erro = %e, "falha ao listar migrações do diagnóstico");
        Vec::new()
    });

    serde_json::json!({
        "status": "ok",
        "app_versao": env!("CARGO_PKG_VERSION"),
        "ambiente": {
            "sistema_operacional": std::env::consts::OS,
            "arquitetura": std::env::consts::ARCH,
            "perfil": if cfg!(debug_assertions) { "debug" } else { "release" },
            "fuso_horario_loja": repo.fuso_horario(),
        },
        "banco": {
            "sqlite_versao": sqlite_ver,
            "integrity_check": integrity,
            "cadeia_hash_livro_caixa_integra": cadeia_livro_ok,
            "cadeia_hash_auditoria_integra": cadeia_auditoria_ok,
            "migracoes": migracoes,
            "estatisticas": {
                "total_produtos": contagens.produtos,
                "total_vendas": contagens.vendas,
                "total_movimentacoes_estoque": contagens.movimentacoes_estoque,
                "total_entradas_livro_caixa": contagens.entradas_livro_caixa,
                "total_registros_auditoria": contagens.registros_auditoria,
            }
        },
        "timestamp": Utc::now()
    })
}

/// `GET /v1/sistema/diagnostico` — pacote de saúde e diagnóstico sem PII (OPS-001).
pub async fn diagnostico(State(state): State<AppState>, ator: Ator) -> impl IntoResponse {
    exigir!(ator, "sistema.diagnostico");
    tracing::debug!(usuario = %ator.0.username, "gerando pacote de diagnóstico");

    let db = state.db.lock().await;
    let diagnostico = montar_diagnostico(&db);
    drop(db);

    (StatusCode::OK, Json(diagnostico))
}

#[derive(Debug, Serialize)]
pub struct PacoteSuporteResponse {
    pub status: String,
    pub arquivo: String,
    pub tamanho_bytes: u64,
    pub blake3_hash: String,
    pub conteudo: Vec<String>,
    pub timestamp: chrono::DateTime<Utc>,
}

type RespostaErro = (StatusCode, Json<serde_json::Value>);

/// Resolve a pasta autorizada de pacotes de suporte, criando-a se necessário.
///
/// O caminho vem de `MIKELLA_SUPPORT_DIR` (default `suporte`). Assim como o
/// backup, o pacote nunca sobrescreve um arquivo existente.
fn raiz_suporte() -> Result<PathBuf, RespostaErro> {
    let raiz = std::env::var("MIKELLA_SUPPORT_DIR").unwrap_or_else(|_| "suporte".to_string());
    let raiz = PathBuf::from(raiz);
    if let Err(e) = fs::create_dir_all(&raiz) {
        tracing::error!(erro = %e, pasta = ?raiz, "falha ao criar pasta de suporte");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": "falha ao preparar pasta de suporte" })),
        ));
    }
    raiz.canonicalize().map_err(|e| {
        tracing::error!(erro = %e, pasta = ?raiz, "pasta de suporte inválida");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": "pasta de suporte inválida" })),
        )
    })
}

fn erro_interno(mensagem: &str) -> RespostaErro {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "erro": mensagem })),
    )
}

/// Serializa um JSON em bytes, devolvendo a resposta de erro pronta em caso de falha.
fn json_para_zip(valor: &serde_json::Value, nome: &str) -> Result<Vec<u8>, RespostaErro> {
    serde_json::to_vec_pretty(valor).map_err(|e| {
        tracing::error!(erro = %e, arquivo = nome, "falha ao serializar o pacote de suporte");
        erro_interno("falha ao montar o pacote de suporte")
    })
}

/// Grava o ZIP com nome único, sem sobrescrever pacote anterior.
///
/// `create_new` fecha a corrida entre "existe?" e "grava" e recusa sobrescrever
/// por construção.
fn gravar_pacote(raiz: &std::path::Path, zip: &[u8]) -> Result<PathBuf, RespostaErro> {
    use std::io::Write;

    let carimbo = Utc::now().format("%Y%m%d_%H%M%S");
    let mut contador = 1u32;
    loop {
        let nome = if contador == 1 {
            format!("suporte_{carimbo}.zip")
        } else {
            format!("suporte_{carimbo}_{contador}.zip")
        };
        let candidato = raiz.join(nome);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidato)
        {
            Ok(mut arquivo) => {
                if let Err(e) = arquivo.write_all(zip) {
                    tracing::error!(erro = %e, arquivo = ?candidato, "falha ao gravar pacote de suporte");
                    drop(arquivo);
                    let _ = fs::remove_file(&candidato);
                    return Err(erro_interno("falha ao gravar o pacote de suporte"));
                }
                return Ok(candidato);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                contador += 1;
                if contador > 100 {
                    tracing::error!(pasta = ?raiz, "pasta de suporte cheia de nomes para o mesmo segundo");
                    return Err(erro_interno("falha ao escolher nome do pacote de suporte"));
                }
            }
            Err(e) => {
                tracing::error!(erro = %e, arquivo = ?candidato, "falha ao criar pacote de suporte");
                return Err(erro_interno("falha ao gravar o pacote de suporte"));
            }
        }
    }
}

/// `POST /v1/sistema/suporte/pacote` — gera um `.zip` com diagnóstico,
/// ambiente, migrações, manifesto e as últimas linhas de log higienizadas.
///
/// O pacote **não** contém banco, backup, chave da loja, token ou PII. Ainda
/// assim, deve ser revisado por uma pessoa antes de sair da loja (OPS-001).
pub async fn pacote_suporte_handler(
    State(state): State<AppState>,
    ator: Ator,
) -> impl IntoResponse {
    exigir!(ator, "sistema.diagnostico");
    tracing::info!(usuario = %ator.0.username, "gerando pacote de suporte");

    let raiz = match raiz_suporte() {
        Ok(raiz) => raiz,
        Err(resposta) => return resposta,
    };

    // 1. Diagnóstico compartilhado com o endpoint JSON.
    let diagnostico = {
        let db = state.db.lock().await;
        montar_diagnostico(&db)
    };

    // 2. Ambiente de execução, sem caminho absoluto nem usuário do SO.
    let ambiente = serde_json::json!({
        "app_versao": env!("CARGO_PKG_VERSION"),
        "sistema_operacional": std::env::consts::OS,
        "arquitetura": std::env::consts::ARCH,
        "familia": std::env::consts::FAMILY,
        "perfil": if cfg!(debug_assertions) { "debug" } else { "release" },
        "gerado_em": Utc::now(),
        "fuso_horario_processo": std::env::var("TZ").unwrap_or_else(|_| "definido pelo SO".to_string()),
    });

    let mut entradas = Vec::new();
    for (nome, valor) in [
        ("diagnostico.json", &diagnostico),
        ("ambiente.json", &ambiente),
    ] {
        match json_para_zip(valor, nome) {
            Ok(conteudo) => entradas.push(EntradaZip {
                nome: nome.to_string(),
                conteudo,
            }),
            Err(resposta) => return resposta,
        }
    }

    entradas.push(EntradaZip {
        nome: "logs.txt".to_string(),
        conteudo: suporte::ler_logs_sanitizados().into_bytes(),
    });

    // 3. Manifesto: permite conferir integridade sem abrir cada arquivo.
    let manifesto = serde_json::json!({
        "formato": "mikella.suporte.v1",
        "gerado_em": Utc::now(),
        "app_versao": env!("CARGO_PKG_VERSION"),
        "aviso": "Pacote sem banco, backup, chave ou PII conhecida. Revise antes de compartilhar.",
        "arquivos": entradas.iter().map(|e| serde_json::json!({
            "nome": e.nome,
            "tamanho_bytes": e.conteudo.len(),
            "blake3": suporte::blake3_hex(&e.conteudo),
        })).collect::<Vec<_>>(),
    });
    match json_para_zip(&manifesto, "manifesto.json") {
        Ok(conteudo) => entradas.push(EntradaZip {
            nome: "manifesto.json".to_string(),
            conteudo,
        }),
        Err(resposta) => return resposta,
    }

    entradas.push(EntradaZip {
        nome: "README.txt".to_string(),
        conteudo: concat!(
            "Pacote de suporte Mikella ERP\n",
            "============================\n\n",
            "Conteudo:\n",
            "  diagnostico.json  Saude do banco, contagens e migracoes (sem PII).\n",
            "  ambiente.json     Versao do app, SO, arquitetura e perfil de build.\n",
            "  logs.txt          Ultimas linhas do log, com dados sensiveis redigidos.\n",
            "  manifesto.json    Tamanho e BLAKE3 de cada arquivo.\n\n",
            "NAO contem: banco de dados, backup, chave da loja, token de sessao,\n",
            "CPF/CNPJ, e-mail ou qualquer dado de cliente.\n\n",
            "Antes de enviar ao suporte, revise logs.txt. A higienizacao e\n",
            "best-effort: um log inesperado pode conter dado que ela nao previu.\n\n",
            "Gerado por um usuario autenticado com a permissao sistema.diagnostico.\n",
            "O evento fica registrado na auditoria com o nome do arquivo.\n",
        )
        .as_bytes()
        .to_vec(),
    });

    let zip = suporte::criar_zip(&entradas);
    let hash_hex = suporte::blake3_hex(&zip);

    // 4. Nome único: nunca sobrescreve pacote anterior.
    let destino = match gravar_pacote(&raiz, &zip) {
        Ok(destino) => destino,
        Err(resposta) => return resposta,
    };

    let tamanho = zip.len() as u64;
    let nome_arquivo = destino
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("suporte.zip")
        .to_string();
    let destino_str = destino.display().to_string();

    tracing::info!(
        arquivo = %destino_str,
        tamanho,
        blake3 = %hash_hex,
        "pacote de suporte gerado"
    );

    // 5. Auditoria best-effort: o pacote já está no disco, então uma falha de
    //    auditoria é registrada no log, não esconde o arquivo gerado.
    {
        let db = state.db.lock().await;
        let auditoria = mikella_infra_sqlite::AuditoriaRepo::novo(db.conn());
        let detalhe = format!("arquivo={nome_arquivo}; tamanho={tamanho}; blake3={hash_hex}");
        if let Err(e) = auditoria.registrar(
            "sistema",
            &nome_arquivo,
            "PACOTE_SUPORTE",
            Some(&ator.id().to_string()),
            None,
            None,
            Some(&detalhe),
        ) {
            tracing::error!(erro = %e, arquivo = %nome_arquivo, "falha ao auditar pacote de suporte");
        }
    }

    let conteudo = entradas.into_iter().map(|e| e.nome).collect();
    (
        StatusCode::OK,
        Json(serde_json::json!(PacoteSuporteResponse {
            status: "sucesso".into(),
            // SEC-017: só o nome do arquivo; o caminho completo fica no log.
            arquivo: nome_arquivo,
            tamanho_bytes: tamanho,
            blake3_hash: hash_hex,
            conteudo,
            timestamp: Utc::now(),
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
