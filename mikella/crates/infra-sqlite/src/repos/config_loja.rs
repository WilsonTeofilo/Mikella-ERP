//! Configuração da loja na tabela `loja_config` (chave/valor).
//!
//! A tabela existe desde a migração 0010. Este repositório centraliza a
//! leitura tipada para que handlers e repositórios não repitam parsing nem
//! valores mágicos. Rust é a autoridade: a UI apenas renderiza o que a API
//! devolve.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::InfraResult;

/// Máximo de parcelas no cartão quando a loja nunca configurou o valor.
pub const MAX_PARCELAS_CARTAO_PADRAO: u32 = 12;
/// Prazo do crediário quando a loja nunca configurou o valor.
pub const PRAZO_CREDIARIO_DIAS_PADRAO: u32 = 30;
/// Teto rígido aceito pela API (evita configuração absurda por engano).
pub const MAX_PARCELAS_CARTAO_LIMITE: u32 = 24;
/// Teto rígido aceito pela API para o prazo do crediário.
pub const PRAZO_CREDIARIO_DIAS_LIMITE: u32 = 365;

/// Limites de pagamento vigentes para a loja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigPagamento {
    pub max_parcelas_cartao: u32,
    pub prazo_crediario_dias: u32,
}

impl Default for ConfigPagamento {
    fn default() -> Self {
        Self {
            max_parcelas_cartao: MAX_PARCELAS_CARTAO_PADRAO,
            prazo_crediario_dias: PRAZO_CREDIARIO_DIAS_PADRAO,
        }
    }
}

impl ConfigPagamento {
    /// Valida uma configuração vinda da API.
    ///
    /// A mensagem é segura para o cliente: sem SQL, caminho ou stack.
    pub fn validar(max_parcelas_cartao: u32, prazo_crediario_dias: u32) -> Result<Self, String> {
        if !(1..=MAX_PARCELAS_CARTAO_LIMITE).contains(&max_parcelas_cartao) {
            return Err(format!(
                "máximo de parcelas no cartão deve ficar entre 1 e {MAX_PARCELAS_CARTAO_LIMITE}"
            ));
        }
        if !(1..=PRAZO_CREDIARIO_DIAS_LIMITE).contains(&prazo_crediario_dias) {
            return Err(format!(
                "prazo do crediário deve ficar entre 1 e {PRAZO_CREDIARIO_DIAS_LIMITE} dias"
            ));
        }
        Ok(Self {
            max_parcelas_cartao,
            prazo_crediario_dias,
        })
    }
}

/// Repositório de configuração da loja.
pub struct ConfigLojaRepo<'c> {
    conn: &'c Connection,
}

impl<'c> ConfigLojaRepo<'c> {
    pub fn novo(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Lê uma chave crua. `Ok(None)` quando a chave não existe.
    pub fn obter(&self, chave: &str) -> InfraResult<Option<String>> {
        let valor = self
            .conn
            .query_row(
                "SELECT valor FROM loja_config WHERE chave = ?1",
                [chave],
                |row| row.get(0),
            )
            .optional()?;
        Ok(valor)
    }

    /// Grava (ou atualiza) uma chave com timestamp de atualização.
    pub fn definir(&self, chave: &str, valor: &str) -> InfraResult<()> {
        self.conn.execute(
            "INSERT INTO loja_config (chave, valor, updated_at)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(chave) DO UPDATE SET
                 valor = excluded.valor,
                 updated_at = excluded.updated_at",
            params![chave, valor],
        )?;
        Ok(())
    }

    /// Fuso IANA da loja. Default `America/Sao_Paulo`.
    pub fn fuso_horario(&self) -> String {
        self.obter("fuso_horario")
            .ok()
            .flatten()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "America/Sao_Paulo".to_string())
    }

    /// Limites de pagamento efetivos.
    ///
    /// Valor ausente, ilegível ou fora da faixa cai no default. Isso protege o
    /// PDV de um banco editado à mão sem quebrar a venda.
    pub fn config_pagamento(&self) -> ConfigPagamento {
        let max_parcelas_cartao = self
            .obter("max_parcelas_cartao")
            .ok()
            .flatten()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .filter(|v| (1..=MAX_PARCELAS_CARTAO_LIMITE).contains(v))
            .unwrap_or(MAX_PARCELAS_CARTAO_PADRAO);

        let prazo_crediario_dias = self
            .obter("prazo_crediario_dias")
            .ok()
            .flatten()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .filter(|v| (1..=PRAZO_CREDIARIO_DIAS_LIMITE).contains(v))
            .unwrap_or(PRAZO_CREDIARIO_DIAS_PADRAO);

        ConfigPagamento {
            max_parcelas_cartao,
            prazo_crediario_dias,
        }
    }

    /// Grava os dois limites de pagamento. O chamador já validou os valores.
    pub fn definir_config_pagamento(&self, config: ConfigPagamento) -> InfraResult<()> {
        self.definir(
            "max_parcelas_cartao",
            &config.max_parcelas_cartao.to_string(),
        )?;
        self.definir(
            "prazo_crediario_dias",
            &config.prazo_crediario_dias.to_string(),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    #[test]
    fn config_pagamento_usa_defaults_do_schema() {
        let db = Database::em_memoria().unwrap();
        let repo = ConfigLojaRepo::novo(db.conn());

        let config = repo.config_pagamento();
        assert_eq!(config.max_parcelas_cartao, MAX_PARCELAS_CARTAO_PADRAO);
        assert_eq!(config.prazo_crediario_dias, PRAZO_CREDIARIO_DIAS_PADRAO);
    }

    #[test]
    fn config_pagamento_persiste_e_rejeita_fora_da_faixa() {
        let db = Database::em_memoria().unwrap();
        let repo = ConfigLojaRepo::novo(db.conn());

        let nova = ConfigPagamento::validar(6, 45).unwrap();
        repo.definir_config_pagamento(nova).unwrap();
        assert_eq!(repo.config_pagamento(), nova);

        assert!(ConfigPagamento::validar(0, 30).is_err());
        assert!(ConfigPagamento::validar(25, 30).is_err());
        assert!(ConfigPagamento::validar(12, 0).is_err());
        assert!(ConfigPagamento::validar(12, 366).is_err());
    }

    #[test]
    fn valor_corrompido_cai_no_default_sem_derrubar_o_pdv() {
        let db = Database::em_memoria().unwrap();
        let repo = ConfigLojaRepo::novo(db.conn());

        repo.definir("max_parcelas_cartao", "muitas").unwrap();
        repo.definir("prazo_crediario_dias", "-1").unwrap();

        let config = repo.config_pagamento();
        assert_eq!(config.max_parcelas_cartao, MAX_PARCELAS_CARTAO_PADRAO);
        assert_eq!(config.prazo_crediario_dias, PRAZO_CREDIARIO_DIAS_PADRAO);
    }
}
