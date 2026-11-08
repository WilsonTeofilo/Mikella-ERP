//! Repositórios SQLite — implementações das portas do domínio.
//!
//! Cada repositório empresta uma `&Connection`. Quando chamado dentro de uma
//! `Transaction`, o `Deref` do rusqlite faz o repositório participar da mesma
//! transação sem precisar de tipo genérico. Isso mantém o caso de uso atômico:
//! ou a venda inteira (cabeçalho, itens, pagamentos, estoque, caixa) entra,
//! ou nada entra.

pub mod auditoria;
pub mod caixa;
pub mod doacao;
pub mod financeiro;
pub mod financeiro_dashboard;
pub mod instance;
pub mod lotes;
pub mod movimentacoes;
pub mod pessoas;
pub mod produtos;
pub mod recibos;
pub mod sistema;
pub mod vendas;

use std::str::FromStr;

use chrono::{DateTime, Utc};

use mikella_shared_kernel::error::DomainResult;

use crate::error::{erro_campo, InfraError, InfraResult};

/// Converte um resultado de infraestrutura em resultado de domínio.
pub(crate) trait ParaDominio<T> {
    fn dominio(self) -> DomainResult<T>;
}

impl<T> ParaDominio<T> for InfraResult<T> {
    fn dominio(self) -> DomainResult<T> {
        self.map_err(Into::into)
    }
}

/// Converte erro de SQLite em resultado de domínio (atalho para `?`).
pub(crate) fn sql<T>(resultado: Result<T, rusqlite::Error>) -> DomainResult<T> {
    resultado.map_err(|erro| InfraError::Sqlite(erro.to_string()).into())
}

/// Interpreta um UUID vindo do banco.
pub(crate) fn parse_id<T>(valor: &str, campo: &'static str) -> InfraResult<T>
where
    T: FromStr<Err = uuid::Error>,
{
    valor.parse::<T>().map_err(|erro| erro_campo(campo, erro))
}

/// Interpreta um timestamp RFC3339 vindo do banco.
pub(crate) fn parse_data_hora(valor: &str, campo: &'static str) -> InfraResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(valor)
        .map(|data| data.with_timezone(&Utc))
        .map_err(|erro| erro_campo(campo, erro))
}

/// Reexporta o formato canônico para os repositórios (`super::data_hora_sql`).
pub(crate) use crate::tempo::data_hora_sql;

/// Interpreta uma data `AAAA-MM-DD`.
pub(crate) fn parse_data(valor: &str, campo: &'static str) -> InfraResult<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(valor, "%Y-%m-%d").map_err(|erro| erro_campo(campo, erro))
}

pub(crate) fn inteiro_para_bool(valor: i64, campo: &'static str) -> InfraResult<bool> {
    match valor {
        0 => Ok(false),
        1 => Ok(true),
        outro => Err(erro_campo(
            campo,
            format!("esperado 0 ou 1, encontrado {outro}"),
        )),
    }
}

pub(crate) fn bool_para_inteiro(valor: bool) -> i64 {
    i64::from(valor)
}

pub(crate) fn limit_seguro(params: &mikella_shared_kernel::pagination::PageParams) -> i64 {
    i64::from(params.limit_clamped(100).max(1))
}
