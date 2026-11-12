//! Verificação, restauração e retenção de cópias de segurança (RES-030 / F8).
//!
//! Regra de ouro: **backup que não é restaurado não é backup**. Este módulo
//! abre a cópia em modo somente-leitura, roda o `PRAGMA integrity_check`,
//! confere as chaves estrangeiras, valida as cadeias de hash do livro-caixa e
//! da auditoria, lê a versão do schema e calcula o BLAKE3 do arquivo.
//!
//! A restauração é feita **offline**, com o servidor parado: o processo atual
//! mantém a conexão SQLite aberta e trocar o arquivo por baixo dele corromperia
//! o estado. O boot do `store-server` expõe `--verificar-backup` e
//! `--restaurar-backup` justamente para esse fluxo de manutenção.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::error::{InfraError, InfraResult};
use crate::repos::sistema::{ContagensDiagnostico, DiagnosticoRepo};

/// Resultado completo da verificação de uma cópia de segurança.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerificacaoBackup {
    pub arquivo: String,
    pub tamanho_bytes: u64,
    pub blake3_hash: String,
    pub integridade: String,
    pub foreign_keys_violadas: i64,
    pub cadeia_livro_caixa_integra: bool,
    pub cadeia_auditoria_integra: bool,
    pub versao_schema: String,
    pub contagens: ContagensDiagnostico,
    /// `true` somente quando nenhum problema foi encontrado.
    pub valido: bool,
    /// Lista legível dos problemas encontrados (sem caminho absoluto de stack).
    pub problemas: Vec<String>,
}

fn contar_violacoes_fk(conn: &Connection) -> InfraResult<i64> {
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let mut rows = stmt.query([])?;
    let mut total = 0i64;
    while rows.next()?.is_some() {
        total += 1;
    }
    Ok(total)
}

fn tabela_existe(conn: &Connection, nome: &str) -> InfraResult<bool> {
    let existe: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [nome],
        |row| row.get(0),
    )?;
    Ok(existe > 0)
}

/// BLAKE3 de um arquivo, lido em blocos.
///
/// Um backup pode ter centenas de megabytes; `fs::read` carregaria tudo na
/// memória de uma máquina de loja (6 GB no piso de hardware). O hash é o mesmo
/// do arquivo inteiro, só que sem o pico de memória.
pub fn blake3_arquivo(caminho: &Path) -> InfraResult<String> {
    use std::io::Read;

    let mut arquivo = fs::File::open(caminho)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let lido = arquivo.read(&mut buffer)?;
        if lido == 0 {
            break;
        }
        hasher.update(&buffer[..lido]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// Verifica uma cópia de segurança sem alterá-la.
///
/// Devolve [`InfraError`] apenas quando o arquivo não pode ser lido/aberto.
/// Um banco aberto mas inconsistente volta com `valido = false` e a lista de
/// `problemas`, para que a UI explique o motivo em vez de devolver um 500 seco.
pub fn verificar_backup(caminho: &Path) -> InfraResult<VerificacaoBackup> {
    let metadados = fs::metadata(caminho).map_err(|e| {
        InfraError::Io(format!(
            "não foi possível ler o backup {}: {e}",
            caminho.display()
        ))
    })?;
    if !metadados.is_file() {
        return Err(InfraError::Integridade(
            "o caminho de backup não é um arquivo".into(),
        ));
    }

    // O hash cobre o arquivo inteiro como ele está no disco: é a impressão
    // digital que o operador pode comparar com o backup offsite.
    let blake3_hash = blake3_arquivo(caminho)?;

    let conn = Connection::open_with_flags(
        caminho,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        InfraError::Integridade(format!(
            "arquivo de backup não é um banco SQLite legível: {e}"
        ))
    })?;

    let mut problemas = Vec::new();

    let integridade =
        match conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0)) {
            Ok(resultado) => resultado,
            Err(e) => {
                problemas.push(format!("integrity_check falhou: {e}"));
                "erro".to_string()
            }
        };
    if integridade != "ok" {
        problemas.push(format!("integrity_check retornou: {integridade}"));
    }

    let foreign_keys_violadas = match contar_violacoes_fk(&conn) {
        Ok(0) => 0,
        Ok(n) => {
            problemas.push(format!("{n} violação(ões) de chave estrangeira"));
            n
        }
        Err(e) => {
            problemas.push(format!("foreign_key_check falhou: {e}"));
            -1
        }
    };

    let repo = DiagnosticoRepo::novo(&conn);

    let (versao_schema, contagens) = if tabela_existe(&conn, "schema_migrations")? {
        let versao = match conn.query_row(
            "SELECT versao FROM schema_migrations ORDER BY versao DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        ) {
            Ok(v) => v,
            Err(e) => {
                problemas.push(format!("schema_migrations ilegível: {e}"));
                "desconhecida".to_string()
            }
        };
        let contagens = match repo.contagens() {
            Ok(c) => c,
            Err(e) => {
                problemas.push(format!("contagens estruturais ilegíveis: {e}"));
                ContagensDiagnostico::default()
            }
        };
        (versao, contagens)
    } else {
        problemas.push("tabela schema_migrations ausente".into());
        ("ausente".to_string(), ContagensDiagnostico::default())
    };

    for tabela in ["livro_caixa", "auditoria", "vendas"] {
        if !tabela_existe(&conn, tabela)? {
            problemas.push(format!("tabela obrigatória ausente: {tabela}"));
        }
    }

    let cadeia_livro_caixa_integra = match repo.cadeia_livro_caixa_integra() {
        Ok(true) => true,
        Ok(false) => {
            problemas.push("cadeia de hash do livro-caixa quebrada".into());
            false
        }
        Err(e) => {
            problemas.push(format!("cadeia do livro-caixa ilegível: {e}"));
            false
        }
    };

    let cadeia_auditoria_integra = match repo.cadeia_auditoria_integra() {
        Ok(true) => true,
        Ok(false) => {
            problemas.push("cadeia de hash da auditoria quebrada".into());
            false
        }
        Err(e) => {
            problemas.push(format!("cadeia da auditoria ilegível: {e}"));
            false
        }
    };

    drop(conn);

    let valido = problemas.is_empty()
        && integridade == "ok"
        && foreign_keys_violadas == 0
        && cadeia_livro_caixa_integra
        && cadeia_auditoria_integra;

    Ok(VerificacaoBackup {
        arquivo: caminho.display().to_string(),
        tamanho_bytes: metadados.len(),
        blake3_hash,
        integridade,
        foreign_keys_violadas,
        cadeia_livro_caixa_integra,
        cadeia_auditoria_integra,
        versao_schema,
        contagens,
        valido,
        problemas,
    })
}

/// Detalhes de uma restauração concluída.
#[derive(Debug, Clone)]
pub struct RestauracaoBackup {
    pub origem: String,
    pub destino: String,
    pub copia_seguranca: Option<String>,
    pub verificacao: VerificacaoBackup,
}

fn caminho_com_sufixo(base: &Path, sufixo: &str) -> PathBuf {
    PathBuf::from(format!("{}{sufixo}", base.display()))
}

/// Restaura `origem` sobre `destino` com rede de segurança.
///
/// Etapas (todas falham de forma observável, sem apagar o banco atual):
/// 1. verifica a origem e recusa backup inválido;
/// 2. copia o banco atual para `*.pre-restauracao-<timestamp>.db`;
/// 3. copia a origem para um temporário e verifica o temporário;
/// 4. remove `-wal`, `-shm` e `-journal` antigos do destino;
/// 5. troca o arquivo e verifica o destino final.
///
/// **Chame apenas com o servidor parado.** O arquivo de destino é o mesmo que
/// o processo abriria no próximo boot.
pub fn restaurar_backup(origem: &Path, destino: &Path) -> InfraResult<RestauracaoBackup> {
    let verificacao = verificar_backup(origem)?;
    if !verificacao.valido {
        return Err(InfraError::Integridade(format!(
            "backup reprovado na verificação: {}",
            verificacao.problemas.join("; ")
        )));
    }

    let origem_canon = origem.canonicalize()?;
    let destino_abs = if destino.is_absolute() {
        destino.to_path_buf()
    } else {
        std::env::current_dir()?.join(destino)
    };
    let pai = destino_abs
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| InfraError::Io("destino de restauração sem pasta".into()))?;
    fs::create_dir_all(pai)?;
    let pai_canon = pai.canonicalize()?;
    let nome_destino = destino_abs
        .file_name()
        .ok_or_else(|| InfraError::Io("destino de restauração sem nome de arquivo".into()))?;
    let destino_final = pai_canon.join(nome_destino);

    if origem_canon == destino_final {
        return Err(InfraError::Integridade(
            "origem e destino da restauração são o mesmo arquivo".into(),
        ));
    }

    // 2. Cópia de segurança do banco atual, antes de qualquer alteração.
    let copia_seguranca = if destino_final.exists() {
        let carimbo = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let nome = destino_final
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("loja.db");
        let copia = pai_canon.join(format!("{nome}.pre-restauracao-{carimbo}.db"));
        fs::copy(&destino_final, &copia)?;
        Some(copia)
    } else {
        None
    };

    // 3. Prepara o temporário e valida antes de tocar no destino.
    let temporario = pai_canon.join(format!(
        ".{}.restaurando",
        destino_final
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("loja.db")
    ));
    if temporario.exists() {
        fs::remove_file(&temporario)?;
    }
    fs::copy(&origem_canon, &temporario)?;

    let verificacao_temporario = match verificar_backup(&temporario) {
        Ok(v) => v,
        Err(e) => {
            let _ = fs::remove_file(&temporario);
            return Err(e);
        }
    };
    if !verificacao_temporario.valido {
        let _ = fs::remove_file(&temporario);
        return Err(InfraError::Integridade(format!(
            "cópia temporária reprovada: {}",
            verificacao_temporario.problemas.join("; ")
        )));
    }

    // 4. WAL/SHM antigos pertencem ao banco anterior: nunca podem sobreviver.
    for sufixo in ["-wal", "-shm", "-journal"] {
        let lateral = caminho_com_sufixo(&destino_final, sufixo);
        if lateral.exists() {
            fs::remove_file(&lateral)?;
        }
    }

    // 5. Troca. Em Windows `rename` não sobrescreve; removemos primeiro e,
    //    se algo falhar, restauramos a cópia de segurança.
    if destino_final.exists() {
        fs::remove_file(&destino_final)?;
    }
    if let Err(e) = fs::rename(&temporario, &destino_final) {
        if let Some(copia) = &copia_seguranca {
            let _ = fs::copy(copia, &destino_final);
        }
        let _ = fs::remove_file(&temporario);
        return Err(e.into());
    }

    let verificacao_final = verificar_backup(&destino_final)?;
    if !verificacao_final.valido {
        return Err(InfraError::Integridade(format!(
            "banco restaurado não passou na verificação final: {}",
            verificacao_final.problemas.join("; ")
        )));
    }

    Ok(RestauracaoBackup {
        origem: origem_canon.display().to_string(),
        destino: destino_final.display().to_string(),
        copia_seguranca: copia_seguranca.map(|p| p.display().to_string()),
        verificacao: verificacao_final,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    fn banco_temporario() -> (PathBuf, Database) {
        let dir = std::env::temp_dir().join(format!("mikella-backup-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("loja.db");
        let db = Database::abrir(&caminho).unwrap();
        (dir, db)
    }

    #[test]
    fn backup_recem_gerado_e_aprovado() {
        let (dir, db) = banco_temporario();
        let destino = dir.join("copia.db");
        DiagnosticoRepo::novo(db.conn())
            .backup_vacuum_into(&destino.to_string_lossy())
            .unwrap();

        let verificacao = verificar_backup(&destino).unwrap();
        assert!(verificacao.valido, "{:?}", verificacao.problemas);
        assert_eq!(verificacao.integridade, "ok");
        assert_eq!(verificacao.foreign_keys_violadas, 0);
        assert!(verificacao.cadeia_livro_caixa_integra);
        assert!(verificacao.cadeia_auditoria_integra);
        assert!(!verificacao.blake3_hash.is_empty());
        assert!(verificacao.tamanho_bytes > 0);

        drop(db);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn arquivo_que_nao_e_sqlite_nao_abre() {
        let dir = std::env::temp_dir().join(format!("mikella-backup-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let falso = dir.join("falso.db");
        fs::write(&falso, b"isto nao e um banco sqlite").unwrap();

        // Abre, mas o integrity_check falha -> inválido, sem panic.
        let resultado = verificar_backup(&falso);
        match resultado {
            Ok(v) => assert!(!v.valido),
            Err(_) => { /* recusa explícita também é aceitável */ }
        }

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn hash_em_blocos_igual_ao_hash_do_arquivo_inteiro() {
        let dir = std::env::temp_dir().join(format!("mikella-hash-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("grande.bin");
        let conteudo: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        fs::write(&caminho, &conteudo).unwrap();

        assert_eq!(
            blake3_arquivo(&caminho).unwrap(),
            blake3::hash(&conteudo).to_hex().to_string()
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn restauracao_preserva_o_banco_anterior_em_copia() {
        let (dir, db) = banco_temporario();
        db.conn()
            .execute(
                "INSERT INTO usuarios (id, nome, username, senha_hash, papel)
                 VALUES ('u1', 'Original', 'original', 'hash', 'DONO')",
                [],
            )
            .unwrap();

        let backup = dir.join("copia.db");
        DiagnosticoRepo::novo(db.conn())
            .backup_vacuum_into(&backup.to_string_lossy())
            .unwrap();

        // Altera o banco atual depois do backup.
        db.conn()
            .execute(
                "INSERT INTO usuarios (id, nome, username, senha_hash, papel)
                 VALUES ('u2', 'Depois', 'depois', 'hash', 'DONO')",
                [],
            )
            .unwrap();
        drop(db);

        let destino = dir.join("loja.db");
        let restauracao = restaurar_backup(&backup, &destino).unwrap();
        assert!(restauracao.verificacao.valido);
        assert!(restauracao.copia_seguranca.is_some());
        assert!(Path::new(restauracao.copia_seguranca.as_ref().unwrap()).exists());

        let restaurado = Database::abrir(&destino).unwrap();
        let total: i64 = restaurado
            .conn()
            .query_row("SELECT COUNT(*) FROM usuarios", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 1, "o backup restaurado tem só o usuário original");

        drop(restaurado);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn restauracao_recusa_backup_corrompido() {
        let (dir, _db) = banco_temporario();
        let falso = dir.join("corrompido.db");
        fs::write(&falso, b"lixo").unwrap();

        let destino = dir.join("loja.db");
        assert!(restaurar_backup(&falso, &destino).is_err());

        fs::remove_dir_all(&dir).ok();
    }
}
