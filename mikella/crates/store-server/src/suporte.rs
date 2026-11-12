//! Pacote de suporte (OPS-001): ZIP com diagnóstico, ambiente, migrações e
//! logs higienizados.
//!
//! O pacote **não** inclui banco, backup, chave da loja, senha, token, CPF ou
//! qualquer dado de cliente. O ZIP usa o método *stored* (sem compressão) para
//! não adicionar dependência: o conteúdo é texto pequeno e o formato ZIP é
//! escrito aqui de forma determinística, com CRC32 e assinaturas padrão.
//!
//! O log é opcional (`MIKELLA_LOG_FILE`). Quando existe, só as últimas linhas
//! entram no pacote e passam por uma higienização best-effort antes da escrita.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Arquivo que entra no ZIP.
pub struct EntradaZip {
    pub nome: String,
    pub conteudo: Vec<u8>,
}

/// CRC32 IEEE 802.3, usado pelo formato ZIP.
pub fn crc32(dados: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in dados {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            // `mascara` é 0xFFFF_FFFF quando o bit baixo é 1, senão 0.
            let mascara = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mascara);
        }
    }
    !crc
}

/// Hash BLAKE3 em hexadecimal, igual ao usado no backup.
pub fn blake3_hex(dados: &[u8]) -> String {
    blake3::hash(dados).to_hex().to_string()
}

/// Data/hora no formato MS-DOS exigido pelos headers do ZIP.
fn dos_datetime() -> (u16, u16) {
    use chrono::{Datelike, Timelike};

    let agora = chrono::Local::now();
    let hora = ((agora.hour() as u16) << 11)
        | ((agora.minute() as u16) << 5)
        | (agora.second() as u16 / 2);
    let ano = agora.year().clamp(1980, 2107) as u16;
    let data = ((ano - 1980) << 9) | ((agora.month() as u16) << 5) | (agora.day() as u16);
    (hora, data)
}

/// Gera um ZIP com método *stored* (sem compressão).
///
/// Suficiente para o pacote de suporte (poucos arquivos de texto) e não exige
/// a dependência `zip`. Nomes são gravados em UTF-8 com a flag 0x0800.
pub fn criar_zip(entradas: &[EntradaZip]) -> Vec<u8> {
    let (hora, data) = dos_datetime();
    let capacidade = 1024 + entradas.iter().map(|e| e.conteudo.len()).sum::<usize>();
    let mut saida = Vec::with_capacity(capacidade);
    let mut central = Vec::new();

    for entrada in entradas {
        let offset = u32::try_from(saida.len()).unwrap_or(u32::MAX);
        let tamanho = u32::try_from(entrada.conteudo.len()).unwrap_or(u32::MAX);
        let crc = crc32(&entrada.conteudo);
        let nome = entrada.nome.as_bytes();
        let nome_len = u16::try_from(nome.len()).unwrap_or(u16::MAX);

        // ── Local file header ────────────────────────────────────────────
        saida.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        saida.extend_from_slice(&20u16.to_le_bytes()); // versão necessária
        saida.extend_from_slice(&0x0800u16.to_le_bytes()); // nomes em UTF-8
        saida.extend_from_slice(&0u16.to_le_bytes()); // método: stored
        saida.extend_from_slice(&hora.to_le_bytes());
        saida.extend_from_slice(&data.to_le_bytes());
        saida.extend_from_slice(&crc.to_le_bytes());
        saida.extend_from_slice(&tamanho.to_le_bytes());
        saida.extend_from_slice(&tamanho.to_le_bytes());
        saida.extend_from_slice(&nome_len.to_le_bytes());
        saida.extend_from_slice(&0u16.to_le_bytes()); // extra
        saida.extend_from_slice(nome);
        saida.extend_from_slice(&entrada.conteudo);

        // ── Central directory header ─────────────────────────────────────
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // versão que criou
        central.extend_from_slice(&20u16.to_le_bytes()); // versão necessária
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // stored
        central.extend_from_slice(&hora.to_le_bytes());
        central.extend_from_slice(&data.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&tamanho.to_le_bytes());
        central.extend_from_slice(&tamanho.to_le_bytes());
        central.extend_from_slice(&nome_len.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comentário
        central.extend_from_slice(&0u16.to_le_bytes()); // disco
        central.extend_from_slice(&0u16.to_le_bytes()); // atributos internos
        central.extend_from_slice(&0u32.to_le_bytes()); // atributos externos
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(nome);
    }

    let cd_offset = u32::try_from(saida.len()).unwrap_or(u32::MAX);
    let cd_tamanho = u32::try_from(central.len()).unwrap_or(u32::MAX);
    let total = u16::try_from(entradas.len()).unwrap_or(u16::MAX);
    saida.extend_from_slice(&central);

    // ── End of central directory ─────────────────────────────────────────
    saida.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    saida.extend_from_slice(&0u16.to_le_bytes()); // disco
    saida.extend_from_slice(&0u16.to_le_bytes()); // disco do diretório
    saida.extend_from_slice(&total.to_le_bytes());
    saida.extend_from_slice(&total.to_le_bytes());
    saida.extend_from_slice(&cd_tamanho.to_le_bytes());
    saida.extend_from_slice(&cd_offset.to_le_bytes());
    saida.extend_from_slice(&0u16.to_le_bytes()); // comentário

    saida
}

/// Lê no máximo `limite` bytes do **fim** do arquivo.
///
/// Um arquivo de log pode crescer indefinidamente; ler o arquivo inteiro só
/// para descartar o começo consumiria memória proporcional ao log (e poderia
/// derrubar o servidor). O segundo item do retorno indica se o corte caiu no
/// meio de uma linha, para o chamador descartar o fragmento.
fn ler_final(caminho: &Path, limite: u64) -> std::io::Result<(Vec<u8>, bool)> {
    let mut arquivo = fs::File::open(caminho)?;
    let tamanho = arquivo.metadata()?.len();
    let inicio = tamanho.saturating_sub(limite);

    let mut meio_de_linha = false;
    if inicio > 0 {
        // Um byte antes do corte diz se estamos no começo de uma linha.
        let mut anterior = [0u8; 1];
        arquivo.seek(SeekFrom::Start(inicio - 1))?;
        arquivo.read_exact(&mut anterior)?;
        meio_de_linha = anterior[0] != b'\n';
        arquivo.seek(SeekFrom::Start(inicio))?;
    }

    let mut bytes = Vec::with_capacity(limite.min(tamanho) as usize);
    arquivo.take(limite).read_to_end(&mut bytes)?;
    Ok((bytes, meio_de_linha))
}

/// Lê o log configurado em `MIKELLA_LOG_FILE`, limita o tamanho e higieniza.
pub fn ler_logs_sanitizados() -> String {
    const LIMITE_BYTES: u64 = 512 * 1024;
    const MAX_LINHAS: usize = 2000;

    let Some(caminho) = std::env::var_os("MIKELLA_LOG_FILE") else {
        return "Nenhum arquivo de log configurado.\n\
                O servidor grava logs em stdout. Para incluir as últimas linhas no\n\
                pacote, inicie o servidor com MIKELLA_LOG_FILE apontando para um arquivo.\n"
            .to_string();
    };

    let caminho = PathBuf::from(caminho);
    let (bytes, meio_de_linha) = match ler_final(&caminho, LIMITE_BYTES) {
        Ok(resultado) => resultado,
        Err(e) => {
            tracing::warn!(erro = %e, "MIKELLA_LOG_FILE não pôde ser lido para o pacote de suporte");
            return "Arquivo de log configurado não pôde ser lido.\n".to_string();
        }
    };

    let mut texto = String::from_utf8_lossy(&bytes).into_owned();
    if meio_de_linha {
        // A primeira linha começou no meio de outra (ou no meio de um caractere
        // UTF-8): descartar até o primeiro `\n` evita publicar fragmento.
        if let Some(pos) = texto.find('\n') {
            texto.replace_range(..=pos, "");
        }
    }
    sanitizar_logs(&texto, MAX_LINHAS)
}

/// Higieniza as últimas `max_linhas` linhas de um log.
///
/// Remove CPF/CNPJ, e-mail, token de sessão, valores de senha/token e caminhos
/// absolutos. É uma proteção em profundidade: o pacote nunca deve ser enviado
/// sem revisão humana.
pub fn sanitizar_logs(texto: &str, max_linhas: usize) -> String {
    let mut linhas: Vec<String> = texto
        .lines()
        .rev()
        .take(max_linhas)
        .map(sanitizar_linha)
        .collect();
    linhas.reverse();
    linhas.join("\n")
}

fn sanitizar_linha(linha: &str) -> String {
    linha
        .split(' ')
        .map(sanitizar_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn sanitizar_token(token: &str) -> String {
    if token.is_empty() {
        return String::new();
    }

    let minusculo = token.to_ascii_lowercase();
    for chave in [
        "senha",
        "password",
        "passwd",
        "token",
        "authorization",
        "codigo",
        "secret",
        "api_key",
        "apikey",
    ] {
        if let Some(resto) = minusculo.strip_prefix(chave) {
            if let Some(separador) = resto.chars().next() {
                if separador == '=' || separador == ':' {
                    let corte = chave.len();
                    return format!("{}{}[REDIGIDO]", &token[..corte], separador);
                }
            }
        }
    }

    if minusculo.starts_with("bearer") {
        return "[AUTORIZACAO-REDIGIDA]".to_string();
    }

    // Token de sessão `mikella_<uuid-v4>`; módulos como `mikella_server:info`
    // são curtos e continuam visíveis.
    if token.len() > 30 && token.starts_with("mikella_") {
        return "mikella_[REDIGIDO]".to_string();
    }

    if token.contains('@') && token.contains('.') && !token.starts_with('@') {
        // Preserva a chave (`usuario=`, `email=`) e redige só o valor.
        if let Some((chave, _)) = token.split_once('=') {
            return format!("{chave}=[EMAIL-REDIGIDO]");
        }
        return "[EMAIL-REDIGIDO]".to_string();
    }

    let digitos = token.chars().filter(|c| c.is_ascii_digit()).count();
    let tem_separador = token.contains('.') || token.contains('-') || token.contains('/');
    let somente_digitos = token.chars().all(|c| c.is_ascii_digit());
    let documento_formatado = tem_separador && (digitos == 11 || digitos == 14);
    let documento_sem_mascara = somente_digitos && (token.len() == 11 || token.len() == 14);
    if documento_formatado || documento_sem_mascara {
        if let Some((chave, _)) = token.split_once('=') {
            return format!("{chave}=[DOCUMENTO-REDIGIDO]");
        }
        return "[DOCUMENTO-REDIGIDO]".to_string();
    }

    let bytes = token.as_bytes();
    if bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        return "<caminho-local>".to_string();
    }
    if minusculo.contains("\\users\\") || minusculo.contains("/home/") {
        return "<caminho-local>".to_string();
    }

    token.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_ieee_conhecido() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn zip_stored_tem_assinaturas_e_conteudo() {
        let zip = criar_zip(&[
            EntradaZip {
                nome: "diagnostico.json".into(),
                conteudo: br#"{"status":"ok"}"#.to_vec(),
            },
            EntradaZip {
                nome: "README.txt".into(),
                conteudo: b"suporte".to_vec(),
            },
        ]);

        assert_eq!(&zip[..4], b"PK\x03\x04");
        assert!(zip.windows(4).any(|w| w == b"PK\x05\x06"));
        assert!(zip.windows(16).any(|w| w == b"diagnostico.json"));
        assert!(zip.windows(7).any(|w| w == b"README."));
        assert!(zip.windows(7).any(|w| w == b"suporte"));
    }

    #[test]
    fn sanitiza_documentos_email_token_e_caminho() {
        let entrada = "usuario=fulano@example.com cpf=123.456.789-00 token=mikella_abcdefghijklmnopqrstuvwxyz123456 senha=segredo C:\\Users\\wilso\\loja.db mikella_server:info";
        let saida = sanitizar_logs(entrada, 10);

        assert!(!saida.contains("fulano@example.com"));
        assert!(!saida.contains("123.456.789-00"));
        assert!(!saida.contains("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!saida.contains("segredo"));
        assert!(!saida.contains("C:\\Users"));
        assert!(saida.contains("mikella_server:info"));
        assert!(saida.contains("usuario=[EMAIL-REDIGIDO]"));
        assert!(saida.contains("senha=[REDIGIDO]"));
    }

    #[test]
    fn sanitiza_cpf_e_cnpj_sem_mascara() {
        let saida = sanitizar_logs("cpf 12345678901 e cnpj 12345678000199", 10);
        assert!(!saida.contains("12345678901"));
        assert!(!saida.contains("12345678000199"));
        assert_eq!(saida.matches("[DOCUMENTO-REDIGIDO]").count(), 2);
    }

    #[test]
    fn logs_limitam_quantidade_de_linhas() {
        let entrada = (1..=10)
            .map(|i| format!("linha {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let saida = sanitizar_logs(&entrada, 3);
        assert_eq!(saida.lines().count(), 3);
        assert!(saida.contains("linha 10"));
        assert!(!saida.contains("linha 7"));
    }

    #[test]
    fn le_apenas_o_final_do_arquivo() {
        let dir = std::env::temp_dir().join(format!("mikella-log-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("app.log");
        let conteudo: String = (1..=5000).map(|i| format!("linha {i}\n")).collect();
        fs::write(&caminho, &conteudo).unwrap();

        let (bytes, meio_de_linha) = ler_final(&caminho, 64).unwrap();
        assert!(meio_de_linha, "o corte caiu no meio de uma linha");
        assert_eq!(bytes.len(), 64);
        let texto = String::from_utf8_lossy(&bytes);
        assert!(texto.ends_with('\n'), "deve terminar na última linha");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn arquivo_menor_que_o_limite_nao_e_truncado() {
        let dir = std::env::temp_dir().join(format!("mikella-log-curto-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("app.log");
        fs::write(&caminho, b"linha unica\n").unwrap();

        let (bytes, meio_de_linha) = ler_final(&caminho, 4096).unwrap();
        assert!(!meio_de_linha);
        assert_eq!(bytes, b"linha unica\n");

        fs::remove_dir_all(&dir).ok();
    }
}
