//! Entidade Pessoa (Cliente, Fornecedor, Transportadora).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use mikella_shared_kernel::{
    documento::{Cpf, Cnpj},
    error::{DomainError, DomainResult},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TipoPessoa {
    Cliente,
    Fornecedor,
    Transportadora,
}

impl TipoPessoa {
    pub fn as_db(&self) -> &'static str {
        match self {
            Self::Cliente => "CLIENTE",
            Self::Fornecedor => "FORNECEDOR",
            Self::Transportadora => "TRANSPORTADORA",
        }
    }

    pub fn from_db(s: &str) -> DomainResult<Self> {
        match s.to_ascii_uppercase().as_str() {
            "CLIENTE" => Ok(Self::Cliente),
            "FORNECEDOR" => Ok(Self::Fornecedor),
            "TRANSPORTADORA" => Ok(Self::Transportadora),
            outro => Err(DomainError::Invalido(format!("Tipo de pessoa inválido: {outro}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pessoa {
    pub id: String,
    pub tipo: TipoPessoa,
    pub nome: String,
    pub telefone: Option<String>,
    pub email: Option<String>,
    pub documento: Option<String>,
    pub endereco: Option<String>,
    pub ativo: bool,
    pub criado_em: DateTime<Utc>,
    pub atualizado_em: DateTime<Utc>,
}

impl Pessoa {
    pub fn novo_cliente(
        id: String,
        nome: String,
        telefone: Option<String>,
        email: Option<String>,
        documento: Option<String>,
        endereco: Option<String>,
        agora: DateTime<Utc>,
    ) -> DomainResult<Self> {
        let nome_limpo = nome.trim().to_string();
        if nome_limpo.is_empty() {
            return Err(DomainError::Invalido("Nome do cliente é obrigatório".into()));
        }

        // Se documento foi preenchido, valida se é CPF ou CNPJ
        let doc_limpo = if let Some(doc) = documento.filter(|d| !d.trim().is_empty()) {
            let digits: String = doc.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() == 11 {
                Cpf::parse(&digits)
                    .map_err(|_| DomainError::Invalido("CPF informado é inválido".into()))?;
            } else if digits.len() == 14 {
                Cnpj::parse(&digits)
                    .map_err(|_| DomainError::Invalido("CNPJ informado é inválido".into()))?;
            } else {
                return Err(DomainError::Invalido(
                    "Documento deve conter 11 dígitos (CPF) ou 14 dígitos (CNPJ)".into(),
                ));
            }
            Some(digits)
        } else {
            None
        };

        let tel_limpo = if let Some(tel) = telefone.filter(|t| !t.trim().is_empty()) {
            let digits: String = tel.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() != 10 && digits.len() != 11 {
                return Err(DomainError::Invalido(
                    "Telefone deve ter DDD + 8 ou 9 dígitos".into(),
                ));
            }
            Some(digits)
        } else {
            None
        };

        let email_limpo = if let Some(email) = email.filter(|e| !e.trim().is_empty()) {
            let e = email.trim().to_string();
            if !e.contains('@') || e.starts_with('@') || e.ends_with('@') {
                return Err(DomainError::Invalido("E-mail informado é inválido".into()));
            }
            Some(e)
        } else {
            None
        };

        Ok(Self {
            id,
            tipo: TipoPessoa::Cliente,
            nome: nome_limpo,
            telefone: tel_limpo,
            email: email_limpo,
            documento: doc_limpo,
            endereco: endereco.map(|e| e.trim().to_string()).filter(|e| !e.is_empty()),
            ativo: true,
            criado_em: agora,
            atualizado_em: agora,
        })
    }
}
