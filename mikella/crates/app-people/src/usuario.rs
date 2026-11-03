//! Casos de uso de gerenciamento de usuários.

use chrono::{DateTime, Utc};
use mikella_domain_people::ports::UsuarioRepository;
use mikella_domain_people::{Papel, Usuario};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::UsuarioId,
};
use serde::Deserialize;

/// Portas necessárias para os casos de uso de pessoa.
pub struct UsuarioRepos<'a> {
    pub usuarios: &'a dyn UsuarioRepository,
}

/// Comando para criação de usuário.
#[derive(Debug, Deserialize)]
pub struct CriarUsuarioCmd {
    pub usuario_id: UsuarioId,
    pub nome: String,
    pub username: String,
    pub senha_hash: String,
    pub papel: String, // serializado como string do DB
    pub email: Option<String>,
    /// Capacidades extras além do papel (apenas DONO/GERENTE pode usar).
    #[serde(default)]
    pub capacidades_extras: Vec<String>,
}

/// Comando para atualização de usuário (campos mutáveis).
#[derive(Debug, Deserialize)]
pub struct AtualizarUsuarioCmd {
    pub usuario_id: UsuarioId,
    pub nome: String,
    pub email: Option<String>,
}

/// Comando para desativação de usuário.
#[derive(Debug, Deserialize)]
pub struct DesativarUsuarioCmd {
    pub usuario_id: UsuarioId,
}

/// Query para listagem de usuários.
#[derive(Debug, Deserialize)]
pub struct ListaUsuarioQuery {
    /// Se true, lista apenas ativos. Se false, lista todos.
    pub ativos: bool,
}

/// Exige uma capacidade do ator (SEG-020: negar por padrão).
fn exigir_capacidade(ator: &Usuario, capacidade: &str) -> DomainResult<()> {
    if ator.tem_capacidade(capacidade) {
        Ok(())
    } else {
        Err(DomainError::PermissaoNegada(format!(
            "Capacidade '{capacidade}' não concedida"
        )))
    }
}

/// Exige hierarquia estrita: o ator só administra papéis abaixo do seu
/// (RF-001, SEG-021). Ninguém altera o dono nem a si mesmo por este caminho.
fn exigir_hierarquia(ator: &Usuario, alvo: &Usuario) -> DomainResult<()> {
    if ator.papel.pode_gerenciar(&alvo.papel) {
        Ok(())
    } else {
        Err(DomainError::PermissaoNegada(format!(
            "Papel '{}' não pode administrar papel '{}'",
            ator.papel.as_db(),
            alvo.papel.as_db()
        )))
    }
}

fn carregar_alvo(repos: &UsuarioRepos<'_>, id: UsuarioId) -> DomainResult<Usuario> {
    repos
        .usuarios
        .buscar_por_id(id)?
        .ok_or_else(|| DomainError::NaoEncontrado {
            entidade: "Usuario",
            id: id.to_string(),
        })
}

/// Cria um novo usuário.
///
/// Regras:
/// - Username deve ser único (case-insensitive).
/// - Papel é validado contra o enum dominio.
/// - O ator precisa de `usuario.gerenciar` e só cria papéis que seu papel
///   permite (`Papel::pode_criar`).
/// - Capacidades extras só podem ser concedidas se o ator já as possui.
pub fn criar_usuario(
    repos: &UsuarioRepos<'_>,
    cmd: CriarUsuarioCmd,
    criador: &Usuario,
) -> DomainResult<Usuario> {
    exigir_capacidade(criador, "usuario.gerenciar")?;

    // Valida unicidade de username (o repositório usa NOCASE).
    if repos.usuarios.buscar_por_username(&cmd.username)?.is_some() {
        return Err(DomainError::EstadoInvalido(format!(
            "Username '{}' já cadastrado",
            cmd.username
        )));
    }

    let papel = Papel::from_db(&cmd.papel)?;

    if !criador.papel.pode_criar(&papel) {
        return Err(DomainError::PermissaoNegada(format!(
            "Papel '{}' não pode criar usuário com papel '{}'",
            criador.papel.as_db(),
            cmd.papel
        )));
    }

    let agora = Utc::now();

    let mut usuario = Usuario::novo(
        cmd.usuario_id,
        cmd.nome,
        cmd.username,
        cmd.senha_hash,
        papel,
        cmd.email,
        agora,
    )?;

    for cap in cmd.capacidades_extras {
        if !criador.tem_capacidade(&cap) {
            return Err(DomainError::PermissaoNegada(format!(
                "Ator não pode conceder a capacidade '{cap}'"
            )));
        }
        usuario.adicionar_capacidade(cap, agora)?;
    }

    repos.usuarios.salvar(&usuario)?;

    tracing::info!(
        usuario_id = %usuario.id,
        username = %usuario.username,
        papel = %usuario.papel.as_db(),
        "usuário criado"
    );

    Ok(usuario)
}

/// Atualiza nome e email de um usuário.
///
/// O próprio usuário pode editar o próprio perfil; para editar terceiros é
/// preciso `usuario.gerenciar` e hierarquia estrita.
pub fn atualizar_usuario(
    repos: &UsuarioRepos<'_>,
    cmd: AtualizarUsuarioCmd,
    ator: &Usuario,
) -> DomainResult<Usuario> {
    let mut usuario = carregar_alvo(repos, cmd.usuario_id)?;

    if ator.id != usuario.id {
        exigir_capacidade(ator, "usuario.gerenciar")?;
        exigir_hierarquia(ator, &usuario)?;
    }

    if !usuario.ativo {
        return Err(DomainError::EstadoInvalido(
            "Usuário inativo — não pode ser atualizado".into(),
        ));
    }

    let agora = Utc::now();
    usuario.atualizar_nome(cmd.nome, agora)?;
    usuario.atualizar_email(cmd.email, agora)?;

    repos.usuarios.atualizar(&usuario)?;

    tracing::info!(usuario_id = %usuario.id, "usuário atualizado");

    Ok(usuario)
}

/// Desativa um usuário (soft-delete).
///
/// Nunca remove dados — preserva histórico de auditoria. Ninguém desativa a
/// si mesmo (evita lockout acidental) e o dono nunca é desativado.
pub fn desativar_usuario(
    repos: &UsuarioRepos<'_>,
    cmd: DesativarUsuarioCmd,
    ator: &Usuario,
) -> DomainResult<()> {
    exigir_capacidade(ator, "usuario.desativar")?;

    let mut usuario = carregar_alvo(repos, cmd.usuario_id)?;

    if ator.id == usuario.id {
        return Err(DomainError::PermissaoNegada(
            "Usuário não pode desativar a própria conta".into(),
        ));
    }
    exigir_hierarquia(ator, &usuario)?;

    if !usuario.ativo {
        return Err(DomainError::EstadoInvalido("Usuário já está inativo".into()));
    }

    let agora = Utc::now();
    usuario.desativar(agora);
    repos.usuarios.atualizar(&usuario)?;

    tracing::info!(usuario_id = %usuario.id, "usuário desativado");

    Ok(())
}

/// Lista usuários conforme a query.
/// Exige `usuario.ver` (DONO e GERENTE).
pub fn listar_usuarios(
    repos: &UsuarioRepos<'_>,
    query: ListaUsuarioQuery,
    ator: &Usuario,
) -> DomainResult<Vec<Usuario>> {
    exigir_capacidade(ator, "usuario.ver")?;
    if query.ativos {
        repos.usuarios.listar_ativos()
    } else {
        repos.usuarios.listar_todos()
    }
}

/// Atualiza a senha de um usuário.
///
/// O próprio usuário pode trocar a própria senha. Para trocar a de terceiros
/// é preciso `usuario.alterar.senha` e hierarquia estrita.
/// SEG-008: o handler revoga as sessões do alvo depois desta operação.
pub fn atualizar_senha(
    repos: &UsuarioRepos<'_>,
    usuario_id: UsuarioId,
    nova_senha_hash: String,
    agora: DateTime<Utc>,
    ator: &Usuario,
) -> DomainResult<()> {
    let mut usuario = carregar_alvo(repos, usuario_id)?;

    if ator.id != usuario.id {
        exigir_capacidade(ator, "usuario.alterar.senha")?;
        exigir_hierarquia(ator, &usuario)?;
    }

    if !usuario.ativo {
        return Err(DomainError::EstadoInvalido(
            "Usuário inativo — não pode atualizar senha".into(),
        ));
    }

    usuario.atualizar_senha(nova_senha_hash, agora);
    repos.usuarios.atualizar(&usuario)?;

    tracing::info!(usuario_id = %usuario.id, "senha atualizada");
    Ok(())
}

/// Gerencia capacidades extras de um usuário (substitui todas as extras).
///
/// Exige `capacidade.gerenciar`, hierarquia estrita e só aceita capacidades
/// do catálogo que o próprio ator possui (impede autoelevação e delegação de
/// poderes que o ator não tem).
pub fn gerenciar_capacidades(
    repos: &UsuarioRepos<'_>,
    usuario_id: UsuarioId,
    capacidades: Vec<String>,
    ator: &Usuario,
    agora: DateTime<Utc>,
) -> DomainResult<Vec<String>> {
    exigir_capacidade(ator, "capacidade.gerenciar")?;

    let mut usuario = carregar_alvo(repos, usuario_id)?;
    exigir_hierarquia(ator, &usuario)?;

    if !usuario.ativo {
        return Err(DomainError::EstadoInvalido(
            "Usuário inativo — não pode gerenciar capacidades".into(),
        ));
    }

    for cap in &capacidades {
        if !ator.tem_capacidade(cap) {
            return Err(DomainError::PermissaoNegada(format!(
                "Ator não pode conceder a capacidade '{cap}'"
            )));
        }
    }

    for cap in usuario.capacidades_extras.clone() {
        usuario.remover_capacidade(&cap, agora)?;
    }
    for cap in capacidades {
        usuario.adicionar_capacidade(cap, agora)?;
    }

    repos.usuarios.atualizar(&usuario)?;

    tracing::info!(usuario_id = %usuario.id, "capacidades atualizadas");
    Ok(usuario.capacidades())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock do repositório para testes de use case.
    struct MockUsuarioRepo<'a> {
        usuarios: Vec<Usuario>,
        _marker: std::marker::PhantomData<&'a ()>,
    }

    impl<'a> MockUsuarioRepo<'a> {
        fn new() -> Self {
            Self {
                usuarios: Vec::new(),
                _marker: std::marker::PhantomData,
            }
        }
    }

    fn ator(papel: Papel) -> Usuario {
        Usuario::novo(
            UsuarioId::new(),
            "Ator".into(),
            "ator".into(),
            "$argon2id$hash".into(),
            papel,
            None,
            Utc::now(),
        )
        .unwrap()
    }

    impl<'a> UsuarioRepository for MockUsuarioRepo<'a> {
        fn salvar(&self, _usuario: &Usuario) -> DomainResult<()> {
            Ok(())
        }
        fn atualizar(&self, _usuario: &Usuario) -> DomainResult<()> {
            Ok(())
        }
        fn buscar_por_id(&self, id: UsuarioId) -> DomainResult<Option<Usuario>> {
            Ok(self.usuarios.iter().find(|u| u.id == id).cloned())
        }
        fn buscar_por_username(&self, username: &str) -> DomainResult<Option<Usuario>> {
            Ok(self
                .usuarios
                .iter()
                .find(|u| u.username == username)
                .cloned())
        }
        fn listar_ativos(&self) -> DomainResult<Vec<Usuario>> {
            Ok(self.usuarios.iter().filter(|u| u.ativo).cloned().collect())
        }
        fn listar_todos(&self) -> DomainResult<Vec<Usuario>> {
            Ok(self.usuarios.clone())
        }
    }

    #[test]
    fn dono_cria_gerente_sucesso() {
        let repo = MockUsuarioRepo::new();
        let repos = UsuarioRepos { usuarios: &repo };

        let cmd = CriarUsuarioCmd {
            usuario_id: UsuarioId::new(),
            nome: "Maria".to_string(),
            username: "maria".to_string(),
            senha_hash: "$argon2id$...".to_string(),
            papel: "GERENTE".to_string(),
            email: Some("maria@example.com".to_string()),
            capacidades_extras: vec![],
        };

        let result = criar_usuario(&repos, cmd, &ator(Papel::Dono));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().papel, Papel::Gerente);
    }

    #[test]
    fn gerente_nao_pode_criar_dono() {
        let repo = MockUsuarioRepo::new();
        let repos = UsuarioRepos { usuarios: &repo };

        let cmd = CriarUsuarioCmd {
            usuario_id: UsuarioId::new(),
            nome: "João".to_string(),
            username: "joao".to_string(),
            senha_hash: "$argon2id$...".to_string(),
            papel: "DONO".to_string(),
            email: None,
            capacidades_extras: vec![],
        };

        let result = criar_usuario(&repos, cmd, &ator(Papel::Gerente));
        assert!(matches!(result, Err(DomainError::PermissaoNegada(_))));
    }

    #[test]
    fn operador_nao_pode_criar_usuario() {
        let repo = MockUsuarioRepo::new();
        let repos = UsuarioRepos { usuarios: &repo };

        let cmd = CriarUsuarioCmd {
            usuario_id: UsuarioId::new(),
            nome: "João".to_string(),
            username: "joao".to_string(),
            senha_hash: "$argon2id$...".to_string(),
            papel: "VENDEDOR".to_string(),
            email: None,
            capacidades_extras: vec![],
        };

        let result = criar_usuario(&repos, cmd, &ator(Papel::OperadorCaixa));
        assert!(matches!(result, Err(DomainError::PermissaoNegada(_))));
    }

    #[test]
    fn username_duplicado_falha() {
        let mut repo = MockUsuarioRepo::new();
        let usuario_existente = Usuario::novo(
            UsuarioId::new(),
            "Maria".into(),
            "maria".into(),
            "hash".into(),
            Papel::Gerente,
            None,
            Utc::now(),
        )
        .unwrap();
        repo.usuarios.push(usuario_existente);

        let repos = UsuarioRepos { usuarios: &repo };

        let cmd = CriarUsuarioCmd {
            usuario_id: UsuarioId::new(),
            nome: "Outro".to_string(),
            username: "maria".to_string(),
            senha_hash: "$argon2id$...".to_string(),
            papel: "VENDEDOR".to_string(),
            email: None,
            capacidades_extras: vec![],
        };

        let result = criar_usuario(&repos, cmd, &ator(Papel::Dono));
        assert!(result.is_err());
    }

    #[test]
    fn papel_invalido_falha() {
        let repo = MockUsuarioRepo::new();
        let repos = UsuarioRepos { usuarios: &repo };

        let cmd = CriarUsuarioCmd {
            usuario_id: UsuarioId::new(),
            nome: "Teste".to_string(),
            username: "teste".to_string(),
            senha_hash: "$argon2id$...".to_string(),
            papel: "PAPEL_INVALIDO".to_string(),
            email: None,
            capacidades_extras: vec![],
        };

        let result = criar_usuario(&repos, cmd, &ator(Papel::Dono));
        assert!(result.is_err());
    }

    #[test]
    fn atualizar_senha_sucesso() {
        // Setup: create a user with existing senha_hash
        let mut repo = MockUsuarioRepo::new();
        let usuario_id = UsuarioId::new();
        let usuario = Usuario::novo(
            usuario_id,
            "João".into(),
            "joao".into(),
            "$argon2id$oldhash".into(),
            Papel::Vendedor,
            None,
            Utc::now(),
        )
        .unwrap();
        repo.usuarios.push(usuario);

        let repos = UsuarioRepos { usuarios: &repo };
        let nova_senha = "nova_senha_segura_123".to_string();
        let agora = Utc::now();
        let mut self_ator = ator(Papel::Vendedor);
        self_ator.id = usuario_id;

        let result = atualizar_senha(&repos, usuario_id, nova_senha, agora, &self_ator);
        assert!(result.is_ok());
    }

    #[test]
    fn atualizar_senha_usuario_inexistente_falha() {
        let repo = MockUsuarioRepo::new();
        let repos = UsuarioRepos { usuarios: &repo };
        let usuario_id = UsuarioId::new();

        let result = atualizar_senha(
            &repos,
            usuario_id,
            "nova_senha_segura_123".to_string(),
            Utc::now(),
            &ator(Papel::Dono),
        );

        assert!(matches!(result, Err(DomainError::NaoEncontrado { .. })));
    }

    #[test]
    fn gerenciar_capacidades_dono_sucesso() {
        let mut repo = MockUsuarioRepo::new();
        let usuario_id = UsuarioId::new();
        let usuario = Usuario::novo(
            usuario_id,
            "João".into(),
            "joao".into(),
            "$argon2id$hash".into(),
            Papel::Vendedor,
            None,
            Utc::now(),
        )
        .unwrap();
        repo.usuarios.push(usuario);

        let repos = UsuarioRepos { usuarios: &repo };
        let novas_capacidades = vec!["financeiro.ver".to_string(), "produto.criar".to_string()];

        let result = gerenciar_capacidades(
            &repos,
            usuario_id,
            novas_capacidades,
            &ator(Papel::Dono),
            Utc::now(),
        );

        assert!(result.is_ok());
        let caps = result.unwrap();
        assert!(caps.contains(&"financeiro.ver".to_string()));
    }

    #[test]
    fn gerenciar_capacidades_nao_dono_falha() {
        let mut repo = MockUsuarioRepo::new();
        let usuario_id = UsuarioId::new();
        let usuario = Usuario::novo(
            usuario_id,
            "João".into(),
            "joao".into(),
            "$argon2id$hash".into(),
            Papel::Vendedor,
            None,
            Utc::now(),
        )
        .unwrap();
        repo.usuarios.push(usuario);

        let repos = UsuarioRepos { usuarios: &repo };
        let novas_capacidades = vec!["financeiro.ver".to_string()];

        let result = gerenciar_capacidades(
            &repos,
            usuario_id,
            novas_capacidades,
            &ator(Papel::Vendedor),
            Utc::now(),
        );

        assert!(matches!(result, Err(DomainError::PermissaoNegada(_))));
    }

    #[test]
    fn gerenciar_capacidades_usuario_inexistente_falha() {
        let repo = MockUsuarioRepo::new();
        let repos = UsuarioRepos { usuarios: &repo };

        let result = gerenciar_capacidades(
            &repos,
            UsuarioId::new(),
            vec!["financeiro.ver".to_string()],
            &ator(Papel::Dono),
            Utc::now(),
        );

        assert!(matches!(result, Err(DomainError::NaoEncontrado { .. })));
    }

    #[test]
    fn gerente_nao_atualiza_dono() {
        let mut repo = MockUsuarioRepo::new();
        let dono = Usuario::novo(
            UsuarioId::new(),
            "Dono".into(),
            "dono".into(),
            "$argon2id$hash".into(),
            Papel::Dono,
            None,
            Utc::now(),
        )
        .unwrap();
        let dono_id = dono.id;
        repo.usuarios.push(dono);
        let repos = UsuarioRepos { usuarios: &repo };

        let result = atualizar_usuario(
            &repos,
            AtualizarUsuarioCmd {
                usuario_id: dono_id,
                nome: "Invadido".into(),
                email: None,
            },
            &ator(Papel::Gerente),
        );
        assert!(matches!(result, Err(DomainError::PermissaoNegada(_))));
    }

    #[test]
    fn gerente_nao_desativa_outro_gerente() {
        let mut repo = MockUsuarioRepo::new();
        let gerente = Usuario::novo(
            UsuarioId::new(),
            "Gerente".into(),
            "gerente".into(),
            "$argon2id$hash".into(),
            Papel::Gerente,
            None,
            Utc::now(),
        )
        .unwrap();
        let gerente_id = gerente.id;
        repo.usuarios.push(gerente);
        let repos = UsuarioRepos { usuarios: &repo };

        let result = desativar_usuario(
            &repos,
            DesativarUsuarioCmd {
                usuario_id: gerente_id,
            },
            &ator(Papel::Gerente),
        );
        assert!(matches!(result, Err(DomainError::PermissaoNegada(_))));
    }

    #[test]
    fn gerente_nao_concede_capacidade_que_nao_tem() {
        let mut repo = MockUsuarioRepo::new();
        let vendedor = Usuario::novo(
            UsuarioId::new(),
            "Vendedor".into(),
            "vendedor".into(),
            "$argon2id$hash".into(),
            Papel::Vendedor,
            None,
            Utc::now(),
        )
        .unwrap();
        let vendedor_id = vendedor.id;
        repo.usuarios.push(vendedor);
        let repos = UsuarioRepos { usuarios: &repo };

        let result = gerenciar_capacidades(
            &repos,
            vendedor_id,
            vec!["usuario.criar.gerente".to_string()],
            &ator(Papel::Gerente),
            Utc::now(),
        );
        assert!(matches!(result, Err(DomainError::PermissaoNegada(_))));
    }

    #[test]
    fn operador_nao_lista_usuarios() {
        let repo = MockUsuarioRepo::new();
        let repos = UsuarioRepos { usuarios: &repo };

        let result = listar_usuarios(
            &repos,
            ListaUsuarioQuery { ativos: true },
            &ator(Papel::OperadorCaixa),
        );
        assert!(matches!(result, Err(DomainError::PermissaoNegada(_))));
    }
}
