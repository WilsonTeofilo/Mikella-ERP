//! Casos de uso de contas a pagar e receber (RF-030).
//!
//! Use cases puros — recebem apenas traits do domínio, retornam `DomainResult`.
//! Não sabem de SQLite, HTTP ou Tauri.

use chrono::{DateTime, Utc};
use mikella_domain_finance::{
    conta::{ContaPagar, ContaReceber, PlanoConta, TipoConta},
    ports::{ContaPagarRepository, ContaReceberRepository, ParcelaRepository},
};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{ContaPagarId, ContaReceberId, UsuarioId},
    money::Money,
};
use serde::Deserialize;

/// Portas necessárias para os casos de uso financeiros.
pub struct ContasRepos<'a> {
    pub contas_a_pagar: &'a dyn ContaPagarRepository,
    pub contas_a_receber: &'a dyn ContaReceberRepository,
    pub parcelas: &'a dyn ParcelaRepository,
}

/// Comando para registrar uma conta a pagar.
pub struct RegistrarContaPagarCmd {
    pub descricao: String,
    pub plano_codigo: String,
    pub plano_nome: String,
    pub parcelas: Vec<(Money, DateTime<Utc>)>,
    pub usuario_id: UsuarioId,
}

/// Comando para registrar uma conta a receber.
pub struct RegistrarContaReceberCmd {
    pub descricao: String,
    pub plano_codigo: String,
    pub plano_nome: String,
    pub parcelas: Vec<(Money, DateTime<Utc>)>,
    pub usuario_id: UsuarioId,
}

/// Comando para liquidar uma parcela de conta a pagar.
pub struct LiquidarParcelaPagarCmd {
    pub conta_id: ContaPagarId,
    pub parcela_indice: usize,
    pub pago_em: DateTime<Utc>,
}

/// Comando para liquidar uma parcela de conta a receber.
pub struct LiquidarParcelaReceberCmd {
    pub conta_id: ContaReceberId,
    pub parcela_indice: usize,
    pub pago_em: DateTime<Utc>,
}

/// Query para listagem de contas por tipo.
#[derive(Debug, Deserialize)]
pub struct ListaContasQuery {
    pub tipo: TipoConta,
}

/// Registra uma nova conta a pagar.
pub fn registrar_conta_pagar(
    repos: &ContasRepos<'_>,
    cmd: RegistrarContaPagarCmd,
    agora: DateTime<Utc>,
) -> DomainResult<ContaPagarId> {
    let plano = PlanoConta::novo(cmd.plano_codigo, cmd.plano_nome)?;
    let mut conta = ContaPagar::nova(cmd.descricao, plano, cmd.usuario_id, agora)?;

    if cmd.parcelas.is_empty() {
        return Err(DomainError::Invalido("pelo menos uma parcela é obrigatória".into()));
    }

    for (valor, vencimento) in cmd.parcelas {
        let vencimento_ajustado = mikella_domain_finance::conta::ajustar_para_dia_util(vencimento);
        conta.adicionar_parcela(valor, vencimento_ajustado)?;
    }

    let id = conta.id;
    repos.contas_a_pagar.salvar(&conta)?;

    tracing::info!(
        conta_id = %id, usuario = %cmd.usuario_id,
        parcelas = conta.parcelas.len(),
        "conta a pagar registrada"
    );

    Ok(id)
}

/// Registra uma nova conta a receber.
pub fn registrar_conta_receber(
    repos: &ContasRepos<'_>,
    cmd: RegistrarContaReceberCmd,
    agora: DateTime<Utc>,
) -> DomainResult<ContaReceberId> {
    let plano = PlanoConta::novo(cmd.plano_codigo, cmd.plano_nome)?;
    let mut conta = ContaReceber::nova(cmd.descricao, plano, cmd.usuario_id, agora)?;

    if cmd.parcelas.is_empty() {
        return Err(DomainError::Invalido("pelo menos uma parcela é obrigatória".into()));
    }

    for (valor, vencimento) in cmd.parcelas {
        let vencimento_ajustado = mikella_domain_finance::conta::ajustar_para_dia_util(vencimento);
        conta.adicionar_parcela(valor, vencimento_ajustado)?;
    }

    let id = conta.id;
    repos.contas_a_receber.salvar(&conta)?;

    tracing::info!(
        conta_id = %id, usuario = %cmd.usuario_id,
        parcelas = conta.parcelas.len(),
        "conta a receber registrada"
    );

    Ok(id)
}

/// Liquidar uma parcela de conta a pagar.
pub fn liquidar_parcela_pagar(
    repos: &ContasRepos<'_>,
    cmd: LiquidarParcelaPagarCmd,
) -> DomainResult<()> {
    let mut conta = repos
        .contas_a_pagar
        .buscar_por_id(cmd.conta_id)?
        .ok_or_else(|| DomainError::NaoEncontrado {
            entidade: "ContaPagar",
            id: cmd.conta_id.to_string(),
        })?;

    conta.liquidar_parcela(cmd.parcela_indice, cmd.pago_em)?;
    repos.contas_a_pagar.atualizar(&conta)?;

    tracing::info!(
        conta_id = %cmd.conta_id, parcela = cmd.parcela_indice,
        "parcela paga"
    );

    Ok(())
}

/// Liquidar uma parcela de conta a receber.
pub fn liquidar_parcela_receber(
    repos: &ContasRepos<'_>,
    cmd: LiquidarParcelaReceberCmd,
) -> DomainResult<()> {
    let mut conta = repos
        .contas_a_receber
        .buscar_por_id(cmd.conta_id)?
        .ok_or_else(|| DomainError::NaoEncontrado {
            entidade: "ContaReceber",
            id: cmd.conta_id.to_string(),
        })?;

    conta.liquidar_parcela(cmd.parcela_indice, cmd.pago_em)?;
    repos.contas_a_receber.atualizar(&conta)?;

    tracing::info!(
        conta_id = %cmd.conta_id, parcela = cmd.parcela_indice,
        "parcela recebida"
    );

    Ok(())
}

/// Resultado de listagem — normaliza para JSON sem expor o tipo interno.
#[derive(Debug, serde::Serialize)]
pub struct ContaFinanceira {
    pub id: String,
    pub tipo: &'static str,
    pub descricao: String,
    pub plano: String,
    pub valor_total: i64,
    pub valor_pago: i64,
    pub valor_em_aberto: i64,
    pub status: &'static str,
}

impl ContaFinanceira {
    fn from_pagar(c: ContaPagar) -> DomainResult<Self> {
        let valor_total = c.valor_total()?.centavos();
        let valor_pago = c.valor_pago()?.centavos();
        let valor_em_aberto = c.valor_em_aberto()?.centavos();
        let status = c.status();
        Ok(Self {
            id: c.id.to_string(),
            tipo: "PAGAMENTO",
            descricao: c.descricao,
            plano: format!("{} - {}", c.plano_conta.codigo, c.plano_conta.nome),
            valor_total,
            valor_pago,
            valor_em_aberto,
            status,
        })
    }

    fn from_receber(c: ContaReceber) -> DomainResult<Self> {
        let valor_total = c.valor_total()?.centavos();
        let valor_pago = c.valor_pago()?.centavos();
        let valor_em_aberto = c.valor_em_aberto()?.centavos();
        let status = c.status();
        Ok(Self {
            id: c.id.to_string(),
            tipo: "RECEBIMENTO",
            descricao: c.descricao,
            plano: format!("{} - {}", c.plano_conta.codigo, c.plano_conta.nome),
            valor_total,
            valor_pago,
            valor_em_aberto,
            status,
        })
    }
}

/// Lista contas abertas (não totalmente pagas).
pub fn listar_contas(
    repos: &ContasRepos<'_>,
    query: ListaContasQuery,
) -> DomainResult<Vec<ContaFinanceira>> {
    match query.tipo {
        TipoConta::Pagamento => repos
            .contas_a_pagar
            .buscar_abertas()?
            .into_iter()
            .map(ContaFinanceira::from_pagar)
            .collect(),
        TipoConta::Recebimento => repos
            .contas_a_receber
            .buscar_abertas()?
            .into_iter()
            .map(ContaFinanceira::from_receber)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    use mikella_domain_finance::conta::Parcela;
    use mikella_shared_kernel::ids::ParcelaId;

    fn agora() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap()
    }

    /// Mock simples para testes.
    struct MockRepos {
        pagar: Vec<ContaPagar>,
        receber: Vec<ContaReceber>,
    }

    impl ContaPagarRepository for MockRepos {
        fn salvar(&self, _c: &ContaPagar) -> DomainResult<()> { Ok(()) }
        fn atualizar(&self, _c: &ContaPagar) -> DomainResult<()> { Ok(()) }
        fn buscar_por_id(&self, id: ContaPagarId) -> DomainResult<Option<ContaPagar>> {
            Ok(self.pagar.iter().find(|c| c.id == id).cloned())
        }
        fn buscar_abertas(&self) -> DomainResult<Vec<ContaPagar>> { Ok(vec![]) }
        fn listar_planos(&self) -> DomainResult<Vec<PlanoConta>> { Ok(vec![]) }
        fn cancelar_abertas_por_referencia(&self, _referencia_id: &str) -> DomainResult<u32> { Ok(0) }
    }

    impl ContaReceberRepository for MockRepos {
        fn salvar(&self, _c: &ContaReceber) -> DomainResult<()> { Ok(()) }
        fn atualizar(&self, _c: &ContaReceber) -> DomainResult<()> { Ok(()) }
        fn buscar_por_id(&self, id: ContaReceberId) -> DomainResult<Option<ContaReceber>> {
            Ok(self.receber.iter().find(|c| c.id == id).cloned())
        }
        fn buscar_abertas(&self) -> DomainResult<Vec<ContaReceber>> { Ok(vec![]) }
        fn listar_planos(&self) -> DomainResult<Vec<PlanoConta>> { Ok(vec![]) }
        fn cancelar_abertas_por_referencia(&self, _referencia_id: &str) -> DomainResult<u32> { Ok(0) }
    }

    impl ParcelaRepository for MockRepos {
        fn buscar_por_id(&self, _id: ParcelaId) -> DomainResult<Option<(ContaPagarId, Parcela)>> { Ok(None) }
        fn buscar_vencidas_ate(&self, _dt: DateTime<Utc>, _uid: UsuarioId) -> DomainResult<Vec<(ContaPagarId, Parcela)>> { Ok(vec![]) }
        fn listar_abertas(&self, _uid: UsuarioId) -> DomainResult<Vec<(ContaPagarId, Parcela)>> { Ok(vec![]) }
    }

    fn setup() -> (MockRepos, ContasRepos<'static>) {
        let mock = Box::leak(Box::new(MockRepos { pagar: vec![], receber: vec![] }));
        let repos = ContasRepos {
            contas_a_pagar: mock,
            contas_a_receber: mock,
            parcelas: mock,
        };
        (MockRepos { pagar: vec![], receber: vec![] }, repos)
    }

    #[test]
    fn registrar_conta_pagar_sucesso() {
        let (_, repos) = setup();
        let cmd = RegistrarContaPagarCmd {
            descricao: "Compra".into(),
            plano_codigo: "3.1.01".into(),
            plano_nome: "Fornecedores".into(),
            parcelas: vec![
                (Money::from_centavos(10000), agora()),
                (Money::from_centavos(5000), agora()),
            ],
            usuario_id: UsuarioId::new(),
        };
        assert!(registrar_conta_pagar(&repos, cmd, agora()).is_ok());
    }

    #[test]
    fn registrar_conta_pagar_sem_parcelas_falha() {
        let (_, repos) = setup();
        let cmd = RegistrarContaPagarCmd {
            descricao: "Conta".into(),
            plano_codigo: "3.1.01".into(),
            plano_nome: "Fornecedores".into(),
            parcelas: vec![],
            usuario_id: UsuarioId::new(),
        };
        assert!(registrar_conta_pagar(&repos, cmd, agora()).is_err());
    }

    #[test]
    fn listar_contas_vazia() {
        let (_, repos) = setup();
        let query = ListaContasQuery {
            tipo: TipoConta::Pagamento,
        };
        let result = listar_contas(&repos, query).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn liquidar_parcela_por_indice_invalido() {
        let (_, repos) = setup();
        let cmd = LiquidarParcelaPagarCmd {
            conta_id: ContaPagarId::new(),
            parcela_indice: 999,
            pago_em: agora(),
        };
        assert!(liquidar_parcela_pagar(&repos, cmd).is_err());
    }
}
