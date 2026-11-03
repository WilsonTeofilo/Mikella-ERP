//! mikella-domain-people
//! Domínio de pessoas: Usuario, Papel, Capacidade.
//!
//! Regras de negócio:
//! - Papel determina capacidades (conjunto de strings).
//! - Cada papel tem um conjunto fixo de capacidades definidas no domínio.
//! - Apenas DONO pode criar GERENTE; GERENTE pode criar OPERADOR_CAIXA e VENDEDOR.
//! - Capacidades são herdadas do papel e podem ser ampliadas individualmente.
//! - Usuário inativo (desativado) não pode autenticar.
//! - PIN é opcional — usado para operações de aprovação no balcão (FRD-050).

pub mod pessoa;
pub mod ports;

pub use pessoa::{Pessoa, TipoPessoa};

use chrono::{DateTime, Utc};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::UsuarioId,
};
use serde::{Deserialize, Serialize};

/// Catálogo fechado de capacidades atômicas (FRD-055, SEG-020).
///
/// Só é possível conceder uma capacidade que exista neste catálogo. Isso
/// impede que um gerente invente strings como `"admin.total"` e amplie os
/// próprios poderes por meio de `capacidades_extras`.
pub const CAPACIDADES_CONHECIDAS: &[&str] = &[
    // Vendas
    "venda.ver",
    "venda.criar",
    "venda.cancelar",
    "venda.estorno",
    "venda.sangria",
    "venda.suprimento",
    "venda.desconto",
    // Estoque
    "estoque.entrada",
    "estoque.saida",
    "estoque.ajuste",
    "estoque.inventario",
    // Produtos
    "produto.ver",
    "produto.criar",
    "produto.atualizar",
    "produto.arquivar",
    // Caixa
    "caixa.ver",
    "caixa.abrir",
    "caixa.fechar",
    // Financeiro
    "financeiro.ver",
    "financeiro.gerenciar",
    // Pessoas
    "usuario.ver",
    "usuario.gerenciar",
    "usuario.criar.gerente",
    "usuario.criar.operador",
    "usuario.ativar",
    "usuario.desativar",
    "usuario.alterar.senha",
    "capacidade.gerenciar",
    // Sistema
    "sistema.backup",
    "sistema.diagnostico",
];

/// Retorna `true` se a capacidade pertence ao catálogo fechado.
pub fn capacidade_conhecida(capacidade: &str) -> bool {
    CAPACIDADES_CONHECIDAS.contains(&capacidade)
}

/// Papéis do sistema e suas capacidades.
///
/// Capacidades atômicas (FRD-055): cada string é uma ação que o usuário pode
/// executar. O conjunto é herdado do papel, e o DONO pode ampliar individualmente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Papel {
    /// Administrador máximo — pode tudo, exceto delegar DONO.
    Dono,
    /// Gerente — opera caixa, gerencia produtos, ajusta estoque.
    Gerente,
    /// Operador de caixa — vende, faz sangria com aprovação.
    OperadorCaixa,
    /// Estoquista — entrada/saída de estoque, inventário.
    Estoquista,
    /// Vendedor — apenas vendas com comanda.
    Vendedor,
}

impl Papel {
    /// Conjunto de capacidades associadas a cada papel (RNF-002).
    ///
    /// Capacidades granulares definem permissões finas. O DONO herda
    /// todas as capacidades do GERENTE e mais: criar gerentes, gerenciar
    /// capacidades de outros.
    pub fn capacidades(&self) -> &'static [&'static str] {
        match self {
            // O dono recebe o catálogo inteiro. `usuario.criar.gerente` só
            // existe aqui — nenhum outro papel pode criar outro gerente.
            Papel::Dono => CAPACIDADES_CONHECIDAS,
            Papel::Gerente => &[
                "venda.ver",
                "venda.criar",
                "venda.cancelar",
                "venda.estorno",
                "venda.sangria",
                "venda.suprimento",
                "venda.desconto",
                "estoque.entrada",
                "estoque.saida",
                "estoque.ajuste",
                "estoque.inventario",
                "produto.ver",
                "produto.criar",
                "produto.atualizar",
                "produto.arquivar",
                "caixa.ver",
                "caixa.abrir",
                "caixa.fechar",
                "financeiro.ver",
                "financeiro.gerenciar",
                "usuario.ver",
                "usuario.gerenciar",
                "usuario.criar.operador",
                "usuario.ativar",
                "usuario.desativar",
                "usuario.alterar.senha",
                "capacidade.gerenciar",
                "sistema.backup",
                "sistema.diagnostico",
            ],
            Papel::OperadorCaixa => &[
                "venda.ver",
                "venda.criar",
                "venda.cancelar",
                "venda.sangria",
                "produto.ver",
                "produto.atualizar",
                "caixa.ver",
                "caixa.abrir",
                "caixa.fechar",
                "financeiro.ver",
                "financeiro.gerenciar",
            ],
            Papel::Estoquista => &[
                "estoque.entrada",
                "estoque.saida",
                "estoque.ajuste",
                "estoque.inventario",
                "produto.ver",
                "produto.atualizar",
            ],
            Papel::Vendedor => &["venda.ver", "venda.criar", "produto.ver"],
        }
    }

    /// Nível hierárquico. Só é permitido gerenciar papéis de nível
    /// **estritamente inferior** (RF-001, SEG-021).
    pub fn nivel(&self) -> u8 {
        match self {
            Papel::Dono => 3,
            Papel::Gerente => 2,
            Papel::OperadorCaixa | Papel::Estoquista | Papel::Vendedor => 1,
        }
    }

    /// `true` quando `self` pode administrar `alvo` (criar, editar,
    /// desativar ou alterar capacidades).
    pub fn pode_gerenciar(&self, alvo: &Papel) -> bool {
        self.nivel() > alvo.nivel()
    }

    /// Verifica se este papel pode criar o papel alvo.
    pub fn pode_criar(&self, alvo: &Papel) -> bool {
        match (self, alvo) {
            (Papel::Dono, _) => true,
            (Papel::Gerente, Papel::OperadorCaixa) => true,
            (Papel::Gerente, Papel::Vendedor) => true,
            (Papel::Gerente, _) => false,
            (_, _) => false,
        }
    }

    /// Converte de string para papel (validação DB).
    pub fn from_db(valor: &str) -> DomainResult<Self> {
        match valor {
            "DONO" => Ok(Papel::Dono),
            "GERENTE" => Ok(Papel::Gerente),
            "OPERADOR_CAIXA" => Ok(Papel::OperadorCaixa),
            "ESTOQUISTA" => Ok(Papel::Estoquista),
            "VENDEDOR" => Ok(Papel::Vendedor),
            _ => Err(DomainError::Invalido(format!(
                "Papel desconhecido: {}",
                valor
            ))),
        }
    }

    /// Converte para string para armazenamento no DB.
    pub fn as_db(&self) -> &'static str {
        match self {
            Papel::Dono => "DONO",
            Papel::Gerente => "GERENTE",
            Papel::OperadorCaixa => "OPERADOR_CAIXA",
            Papel::Estoquista => "ESTOQUISTA",
            Papel::Vendedor => "VENDEDOR",
        }
    }
}

/// Representa o PIN de um usuário (armazenado como hash, nunca em claro).
///
/// O PIN é opcional — é usado para aprovação de operações sensíveis no balcão
/// (FRD-050: cancelamento, estorno, sangria, ajuste de estoque, fechamento).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinHash(pub String);

/// Entidade Usuario do domínio.
///
/// Representa um usuário do sistema com papel e capacidades.
/// Regras:
/// - Username é único (insensível a maiúsculas/minúsculas).
/// - Email é opcional (usado para 2FA e notificações — REC-002).
/// - Papel determina capacidades iniciais.
/// - Capacidades extras podem ser adicionadas individualmente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usuario {
    pub id: UsuarioId,
    pub nome: String,
    pub username: String,
    pub email: Option<String>,
    pub senha_hash: String, // Argon2id
    pub papel: Papel,
    pub pin_hash: Option<String>,        // Argon2id do PIN (se houver)
    pub capacidades_extras: Vec<String>, // capacidades além do papel
    pub ativo: bool,
    pub criado_em: DateTime<Utc>,
    pub atualizado_em: DateTime<Utc>,
}

impl Usuario {
    /// Cria um novo usuário.
    ///
    /// Regras de validação:
    /// - Nome não pode ser vazio.
    /// - Username: 3-64 caracteres, alfanumérico + underscore.
    /// - Senha hash não pode ser vazio.
    /// - Email, se fornecido, deve estar em formato básico válido.
    pub fn novo(
        id: UsuarioId,
        nome: String,
        username: String,
        senha_hash: String,
        papel: Papel,
        email: Option<String>,
        agora: DateTime<Utc>,
    ) -> DomainResult<Self> {
        // Validação de nome
        let nome_trim = nome.trim();
        if nome_trim.is_empty() {
            return Err(DomainError::Invalido("Nome não pode ser vazio".into()));
        }
        let nome = nome_trim.to_string();

        // Validação de username
        let username_trim = username.trim();
        if username_trim.len() < 3 || username_trim.len() > 64 {
            return Err(DomainError::Invalido(
                "Username deve ter entre 3 e 64 caracteres".into(),
            ));
        }
        if !username_trim
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_')
        {
            return Err(DomainError::Invalido(
                "Username deve conter apenas letras, números e underscore".into(),
            ));
        }
        let username = username_trim.to_string();

        // Validação de senha_hash
        if senha_hash.trim().is_empty() {
            return Err(DomainError::Invalido(
                "Senha hash não pode ser vazia".into(),
            ));
        }

        // Validação de email (ótimo apenas formato básico)
        if let Some(email_ref) = &email {
            if email_ref.trim().is_empty() {
                return Err(DomainError::Invalido("Email não pode ser vazio".into()));
            }
            if !email_ref.contains('@') || !email_ref.contains('.') {
                return Err(DomainError::Invalido("Email em formato inválido".into()));
            }
        }

        Ok(Self {
            id,
            nome,
            username,
            email,
            senha_hash,
            papel,
            pin_hash: None,
            capacidades_extras: Vec::new(),
            ativo: true,
            criado_em: agora,
            atualizado_em: agora,
        })
    }

    /// Atualiza o nome do usuário.
    pub fn atualizar_nome(&mut self, novo_nome: String, agora: DateTime<Utc>) -> DomainResult<()> {
        let nome_trim = novo_nome.trim();
        if nome_trim.is_empty() {
            return Err(DomainError::Invalido("Nome não pode ser vazio".into()));
        }
        self.nome = nome_trim.to_string();
        self.atualizado_em = agora;
        Ok(())
    }

    /// Atualiza o email.
    pub fn atualizar_email(
        &mut self,
        novo_email: Option<String>,
        agora: DateTime<Utc>,
    ) -> DomainResult<()> {
        if let Some(email_ref) = &novo_email {
            if email_ref.trim().is_empty() {
                return Err(DomainError::Invalido("Email não pode ser vazio".into()));
            }
            if !email_ref.contains('@') || !email_ref.contains('.') {
                return Err(DomainError::Invalido("Email em formato inválido".into()));
            }
        }
        self.email = novo_email;
        self.atualizado_em = agora;
        Ok(())
    }

    /// Define o PIN do usuário (hash Argon2id).
    pub fn definir_pin(&mut self, pin_hash: String, agora: DateTime<Utc>) {
        self.pin_hash = Some(pin_hash);
        self.atualizado_em = agora;
    }

    /// Remove o PIN do usuário.
    pub fn remover_pin(&mut self, agora: DateTime<Utc>) {
        self.pin_hash = None;
        self.atualizado_em = agora;
    }

    /// Adiciona uma capacidade extra ao usuário.
    ///
    /// Rejeita capacidades fora do catálogo fechado (SEG-020). Sem essa
    /// validação, uma string inventada poderia virar uma permissão real em
    /// qualquer verificação futura que comparasse strings livremente.
    pub fn adicionar_capacidade(
        &mut self,
        capacidade: String,
        agora: DateTime<Utc>,
    ) -> DomainResult<()> {
        if !capacidade_conhecida(&capacidade) {
            return Err(DomainError::Invalido(format!(
                "Capacidade desconhecida: {capacidade}"
            )));
        }
        if !self.capacidades_extras.contains(&capacidade) {
            self.capacidades_extras.push(capacidade);
            self.atualizado_em = agora;
        }
        Ok(())
    }

    /// Remove uma capacidade extra do usuário.
    pub fn remover_capacidade(
        &mut self,
        capacidade: &str,
        agora: DateTime<Utc>,
    ) -> DomainResult<()> {
        if let Some(pos) = self.capacidades_extras.iter().position(|c| c == capacidade) {
            self.capacidades_extras.remove(pos);
            self.atualizado_em = agora;
        }
        Ok(())
    }

    /// Lista todas as capacidades do usuário (herdadas do papel + extras).
    pub fn capacidades(&self) -> Vec<String> {
        let mut caps: Vec<String> = self
            .papel
            .capacidades()
            .iter()
            .map(|s| s.to_string())
            .collect();
        for extra in &self.capacidades_extras {
            if !caps.contains(extra) {
                caps.push(extra.clone());
            }
        }
        caps
    }

    /// Verifica se o usuário possui uma capacidade.
    pub fn tem_capacidade(&self, capacidade: &str) -> bool {
        self.papel.capacidades().contains(&capacidade)
            || self.capacidades_extras.iter().any(|c| c == capacidade)
    }

    /// Desativa o usuário (soft-delete — preserve histórico).
    pub fn desativar(&mut self, agora: DateTime<Utc>) {
        self.ativo = false;
        self.atualizado_em = agora;
    }

    /// Reativa o usuário.
    pub fn ativar(&mut self, agora: DateTime<Utc>) {
        self.ativo = true;
        self.atualizado_em = agora;
    }

    /// Atualiza a senha (hash Argon2id).
    pub fn atualizar_senha(&mut self, novo_hash: String, agora: DateTime<Utc>) {
        self.senha_hash = novo_hash;
        self.atualizado_em = agora;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn novo_usuario_valido() {
        let u = Usuario::novo(
            UsuarioId::new(),
            "João Silva".into(),
            "joao".into(),
            "$argon2id$...".into(),
            Papel::OperadorCaixa,
            None,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(u.nome, "João Silva");
        assert_eq!(u.username, "joao");
        assert!(u.ativo);
        assert!(u.email.is_none());
    }

    #[test]
    fn nome_vazio_falha() {
        let r = Usuario::novo(
            UsuarioId::new(),
            "   ".into(),
            "joao".into(),
            "hash".into(),
            Papel::OperadorCaixa,
            None,
            Utc::now(),
        );
        assert!(r.is_err());
    }

    #[test]
    fn username_curto_falha() {
        let r = Usuario::novo(
            UsuarioId::new(),
            "João".into(),
            "ab".into(),
            "hash".into(),
            Papel::OperadorCaixa,
            None,
            Utc::now(),
        );
        assert!(r.is_err());
    }

    #[test]
    fn username_caracteres_invalidos_falha() {
        let r = Usuario::novo(
            UsuarioId::new(),
            "João".into(),
            "joao@admin".into(),
            "hash".into(),
            Papel::OperadorCaixa,
            None,
            Utc::now(),
        );
        assert!(r.is_err());
    }

    #[test]
    fn email_invalido_falha() {
        let r = Usuario::novo(
            UsuarioId::new(),
            "João".into(),
            "joao".into(),
            "hash".into(),
            Papel::OperadorCaixa,
            Some("email-invalido".into()),
            Utc::now(),
        );
        assert!(r.is_err());
    }

    #[test]
    fn senha_hash_vazia_falha() {
        let r = Usuario::novo(
            UsuarioId::new(),
            "João".into(),
            "joao".into(),
            "".into(),
            Papel::OperadorCaixa,
            None,
            Utc::now(),
        );
        assert!(r.is_err());
    }

    #[test]
    fn papel_dono_pode_criar_qualquer_papel() {
        assert!(Papel::Dono.pode_criar(&Papel::Gerente));
        assert!(Papel::Dono.pode_criar(&Papel::OperadorCaixa));
        assert!(Papel::Dono.pode_criar(&Papel::Vendedor));
        assert!(Papel::Dono.pode_criar(&Papel::Estoquista));
    }

    #[test]
    fn papel_gerente_pode_criar_operador_e_vendedor() {
        assert!(Papel::Gerente.pode_criar(&Papel::OperadorCaixa));
        assert!(Papel::Gerente.pode_criar(&Papel::Vendedor));
        assert!(!Papel::Gerente.pode_criar(&Papel::Dono));
        assert!(!Papel::Gerente.pode_criar(&Papel::Gerente));
    }

    #[test]
    fn papel_operador_nao_pode_criar_ninguem() {
        assert!(!Papel::OperadorCaixa.pode_criar(&Papel::Vendedor));
    }

    #[test]
    fn capacidades_herdadas_do_papel() {
        let u = Usuario::novo(
            UsuarioId::new(),
            "João".into(),
            "joao".into(),
            "hash".into(),
            Papel::OperadorCaixa,
            None,
            Utc::now(),
        )
        .unwrap();
        assert!(u.tem_capacidade("venda.criar"));
        assert!(u.tem_capacidade("caixa.abrir"));
        assert!(!u.tem_capacidade("produto.arquivar"));
    }

    #[test]
    fn capacidades_extras_somam_ao_papel() {
        let mut u = Usuario::novo(
            UsuarioId::new(),
            "João".into(),
            "joao".into(),
            "hash".into(),
            Papel::Vendedor,
            None,
            Utc::now(),
        )
        .unwrap();
        u.adicionar_capacidade("produto.atualizar".into(), Utc::now())
            .unwrap();
        assert!(u.tem_capacidade("venda.criar"));
        assert!(u.tem_capacidade("produto.atualizar"));
    }

    #[test]
    fn capacidade_desconhecida_rejeitada() {
        let mut u = Usuario::novo(
            UsuarioId::new(),
            "João".into(),
            "joao".into(),
            "hash".into(),
            Papel::Vendedor,
            None,
            Utc::now(),
        )
        .unwrap();
        assert!(u
            .adicionar_capacidade("admin.total".into(), Utc::now())
            .is_err());
    }

    #[test]
    fn hierarquia_e_estrita() {
        assert!(Papel::Dono.pode_gerenciar(&Papel::Gerente));
        assert!(Papel::Dono.pode_gerenciar(&Papel::Vendedor));
        assert!(Papel::Gerente.pode_gerenciar(&Papel::Vendedor));
        assert!(!Papel::Gerente.pode_gerenciar(&Papel::Dono));
        assert!(!Papel::Gerente.pode_gerenciar(&Papel::Gerente));
        assert!(!Papel::Vendedor.pode_gerenciar(&Papel::Vendedor));
    }

    #[test]
    fn catalogo_de_capacidades_nao_tem_duplicatas() {
        for (i, cap) in CAPACIDADES_CONHECIDAS.iter().enumerate() {
            assert!(!cap.is_empty());
            assert!(!CAPACIDADES_CONHECIDAS[..i].contains(cap), "duplicata: {cap}");
        }
    }

    #[test]
    fn from_db_roundtrip() {
        let p = Papel::from_db("DONO").unwrap();
        assert_eq!(p.as_db(), "DONO");
        let p = Papel::from_db("VENDEDOR").unwrap();
        assert_eq!(p.as_db(), "VENDEDOR");
        assert!(Papel::from_db("INVALIDO").is_err());
    }

    #[test]
    fn desativar_ativar() {
        let mut u = Usuario::novo(
            UsuarioId::new(),
            "João".into(),
            "joao".into(),
            "hash".into(),
            Papel::OperadorCaixa,
            None,
            Utc::now(),
        )
        .unwrap();
        assert!(u.ativo);
        u.desativar(Utc::now());
        assert!(!u.ativo);
        u.ativar(Utc::now());
        assert!(u.ativo);
    }
}
