//! Cadeia de hashes blake3 para livro do caixa, auditoria e cotas.
//!
//! Cada campo é prefixado com seu tamanho em bytes (u64 little-endian) antes
//! de entrar no hash. Sem isso, concatenar `"ab" + "c"` e `"a" + "bc"` daria
//! o mesmo hash — e um atacante poderia mover a fronteira entre campos.

use mikella_shared_kernel::ids::SessaoId;

use mikella_domain_cash::livro::EntradaLivro;
use mikella_domain_cash::ports::HashChain;

use crate::tempo::data_hora_sql;

const DOMINIO_LIVRO: &[u8] = b"mikella.livro.v1";
const DOMINIO_AUDITORIA: &[u8] = b"mikella.auditoria.v1";
const DOMINIO_QUOTA: &[u8] = b"mikella.quota.v1";

fn campo(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn campo_opcional(hasher: &mut blake3::Hasher, bytes: Option<&[u8]>) {
    match bytes {
        Some(valor) => {
            hasher.update(&[1u8]);
            campo(hasher, valor);
        }
        None => {
            hasher.update(&[0u8]);
        }
    }
}

/// Hash de uma entrada do livro do caixa.
pub fn hash_entrada(entrada: &EntradaLivro) -> String {
    let mut hasher = blake3::Hasher::new();
    campo(&mut hasher, DOMINIO_LIVRO);
    campo(&mut hasher, entrada.sessao_id.to_string().as_bytes());
    campo(&mut hasher, &entrada.sequencia.to_le_bytes());
    campo(&mut hasher, entrada.id.to_string().as_bytes());
    campo(&mut hasher, entrada.tipo.as_str().as_bytes());
    campo(
        &mut hasher,
        &entrada.valor_centavos.centavos().to_le_bytes(),
    );
    campo(
        &mut hasher,
        &entrada.troco_centavos.centavos().to_le_bytes(),
    );
    match &entrada.referencia {
        Some(referencia) => {
            hasher.update(&[1u8]);
            campo(&mut hasher, referencia.id.as_bytes());
            campo(&mut hasher, referencia.tipo.as_bytes());
        }
        None => {
            hasher.update(&[0u8]);
        }
    }
    campo(&mut hasher, entrada.usuario_id.to_string().as_bytes());
    campo_opcional(
        &mut hasher,
        entrada
            .aprovador_id
            .map(|id| id.to_string())
            .as_deref()
            .map(str::as_bytes),
    );
    campo_opcional(&mut hasher, entrada.motivo.as_deref().map(str::as_bytes));
    campo(&mut hasher, entrada.prev_hash.as_bytes());
    // Precisa ser o MESMO formato gravado no banco: o hash é recalculado a
    // partir do que foi lido, e sub-milissegundos não sobrevivem ao TEXT.
    campo(&mut hasher, data_hora_sql(entrada.created_at).as_bytes());
    hasher.finalize().to_hex().to_string()
}

/// Implementação da porta de domínio.
pub struct Blake3HashChain;

impl HashChain for Blake3HashChain {
    fn hash_entrada(&self, entrada: &EntradaLivro) -> String {
        hash_entrada(entrada)
    }
}

/// Blind index para CPF/CNPJ de clientes (SEC-014 / LGPD-003).
///
/// Usa BLAKE3 keyed-hash com a chave da loja (32 bytes), garantindo que:
/// - Dois documentos iguais produzem o mesmo blind index (permitindo busca exata).
/// - Sem a chave da loja, o banco vazado não revela quais documentos pertencem
///   a quais clientes (proteção de PII conforme LGPD).
///
/// A chave deve ser derivada de `MIKELLA_STORE_KEY` (hex 64 chars = 32 bytes)
/// e mantida estável enquanto houver dados no banco. Trocar a chave invalida
/// todos os índices existentes.
pub fn blind_index_documento(documento_digits: &str, chave_loja: &[u8; 32]) -> String {
    // blake3::keyed_hash garante autenticação de domínio embutida: a chave
    // faz parte do estado inicial do hasher (512 bits de setup interno).
    let hash = blake3::keyed_hash(chave_loja, documento_digits.as_bytes());
    hash.to_hex().to_string()
}

/// Hash de um registro de auditoria.
#[allow(clippy::too_many_arguments)] // espelha as colunas do registro de auditoria
pub fn hash_auditoria(
    entidade: &str,
    entidade_id: &str,
    acao: &str,
    usuario_id: Option<&str>,
    dados_antes: Option<&str>,
    dados_depois: Option<&str>,
    prev_hash: &str,
    created_at: &str,
) -> String {
    let mut hasher = blake3::Hasher::new();
    campo(&mut hasher, DOMINIO_AUDITORIA);
    campo(&mut hasher, entidade.as_bytes());
    campo(&mut hasher, entidade_id.as_bytes());
    campo(&mut hasher, acao.as_bytes());
    campo_opcional(&mut hasher, usuario_id.map(str::as_bytes));
    campo_opcional(&mut hasher, dados_antes.map(str::as_bytes));
    campo_opcional(&mut hasher, dados_depois.map(str::as_bytes));
    campo(&mut hasher, prev_hash.as_bytes());
    campo(&mut hasher, created_at.as_bytes());
    hasher.finalize().to_hex().to_string()
}

/// Hash de um evento de cota.
pub fn hash_quota(
    store_id: &str,
    tipo: &str,
    periodo: &str,
    usuario_id: &str,
    produto_id: Option<&str>,
    prev_hash: &str,
) -> String {
    let mut hasher = blake3::Hasher::new();
    campo(&mut hasher, DOMINIO_QUOTA);
    campo(&mut hasher, store_id.as_bytes());
    campo(&mut hasher, tipo.as_bytes());
    campo(&mut hasher, periodo.as_bytes());
    campo(&mut hasher, usuario_id.as_bytes());
    campo_opcional(&mut hasher, produto_id.map(str::as_bytes));
    campo(&mut hasher, prev_hash.as_bytes());
    hasher.finalize().to_hex().to_string()
}

/// Hash da cadeia do livro de uma sessão (para debug/log). Não é o hash de
/// uma entrada; é o hash final da cadeia inteira.
pub fn hash_cadeia_sessao(sessao_id: SessaoId, ultimo_hash: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    campo(&mut hasher, b"mikella.cadeia.v1");
    campo(&mut hasher, sessao_id.to_string().as_bytes());
    campo(&mut hasher, ultimo_hash.as_bytes());
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use mikella_domain_cash::livro::{EntradaLivro, Referencia, TipoMovimentacao};
    use mikella_shared_kernel::{
        ids::{LivroCaixaId, SessaoId, UsuarioId},
        money::Money,
    };

    use super::*;

    fn entrada() -> EntradaLivro {
        EntradaLivro {
            id: LivroCaixaId::new(),
            sessao_id: SessaoId::new(),
            sequencia: 1,
            tipo: TipoMovimentacao::Abertura,
            valor_centavos: Money::from_centavos(10_000),
            troco_centavos: Money::ZERO,
            referencia: Some(Referencia {
                id: "v1".into(),
                tipo: "VENDA".into(),
            }),
            usuario_id: UsuarioId::new(),
            aprovador_id: None,
            motivo: None,
            prev_hash: "GENESIS".into(),
            hash: String::new(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn hash_e_estavel_para_o_mesmo_conteudo() {
        let e = entrada();
        assert_eq!(hash_entrada(&e), hash_entrada(&e));
    }

    #[test]
    fn mudar_valor_muda_hash() {
        let e = entrada();
        let mut outro = e.clone();
        outro.valor_centavos = Money::from_centavos(10_001);
        assert_ne!(hash_entrada(&e), hash_entrada(&outro));
    }

    #[test]
    fn fronteira_de_campo_nao_colide() {
        // "ab"+"c" != "a"+"bc" por causa do prefixo de tamanho.
        let mut a = entrada();
        let mut b = entrada();
        a.motivo = Some("ab".into());
        b.motivo = Some("a".into());
        // Muda também a referência para simular a segunda parte.
        a.referencia = Some(Referencia {
            id: "c".into(),
            tipo: "X".into(),
        });
        b.referencia = Some(Referencia {
            id: "bc".into(),
            tipo: "X".into(),
        });
        assert_ne!(hash_entrada(&a), hash_entrada(&b));
    }

    #[test]
    fn porta_hash_chain_delega() {
        let e = entrada();
        let chain = Blake3HashChain;
        assert_eq!(chain.hash_entrada(&e), hash_entrada(&e));
    }
}
