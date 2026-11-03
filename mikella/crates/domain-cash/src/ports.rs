use crate::{
    livro::{EntradaLivro, LivroCaixa},
    sessao::SessaoCaixa,
};
/// Portas (traits) do domínio do caixa — implementadas pela camada de infra.
/// O domínio NUNCA conhece SQLite, axum ou Tauri; apenas estas traits.
///
/// As traits são síncronas de propósito: o banco é local e o servidor usa um
/// único escritor. Isso permite que a infra empreste `&Connection`/`&Transaction`
/// sem `Mutex` compartilhado nem `unsafe`.
use mikella_shared_kernel::{error::DomainResult, ids::SessaoId};

/// Repositório de sessões de caixa.
pub trait SessaoRepository {
    fn salvar(&self, sessao: &SessaoCaixa) -> DomainResult<()>;
    fn buscar_por_id(&self, id: SessaoId) -> DomainResult<Option<SessaoCaixa>>;
    fn buscar_sessoes_abertas(&self) -> DomainResult<Vec<SessaoCaixa>>;
    fn atualizar(&self, sessao: &SessaoCaixa) -> DomainResult<()>;
}

/// Posição atual da cadeia de uma sessão — permite encadear a próxima
/// entrada em O(1) sem carregar o livro inteiro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PosicaoLivro {
    /// Hash da última entrada, ou `"GENESIS"` se o livro está vazio.
    pub ultimo_hash: String,
    /// Sequência que a próxima entrada deve usar.
    pub proxima_sequencia: i64,
}

/// Repositório do livro do caixa.
pub trait LivroCaixaRepository {
    /// Persiste uma nova entrada (imutável — INSERT only, nunca UPDATE/DELETE).
    fn inserir_entrada(&self, entrada: &EntradaLivro) -> DomainResult<()>;
    /// Carrega todo o livro de uma sessão (ordenado por sequência ASC).
    fn carregar_livro(&self, sessao_id: SessaoId) -> DomainResult<LivroCaixa>;
    /// Posição da cadeia para encadear a próxima entrada.
    fn posicao_atual(&self, sessao_id: SessaoId) -> DomainResult<PosicaoLivro>;
}

/// Calcula o hash de uma entrada do livro. Implementado na infraestrutura
/// (blake3); o domínio apenas declara a porta.
pub trait HashChain {
    fn hash_entrada(&self, entrada: &EntradaLivro) -> String;
}
