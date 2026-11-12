//! Handlers de operações de sistema: Cópia de segurança local (RES-030) e Diagnóstico (OPS-001).

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use mikella_infra_hardware::PrinterPort;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::suporte::{self, EntradaZip};
use mikella_infra_sqlite::{
    blake3_arquivo, expurgar_clientes_por_retencao, verificar_backup, DiagnosticoRepo,
};

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
fn raiz_backup_de_arquivos() -> Result<(PathBuf, PathBuf), (StatusCode, Json<serde_json::Value>)> {
    let raiz = std::env::var("MIKELLA_BACKUP_DIR").unwrap_or_else(|_| "backups".to_string());
    let raiz = PathBuf::from(raiz);
    if let Err(e) = fs::create_dir_all(&raiz) {
        tracing::error!(erro = %e, pasta = ?raiz, "falha ao criar pasta de backup");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": "falha ao preparar pasta de backup" })),
        ));
    }
    let canon = match raiz.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(erro = %e, pasta = ?raiz, "pasta de backup inválida");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "pasta de backup inválida" })),
            ));
        }
    };
    Ok((raiz, canon))
}

/// Valida que `arquivo` está dentro da pasta de backup e tem extensão `.db`.
fn resolver_arquivo_backup(
    arquivo: &str,
) -> Result<PathBuf, (StatusCode, Json<serde_json::Value>)> {
    let (raiz, raiz_canon) = raiz_backup_de_arquivos()?;
    let pedido = PathBuf::from(arquivo.trim());
    let Some(nome) = pedido.file_name() else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "erro": "caminho de backup inválido" })),
        ));
    };
    if nome.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "erro": "informe o arquivo de backup" })),
        ));
    }
    if pedido.extension().and_then(|e| e.to_str()) != Some("db") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "erro": "backup deve ter extensão .db" })),
        ));
    }
    let pai = pedido
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(&raiz);
    let pai_canon = match pai.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "erro": "pasta do backup não existe" })),
            ))
        }
    };
    if !pai_canon.starts_with(&raiz_canon) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "erro": "só é possível verificar backups dentro de MIKELLA_BACKUP_DIR"
            })),
        ));
    }
    let candidato = pai_canon.join(nome);
    // Um link simbólico dentro da pasta de backup poderia apontar para fora
    // dela. Quando o arquivo existe, o caminho real também precisa continuar
    // dentro da raiz autorizada.
    if candidato.exists() {
        let canon = match candidato.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "erro": "arquivo de backup inválido" })),
                ))
            }
        };
        if !canon.starts_with(&raiz_canon) {
            return Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "erro": "só é possível verificar backups dentro de MIKELLA_BACKUP_DIR"
                })),
            ));
        }
        return Ok(canon);
    }
    Ok(candidato)
}

/// Corta um texto vindo do banco/arquivo em categoria legível.
///
/// O trecho depois do primeiro `:` costuma ser mensagem do SQLite, que pode
/// conter nome de tabela, caminho ou detalhe interno. Esse detalhe fica no log
/// (SEC-017); o cliente recebe só o rótulo estável e sem caractere de controle.
fn rotulo_do_problema(problema: &str) -> String {
    // 1. Corta o detalhe depois do primeiro `:` (SQLite, caminho, stack).
    // 2. Passa pelo mesmo higienizador do log, que redige caminho POSIX,
    //    documento, e-mail e token que tenham sobrado no rótulo.
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
fn resumo_verificacao(verificacao: &mikella_infra_sqlite::VerificacaoBackup) -> serde_json::Value {
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
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "erro": "arquivo de backup não encontrado" })),
        );
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha ao listar backups" })),
            );
        }
    }

    backups.sort_by(|a, b| {
        let ma = a
            .get("modificado_em")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let mb = b
            .get("modificado_em")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        mb.cmp(ma)
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({ "backups": backups })),
    )
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExpurgoLgpdRequest {
    /// Retenção em anos. O mínimo aceito é 5 (guarda fiscal).
    pub anos: Option<u32>,
    /// `false` (padrão) apenas simula; `true` anonimiza de verdade.
    pub aplicar: bool,
}

/// `POST /v1/sistema/lgpd/expurgo` — simula ou aplica a anonimização de
/// clientes sem movimentação dentro do prazo de retenção (LGPD Art. 16).
///
/// Nunca apaga registros fiscais (`vendas`, `livro_caixa`, `auditoria`): apenas
/// os dados pessoais em `pessoas` são substituídos, preservando o `id` para as
/// chaves estrangeiras. A aplicação e a auditoria entram na mesma transação.
pub async fn expurgo_lgpd_handler(
    State(state): State<AppState>,
    ator: Ator,
    payload: Option<Json<ExpurgoLgpdRequest>>,
) -> impl IntoResponse {
    exigir!(ator, "sistema.configurar");

    let pedido = payload.map(|Json(p)| p).unwrap_or_default();
    let anos = pedido.anos.unwrap_or(5);
    if anos < 5 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "erro": "a retenção mínima é de 5 anos; prazos menores exigem revisão jurídica"
            })),
        );
    }

    let mut db = state.db.lock().await;
    let fuso = DiagnosticoRepo::novo(db.conn()).fuso_horario();

    let resultado = if pedido.aplicar {
        let tx = match db.transacao() {
            Ok(tx) => tx,
            Err(e) => {
                // SEC-017: detalhe do SQLite só no log.
                tracing::error!(erro = %e, "falha ao abrir transação do expurgo LGPD");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha ao iniciar o expurgo" })),
                );
            }
        };

        let relatorio = match expurgar_clientes_por_retencao(&tx, &fuso, anos, true) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(erro = %e, "falha ao aplicar expurgo LGPD");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha ao aplicar o expurgo" })),
                );
            }
        };

        let detalhe = format!(
            "anonimizados={}; prazo_anos={anos}; corte={}",
            relatorio.clientes_anonimizados, relatorio.data_corte
        );
        let auditoria = mikella_infra_sqlite::AuditoriaRepo::novo(&tx);
        if let Err(e) = auditoria.registrar(
            "lgpd",
            "pessoas",
            "EXPURGO_LGPD",
            Some(&ator.id().to_string()),
            None,
            None,
            Some(&detalhe),
        ) {
            tracing::error!(erro = %e, "falha ao auditar expurgo LGPD");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha ao registrar auditoria do expurgo" })),
            );
        }

        if let Err(e) = tx.commit() {
            tracing::error!(erro = %e, "falha ao confirmar expurgo LGPD");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha ao confirmar o expurgo" })),
            );
        }

        relatorio
    } else {
        match expurgar_clientes_por_retencao(db.conn(), &fuso, anos, false) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(erro = %e, "falha ao simular expurgo LGPD");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha ao simular o expurgo" })),
                );
            }
        }
    };

    (StatusCode::OK, Json(serde_json::json!(resultado)))
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

    // Tamanho e hash BLAKE3 do arquivo gerado: o hash é lido em blocos para não
    // carregar o banco inteiro na memória (a máquina-piso tem 6 GB).
    let tamanho = match fs::metadata(&destino) {
        Ok(metadados) => metadados.len(),
        Err(e) => {
            tracing::error!(erro = %e, arquivo = %destino_str, "falha ao medir arquivo de backup gerado");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    serde_json::json!({ "erro": "backup gerado mas falhou verificação de integridade" }),
                ),
            );
        }
    };
    let hash_hex = match blake3_arquivo(&destino) {
        Ok(hash) => hash,
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

    // 5. Schema aplicado: prova em qual versão o banco estava quando o
    //    problema aconteceu. Só nomes de arquivo e checksums, sem dados.
    let migracoes = match repo.migracoes() {
        Ok(migracoes) => migracoes,
        Err(e) => {
            tracing::error!(erro = %e, "falha ao listar migrações do diagnóstico");
            Vec::new()
        }
    };

    let fuso_horario = repo.fuso_horario();

    serde_json::json!({
        "status": "ok",
        "app_versao": env!("CARGO_PKG_VERSION"),
        "ambiente": {
            "sistema_operacional": std::env::consts::OS,
            "arquitetura": std::env::consts::ARCH,
            "perfil": if cfg!(debug_assertions) { "debug" } else { "release" },
            "fuso_horario_loja": fuso_horario,
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

/// Resolve a pasta autorizada de pacotes de suporte, criando-a se necessário.
///
/// O caminho vem de `MIKELLA_SUPPORT_DIR` (default `suporte`). Assim como o
/// backup, o pacote nunca sobrescreve um arquivo existente.
fn raiz_suporte() -> Result<PathBuf, (StatusCode, Json<serde_json::Value>)> {
    let raiz = std::env::var("MIKELLA_SUPPORT_DIR").unwrap_or_else(|_| "suporte".to_string());
    let raiz = PathBuf::from(raiz);
    if let Err(e) = fs::create_dir_all(&raiz) {
        tracing::error!(erro = %e, pasta = ?raiz, "falha ao criar pasta de suporte");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "erro": "falha ao preparar pasta de suporte" })),
        ));
    }
    match raiz.canonicalize() {
        Ok(p) => Ok(p),
        Err(e) => {
            tracing::error!(erro = %e, pasta = ?raiz, "pasta de suporte inválida");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "pasta de suporte inválida" })),
            ))
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
        match serde_json::to_vec_pretty(valor) {
            Ok(conteudo) => entradas.push(EntradaZip {
                nome: nome.to_string(),
                conteudo,
            }),
            Err(e) => {
                tracing::error!(erro = %e, arquivo = nome, "falha ao serializar o pacote de suporte");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha ao montar o pacote de suporte" })),
                );
            }
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
    match serde_json::to_vec_pretty(&manifesto) {
        Ok(conteudo) => entradas.push(EntradaZip {
            nome: "manifesto.json".to_string(),
            conteudo,
        }),
        Err(e) => {
            tracing::error!(erro = %e, "falha ao serializar o manifesto do pacote de suporte");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "erro": "falha ao montar o pacote de suporte" })),
            );
        }
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

    // 4. Nome único: nunca sobrescreve pacote anterior. `create_new` fecha a
    //    corrida entre "existe?" e "grava" e recusa sobrescrever por construção.
    use std::io::Write;

    let carimbo = Utc::now().format("%Y%m%d_%H%M%S");
    let mut contador = 1u32;
    let destino = loop {
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
                if let Err(e) = arquivo.write_all(&zip) {
                    tracing::error!(erro = %e, arquivo = ?candidato, "falha ao gravar pacote de suporte");
                    drop(arquivo);
                    let _ = fs::remove_file(&candidato);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({ "erro": "falha ao gravar o pacote de suporte" })),
                    );
                }
                break candidato;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                contador += 1;
                if contador > 100 {
                    tracing::error!(pasta = ?raiz, "pasta de suporte cheia de nomes para o mesmo segundo");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(
                            serde_json::json!({ "erro": "falha ao escolher nome do pacote de suporte" }),
                        ),
                    );
                }
            }
            Err(e) => {
                tracing::error!(erro = %e, arquivo = ?candidato, "falha ao criar pacote de suporte");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "erro": "falha ao gravar o pacote de suporte" })),
                );
            }
        }
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

static CEP_CACHE: std::sync::OnceLock<RwLock<HashMap<String, CepInfo>>> =
    std::sync::OnceLock::new();

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
                    let logradouro = body
                        .get("logradouro")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let bairro = body
                        .get("bairro")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let localidade = body
                        .get("localidade")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let uf = body
                        .get("uf")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_ascii_uppercase();

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
                let street = body
                    .get("street")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let neighborhood = body
                    .get("neighborhood")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let city = body
                    .get("city")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let state = body
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_uppercase();

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

    #[test]
    fn resumo_de_verificacao_nao_expoe_caminho_nem_detalhe_interno() {
        let mut verificacao = mikella_infra_sqlite::VerificacaoBackup::default();
        verificacao.arquivo = r"C:\Users\loja\dados\backup.db".to_string();
        verificacao.integridade = "*** in database main *** page 3: btree".to_string();
        verificacao.versao_schema = "0012\nmalicioso".to_string();
        verificacao.problemas = vec![
            r"integrity_check falhou: unable to open C:\Users\loja\dados\backup.db".to_string(),
            "tabela obrigatória ausente: vendas".to_string(),
            "/home/loja/backup.db corrompido".to_string(),
        ];

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
