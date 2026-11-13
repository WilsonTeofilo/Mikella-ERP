use chrono::{DateTime, Utc};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{LivroCaixaId, SessaoId, UsuarioId},
    money::Money,
};
use serde::{Deserialize, Serialize};

/// Tipo de movimentação no livro do caixa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TipoMovimentacao {
    Abertura,
    VendaDinheiro,
    VendaDebito,
    VendaCredito,
    VendaPix,
    VendaOutro,
    Suprimento,
    Sangria,
    Estorno,
    Cancelamento,
    Fechamento,
}

impl TipoMovimentacao {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Abertura => "ABERTURA",
            Self::VendaDinheiro => "VENDA_DINHEIRO",
            Self::VendaDebito => "VENDA_DEBITO",
            Self::VendaCredito => "VENDA_CREDITO",
            Self::VendaPix => "VENDA_PIX",
            Self::VendaOutro => "VENDA_OUTRO",
            Self::Suprimento => "SUPRIMENTO",
            Self::Sangria => "SANGRIA",
            Self::Estorno => "ESTORNO",
            Self::Cancelamento => "CANCELAMENTO",
            Self::Fechamento => "FECHAMENTO",
        }
    }

    pub fn from_db(valor: &str) -> DomainResult<Self> {
        match valor {
            "ABERTURA" => Ok(Self::Abertura),
            "VENDA_DINHEIRO" => Ok(Self::VendaDinheiro),
            "VENDA_DEBITO" => Ok(Self::VendaDebito),
            "VENDA_CREDITO" => Ok(Self::VendaCredito),
            "VENDA_PIX" => Ok(Self::VendaPix),
            "VENDA_OUTRO" => Ok(Self::VendaOutro),
            "SUPRIMENTO" => Ok(Self::Suprimento),
            "SANGRIA" => Ok(Self::Sangria),
            "ESTORNO" => Ok(Self::Estorno),
            "CANCELAMENTO" => Ok(Self::Cancelamento),
            "FECHAMENTO" => Ok(Self::Fechamento),
            outro => Err(DomainError::Invalido(format!(
                "Tipo de movimentação desconhecido no banco: {outro}"
            ))),
        }
    }

    /// Efeito no saldo de dinheiro físico no caixa.
    ///
    /// Débito/crédito/pix não movimentam dinheiro físico, e `Fechamento`
    /// é apenas informativo (registra contagem, esperado e diferença) —
    /// incluí-lo no somatório duplicaria o saldo. Esta é a única fonte de
    /// verdade sobre o que entra no saldo esperado.
    pub fn afeta_saldo_fisico(&self) -> bool {
        matches!(
            self,
            Self::Abertura
                | Self::VendaDinheiro
                | Self::Suprimento
                | Self::Sangria
                | Self::Estorno
                | Self::Cancelamento
        )
    }
}

/// Referência ao documento que originou a movimentação.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Referencia {
    pub id: String,
    pub tipo: String, // "VENDA", "ESTORNO", "SUPRIMENTO", etc.
}

/// Uma entrada imutável no livro do caixa.
///
/// Após gravada, NUNCA pode ser alterada ou removida.
/// Correções são feitas por movimentações compensatórias.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntradaLivro {
    pub id: LivroCaixaId,
    pub sessao_id: SessaoId,
    /// Posição na cadeia da sessão. Começa em 1 e nunca tem buracos.
    /// Existe para dar ordem determinística (created_at pode empatar no mesmo milissegundo).
    pub sequencia: i64,
    pub tipo: TipoMovimentacao,
    /// Valor líquido: positivo = entrada, negativo = saída.
    pub valor_centavos: Money,
    /// Troco entregue (só para VENDA_DINHEIRO).
    pub troco_centavos: Money,
    pub referencia: Option<Referencia>,
    pub usuario_id: UsuarioId,
    pub aprovador_id: Option<UsuarioId>,
    pub motivo: Option<String>,
    /// Hash blake3(prev_hash || campos relevantes) — FRD-053
    pub prev_hash: String,
    pub hash: String,
    pub created_at: DateTime<Utc>,
}

/// O livro do caixa — sequência imutável de movimentações de uma sessão.
#[derive(Debug)]
pub struct LivroCaixa {
    pub sessao_id: SessaoId,
    entradas: Vec<EntradaLivro>,
}

impl LivroCaixa {
    pub fn novo(sessao_id: SessaoId) -> Self {
        Self {
            sessao_id,
            entradas: Vec::new(),
        }
    }

    /// Reconstrói o livro a partir de entradas já persistidas (ordenadas por created_at ASC).
    pub fn de_entradas(sessao_id: SessaoId, entradas: Vec<EntradaLivro>) -> Self {
        Self {
            sessao_id,
            entradas,
        }
    }

    /// Calcula o saldo esperado como Σ(valor - troco) apenas das movimentações
    /// que afetam dinheiro físico (`TipoMovimentacao::afeta_saldo_fisico`).
    ///
    /// Venda em débito/crédito/Pix NÃO aumenta o dinheiro na gaveta, e
    /// `Fechamento` é informativo. Somar tudo aqui é o bug que faz o caixa
    /// "voltar errado" — o valor oficial nunca vem de uma coluna armazenada.
    pub fn saldo_esperado(&self) -> DomainResult<Money> {
        self.entradas
            .iter()
            .filter(|e| e.tipo.afeta_saldo_fisico())
            .try_fold(Money::ZERO, |acc, e| {
                acc.somar(e.valor_centavos)
                    .and_then(|s| s.subtrair(e.troco_centavos))
            })
            .map_err(|e| DomainError::InvarianteViolada(e.to_string()))
    }

    /// Retorna o hash da última entrada (necessário para encadear a próxima).
    pub fn ultimo_hash(&self) -> &str {
        self.entradas
            .last()
            .map(|e| e.hash.as_str())
            .unwrap_or("GENESIS") // hash da primeira entrada usa "GENESIS" como prev_hash
    }

    pub fn entradas(&self) -> &[EntradaLivro] {
        &self.entradas
    }

    pub fn len(&self) -> usize {
        self.entradas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entradas.is_empty()
    }

    /// Verifica apenas o encadeamento (`prev_hash` de cada entrada aponta para
    /// o hash da anterior). Não prova que o conteúdo não foi alterado.
    pub fn verificar_encadeamento(&self) -> DomainResult<()> {
        let mut prev = "GENESIS";
        for entrada in &self.entradas {
            if entrada.prev_hash != prev {
                return Err(DomainError::InvarianteViolada(format!(
                    "Cadeia de hash quebrada no livro da sessão {:?} na entrada {:?}",
                    self.sessao_id, entrada.id
                )));
            }
            prev = &entrada.hash;
        }
        Ok(())
    }

    /// Verifica integridade completa: encadeamento **e** recálculo do hash de
    /// cada entrada com a função fornecida pela infraestrutura (blake3).
    ///
    /// O domínio não conhece o algoritmo; recebe a função pronta.
    pub fn verificar_integridade<F>(&self, recalcular: F) -> DomainResult<()>
    where
        F: Fn(&EntradaLivro) -> String,
    {
        self.verificar_encadeamento()?;
        for entrada in &self.entradas {
            if recalcular(entrada) != entrada.hash {
                return Err(DomainError::InvarianteViolada(format!(
                    "Hash divergente no livro da sessão {:?} na entrada {:?}",
                    self.sessao_id, entrada.id
                )));
            }
        }
        Ok(())
    }

    /// Adiciona uma nova entrada ao livro (apenas em memória; persistência é responsabilidade do infra).
    pub fn adicionar(&mut self, entrada: EntradaLivro) -> DomainResult<()> {
        if entrada.sessao_id != self.sessao_id {
            return Err(DomainError::InvarianteViolada(
                "Entrada pertence a outra sessão".into(),
            ));
        }
        let esperada = self.proxima_sequencia();
        if entrada.sequencia != esperada {
            return Err(DomainError::InvarianteViolada(format!(
                "Sequência fora de ordem: esperada {esperada}, recebida {}",
                entrada.sequencia
            )));
        }
        if entrada.prev_hash != self.ultimo_hash() {
            return Err(DomainError::InvarianteViolada(
                "Hash anterior não corresponde ao último hash do livro".into(),
            ));
        }
        self.entradas.push(entrada);
        Ok(())
    }

    /// Próxima posição livre na cadeia da sessão (1 para livro vazio).
    pub fn proxima_sequencia(&self) -> i64 {
        self.entradas.last().map(|e| e.sequencia + 1).unwrap_or(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saldo_vazio_e_zero() {
        let livro = LivroCaixa::novo(SessaoId::new());
        assert_eq!(livro.saldo_esperado().unwrap().centavos(), 0);
    }

    #[test]
    fn saldo_soma_corretamente() {
        let sessao_id = SessaoId::new();
        let mut livro = LivroCaixa::novo(sessao_id);

        let abertura = EntradaLivro {
            id: LivroCaixaId::new(),
            sessao_id,
            sequencia: 1,
            tipo: TipoMovimentacao::Abertura,
            valor_centavos: Money::from_centavos(50000), // R$ 500
            troco_centavos: Money::ZERO,
            referencia: None,
            usuario_id: UsuarioId::new(),
            aprovador_id: None,
            motivo: None,
            prev_hash: "GENESIS".into(),
            hash: "HASH_ABERTURA".into(),
            created_at: Utc::now(),
        };
        livro.adicionar(abertura).unwrap();

        let venda = EntradaLivro {
            id: LivroCaixaId::new(),
            sessao_id,
            sequencia: 2,
            tipo: TipoMovimentacao::VendaDinheiro,
            valor_centavos: Money::from_centavos(15000), // R$ 150 recebido
            troco_centavos: Money::from_centavos(1000),  // R$ 10 de troco
            referencia: Some(Referencia {
                id: "venda-1".into(),
                tipo: "VENDA".into(),
            }),
            usuario_id: UsuarioId::new(),
            aprovador_id: None,
            motivo: None,
            prev_hash: "HASH_ABERTURA".into(),
            hash: "HASH_VENDA".into(),
            created_at: Utc::now(),
        };
        livro.adicionar(venda).unwrap();

        // saldo = 500 + (150 - 10) = 640
        assert_eq!(livro.saldo_esperado().unwrap().centavos(), 64000);
    }

    fn entrada(
        sessao_id: SessaoId,
        seq: i64,
        tipo: TipoMovimentacao,
        valor: i64,
        troco: i64,
        prev: &str,
        hash: &str,
    ) -> EntradaLivro {
        EntradaLivro {
            id: LivroCaixaId::new(),
            sessao_id,
            sequencia: seq,
            tipo,
            valor_centavos: Money::from_centavos(valor),
            troco_centavos: Money::from_centavos(troco),
            referencia: None,
            usuario_id: UsuarioId::new(),
            aprovador_id: None,
            motivo: None,
            prev_hash: prev.into(),
            hash: hash.into(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn venda_eletronica_nao_infla_saldo_fisico() {
        let sessao = SessaoId::new();
        let mut livro = LivroCaixa::novo(sessao);
        livro
            .adicionar(entrada(
                sessao,
                1,
                TipoMovimentacao::Abertura,
                50_000,
                0,
                "GENESIS",
                "H1",
            ))
            .unwrap();
        livro
            .adicionar(entrada(
                sessao,
                2,
                TipoMovimentacao::VendaPix,
                100_000,
                0,
                "H1",
                "H2",
            ))
            .unwrap();
        livro
            .adicionar(entrada(
                sessao,
                3,
                TipoMovimentacao::VendaDebito,
                50_000,
                0,
                "H2",
                "H3",
            ))
            .unwrap();
        livro
            .adicionar(entrada(
                sessao,
                4,
                TipoMovimentacao::VendaDinheiro,
                20_000,
                5_000,
                "H3",
                "H4",
            ))
            .unwrap();

        // 500 de abertura + 150 (200 recebidos - 50 de troco). Pix/débito não contam.
        assert_eq!(livro.saldo_esperado().unwrap().centavos(), 65_000);
    }

    #[test]
    fn fechamento_nao_duplica_saldo() {
        let sessao = SessaoId::new();
        let mut livro = LivroCaixa::novo(sessao);
        livro
            .adicionar(entrada(
                sessao,
                1,
                TipoMovimentacao::Abertura,
                10_000,
                0,
                "GENESIS",
                "H1",
            ))
            .unwrap();
        livro
            .adicionar(entrada(
                sessao,
                2,
                TipoMovimentacao::Fechamento,
                10_000,
                0,
                "H1",
                "H2",
            ))
            .unwrap();
        assert_eq!(livro.saldo_esperado().unwrap().centavos(), 10_000);
    }

    #[test]
    fn sangria_reduz_saldo() {
        let sessao = SessaoId::new();
        let mut livro = LivroCaixa::novo(sessao);
        livro
            .adicionar(entrada(
                sessao,
                1,
                TipoMovimentacao::Abertura,
                50_000,
                0,
                "GENESIS",
                "H1",
            ))
            .unwrap();
        livro
            .adicionar(entrada(
                sessao,
                2,
                TipoMovimentacao::Sangria,
                -20_000,
                0,
                "H1",
                "H2",
            ))
            .unwrap();
        assert_eq!(livro.saldo_esperado().unwrap().centavos(), 30_000);
    }

    #[test]
    fn sequencia_fora_de_ordem_falha() {
        let sessao = SessaoId::new();
        let mut livro = LivroCaixa::novo(sessao);
        livro
            .adicionar(entrada(
                sessao,
                1,
                TipoMovimentacao::Abertura,
                10_000,
                0,
                "GENESIS",
                "H1",
            ))
            .unwrap();
        let r = livro.adicionar(entrada(
            sessao,
            3,
            TipoMovimentacao::VendaDinheiro,
            1_000,
            0,
            "H1",
            "H2",
        ));
        assert!(r.is_err());
    }

    #[test]
    fn encadeamento_quebrado_falha() {
        let sessao = SessaoId::new();
        let mut livro = LivroCaixa::novo(sessao);
        livro
            .adicionar(entrada(
                sessao,
                1,
                TipoMovimentacao::Abertura,
                10_000,
                0,
                "GENESIS",
                "H1",
            ))
            .unwrap();
        livro
            .adicionar(entrada(
                sessao,
                2,
                TipoMovimentacao::VendaDinheiro,
                1_000,
                0,
                "H1",
                "H2",
            ))
            .unwrap();
        assert!(livro.verificar_encadeamento().is_ok());

        // Alguém alterou o prev_hash da segunda entrada.
        let mut entradas = livro.entradas().to_vec();
        entradas[1].prev_hash = "HACK".into();
        let adulterado = LivroCaixa::de_entradas(sessao, entradas);
        assert!(adulterado.verificar_encadeamento().is_err());
    }

    #[test]
    fn hash_divergente_falha() {
        let sessao = SessaoId::new();
        let mut livro = LivroCaixa::novo(sessao);
        livro
            .adicionar(entrada(
                sessao,
                1,
                TipoMovimentacao::Abertura,
                10_000,
                0,
                "GENESIS",
                "H1",
            ))
            .unwrap();

        // Hasher de teste: sem o conteúdo esperado, o hash não confere.
        let resultado =
            livro.verificar_integridade(|e| format!("{}|{}", e.id, e.valor_centavos.centavos()));
        assert!(resultado.is_err());
    }
}
