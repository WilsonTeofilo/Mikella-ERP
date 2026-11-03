//! Casos de uso do caixa: abrir, movimentar e fechar.
//!
//! A sessão nunca fecha sozinha. Se faltar energia ou internet, o estado fica
//! exatamente onde parou: a sessão continua `ABERTA` e o livro imutável guarda
//! cada movimento. O fechamento só acontece por comando explícito.

use mikella_domain_cash::livro::{EntradaLivro, Referencia, TipoMovimentacao};
use mikella_domain_cash::ports::{HashChain, LivroCaixaRepository, SessaoRepository};
use mikella_domain_cash::sessao::{DiferencaCaixa, EstadoSessao, SessaoCaixa};
use mikella_shared_kernel::clock::Clock;
use mikella_shared_kernel::error::{DomainError, DomainResult};
use mikella_shared_kernel::ids::{DispositivoId, LivroCaixaId, SessaoId, UsuarioId};
use mikella_shared_kernel::money::Money;

pub struct CaixaRepos<'a> {
    pub sessoes: &'a dyn SessaoRepository,
    pub livro: &'a dyn LivroCaixaRepository,
    pub hash: &'a dyn HashChain,
    pub clock: &'a dyn Clock,
}

#[derive(Debug)]
pub struct AbrirCaixaCmd {
    pub sessao_id: SessaoId,
    pub numero_caixa: u8,
    pub dispositivo_id: DispositivoId,
    pub operador_id: UsuarioId,
    pub saldo_inicial: Money,
    pub fechamento_cego: bool,
}

/// Abre a sessão e grava a entrada `ABERTURA` no livro.
/// Reenviar o mesmo `sessao_id` devolve a sessão existente (idempotente).
pub fn abrir_caixa(repos: &CaixaRepos<'_>, cmd: AbrirCaixaCmd) -> DomainResult<SessaoCaixa> {
    if let Some(existente) = repos.sessoes.buscar_por_id(cmd.sessao_id)? {
        if existente.dispositivo_id != cmd.dispositivo_id
            || existente.numero_caixa != cmd.numero_caixa
        {
            return Err(DomainError::PermissaoNegada(
                "Já existe uma sessão com este ID em outro caixa".into(),
            ));
        }
        return Ok(existente);
    }

    let abertas = repos.sessoes.buscar_sessoes_abertas()?;
    if let Some(conflito) = abertas.iter().find(|sessao| {
        sessao.dispositivo_id == cmd.dispositivo_id && sessao.numero_caixa == cmd.numero_caixa
    }) {
        return Err(DomainError::EstadoInvalido(format!(
            "O caixa {} deste dispositivo já está aberto (sessão {})",
            cmd.numero_caixa, conflito.id
        )));
    }

    let agora = repos.clock.now();
    let sessao = SessaoCaixa::abrir(
        cmd.sessao_id,
        cmd.numero_caixa,
        cmd.dispositivo_id,
        cmd.operador_id,
        cmd.saldo_inicial,
        cmd.fechamento_cego,
        agora,
    )?;

    repos.sessoes.salvar(&sessao)?;

    let posicao = repos.livro.posicao_atual(sessao.id)?;
    let mut entrada = EntradaLivro {
        id: LivroCaixaId::new(),
        sessao_id: sessao.id,
        sequencia: posicao.proxima_sequencia,
        tipo: TipoMovimentacao::Abertura,
        valor_centavos: cmd.saldo_inicial,
        troco_centavos: Money::ZERO,
        referencia: Some(Referencia {
            id: sessao.id.to_string(),
            tipo: "ABERTURA".to_string(),
        }),
        usuario_id: cmd.operador_id,
        aprovador_id: None,
        motivo: None,
        prev_hash: posicao.ultimo_hash,
        hash: String::new(),
        created_at: agora,
    };
    entrada.hash = repos.hash.hash_entrada(&entrada);
    repos.livro.inserir_entrada(&entrada)?;

    tracing::info!(
        sessao_id = %sessao.id,
        numero_caixa = sessao.numero_caixa,
        saldo_inicial_centavos = cmd.saldo_inicial.centavos(),
        "caixa aberto"
    );

    Ok(sessao)
}

#[derive(Debug)]
pub struct FecharCaixaCmd {
    pub sessao_id: SessaoId,
    /// Quem está fechando (pode ser o gerente, não o operador da sessão).
    /// Fica registrado no lançamento `FECHAMENTO` para auditoria.
    pub usuario_id: UsuarioId,
    /// Valor contado fisicamente pelo operador.
    pub saldo_contado: Money,
}

/// Fecha a sessão, grava o `FECHAMENTO` (informativo) e calcula a diferença.
///
/// Antes de fechar, recalcula a cadeia de hashes do livro inteiro. Se alguém
/// alterou uma linha direto no banco, o fechamento é recusado.
pub fn fechar_caixa(repos: &CaixaRepos<'_>, cmd: FecharCaixaCmd) -> DomainResult<DiferencaCaixa> {
    let mut sessao =
        repos
            .sessoes
            .buscar_por_id(cmd.sessao_id)?
            .ok_or_else(|| DomainError::NaoEncontrado {
                entidade: "SessaoCaixa",
                id: cmd.sessao_id.to_string(),
            })?;

    // Idempotência: sessão já fechada devolve o resultado gravado.
    if sessao.estado == EstadoSessao::Fechada {
        let contado = sessao.saldo_contado_centavos.unwrap_or(Money::ZERO);
        return Ok(DiferencaCaixa {
            esperado: sessao.saldo_esperado_centavos,
            contado,
            diferenca: contado - sessao.saldo_esperado_centavos,
        });
    }

    cmd.saldo_contado
        .assert_nao_negativo()
        .map_err(|_| DomainError::Invalido("Saldo contado não pode ser negativo".into()))?;

    let agora = repos.clock.now();

    let livro = repos.livro.carregar_livro(sessao.id)?;
    livro.verificar_integridade(|entrada| repos.hash.hash_entrada(entrada))?;

    let esperado_livro = livro.saldo_esperado()?;
    if esperado_livro != sessao.saldo_esperado_centavos {
        return Err(DomainError::InvarianteViolada(format!(
            "Projeção do caixa ({}) diverge do livro ({}). Feche o caixa apenas após investigar.",
            sessao.saldo_esperado_centavos, esperado_livro
        )));
    }

    if sessao.estado == EstadoSessao::Aberta {
        sessao.iniciar_fechamento()?;
    }

    let diferenca = sessao.finalizar_fechamento(cmd.saldo_contado, agora)?;

    let posicao = repos.livro.posicao_atual(sessao.id)?;
    let mut entrada = EntradaLivro {
        id: LivroCaixaId::new(),
        sessao_id: sessao.id,
        sequencia: posicao.proxima_sequencia,
        tipo: TipoMovimentacao::Fechamento,
        valor_centavos: cmd.saldo_contado,
        troco_centavos: Money::ZERO,
        referencia: Some(Referencia {
            id: sessao.id.to_string(),
            tipo: "FECHAMENTO".to_string(),
        }),
        usuario_id: cmd.usuario_id,
        aprovador_id: None,
        motivo: Some(format!(
            "esperado {} / contado {} / diferença {}",
            diferenca.esperado, diferenca.contado, diferenca.diferenca
        )),
        prev_hash: posicao.ultimo_hash,
        hash: String::new(),
        created_at: agora,
    };
    entrada.hash = repos.hash.hash_entrada(&entrada);
    repos.livro.inserir_entrada(&entrada)?;

    repos.sessoes.atualizar(&sessao)?;

    tracing::info!(
        sessao_id = %sessao.id,
        esperado_centavos = diferenca.esperado.centavos(),
        contado_centavos = diferenca.contado.centavos(),
        diferenca_centavos = diferenca.diferenca.centavos(),
        "caixa fechado"
    );

    Ok(diferenca)
}

/// Registra uma sangria (retirada de dinheiro) com aprovação.
pub fn registrar_sangria(
    repos: &CaixaRepos<'_>,
    sessao_id: SessaoId,
    valor: Money,
    operador_id: UsuarioId,
    aprovador_id: UsuarioId,
    motivo: String,
) -> DomainResult<()> {
    if valor.is_zero() || valor.is_negative() {
        return Err(DomainError::Invalido(
            "Sangria deve ter valor positivo".into(),
        ));
    }
    if motivo.trim().is_empty() {
        return Err(DomainError::Invalido("Sangria exige motivo".into()));
    }

    let sessao =
        repos
            .sessoes
            .buscar_por_id(sessao_id)?
            .ok_or_else(|| DomainError::NaoEncontrado {
                entidade: "SessaoCaixa",
                id: sessao_id.to_string(),
            })?;
    if !sessao.is_aberta() {
        return Err(DomainError::EstadoInvalido(
            "Sangria exige sessão aberta".into(),
        ));
    }
    if valor > sessao.saldo_esperado_centavos {
        return Err(DomainError::Invalido(format!(
            "Sangria ({}) maior que o saldo em caixa ({})",
            valor, sessao.saldo_esperado_centavos
        )));
    }

    let agora = repos.clock.now();
    let posicao = repos.livro.posicao_atual(sessao_id)?;
    let mut entrada = EntradaLivro {
        id: LivroCaixaId::new(),
        sessao_id,
        sequencia: posicao.proxima_sequencia,
        tipo: TipoMovimentacao::Sangria,
        valor_centavos: -valor,
        troco_centavos: Money::ZERO,
        referencia: None,
        usuario_id: operador_id,
        aprovador_id: Some(aprovador_id),
        motivo: Some(motivo),
        prev_hash: posicao.ultimo_hash,
        hash: String::new(),
        created_at: agora,
    };
    entrada.hash = repos.hash.hash_entrada(&entrada);
    repos.livro.inserir_entrada(&entrada)?;
    Ok(())
}
