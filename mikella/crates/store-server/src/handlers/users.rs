//! Handlers de gerenciamento de usuários (MVP-1 — contínuo).
//!
//! Casos de uso de usuário via `app_people::usuario`:
//! - criar_usuario (POST /v1/usuarios)
//! - listar_usuarios (GET /v1/usuarios)
//! - buscar_usuario (GET /v1/usuarios/:id)
//! - atualizar_usuario (PUT /v1/usuarios/:id)
//! - desativar_usuario (POST /v1/usuarios/:id/desativar)
//! - gerenciar_capacidades (POST /v1/usuarios/:id/capacidades)
//! - atualizar_senha (PUT /v1/usuarios/:id/senha)
//!
//! Regras de segurança (SEG-010, SEG-020, SEG-021, FRD-055):
//! - O ator vem do token + banco, nunca do corpo do request.
//! - Hierarquia estrita no domínio: gerente não altera dono nem outro gerente.
//! - Capacidades fora do catálogo ou que o ator não possui são recusadas.
//! - Senha é hasheada server-side (Argon2id) antes de tocar o domínio.
//! - Alterar senha, capacidades, e-mail ou desativar revoga as sessões do alvo.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use mikella_app_people::usuario::{
    atualizar_senha as atualizar_senha_uc, atualizar_usuario as atualizar_usuario_uc,
    criar_usuario, desativar_usuario as desativar_usuario_uc,
    gerenciar_capacidades as gerenciar_capacidades_uc, listar_usuarios, AtualizarUsuarioCmd,
    CriarUsuarioCmd, DesativarUsuarioCmd, ListaUsuarioQuery, UsuarioRepos,
};
use mikella_domain_people::Usuario;
use mikella_infra_sqlite::UsuarioRepo;
use mikella_shared_kernel::clock::{Clock, SystemClock};
use mikella_shared_kernel::ids::UsuarioId;
use uuid::Uuid;

use crate::dto::{AtualizarSenhaForm, AtualizarUsuarioForm, CapacidadesForm, CriarUsuarioForm};
use crate::exigir;
use crate::middleware::Ator;
use crate::state::AppState;
use crate::util::{bad_request, error_para_http, hash_senha, senha_aceitavel};

/// `POST /v1/usuarios` — cria um novo usuário.
pub async fn criar_usuario_handler(
    State(state): State<AppState>,
    ator: Ator,
    Json(form): Json<CriarUsuarioForm>,
) -> impl IntoResponse {
    exigir!(ator, "usuario.gerenciar");
    tracing::debug!(operador = %ator.0.username, "solicitação de criação de usuário");

    if form.nome.trim().is_empty() {
        return bad_request("nome não pode ser vazio");
    }
    if form.username.trim().len() < 3 || form.username.trim().len() > 64 {
        return bad_request("username deve ter entre 3 e 64 caracteres");
    }
    if form.email.trim().is_empty() {
        return bad_request("e-mail é obrigatório para entrega do código 2FA");
    }
    if !senha_aceitavel(&form.senha) {
        return bad_request("senha fraca: use ao menos 8 caracteres e evite senhas comuns");
    }

    let usuario_id: Uuid = match form.usuario_id.parse() {
        Ok(id) => id,
        Err(_) => return bad_request("usuario_id inválido (UUID esperado)"),
    };

    let db = state.db.lock().await;
    let senha_hash = hash_senha(&form.senha);

    let cmd = CriarUsuarioCmd {
        usuario_id: UsuarioId::from_uuid(usuario_id),
        nome: form.nome,
        username: form.username,
        senha_hash,
        papel: form.papel,
        email: Some(form.email),
        capacidades_extras: form.capacidades_extras,
    };

    let usuarios_repo = UsuarioRepo::novo(db.conn());
    let repos = UsuarioRepos {
        usuarios: &usuarios_repo,
    };

    match criar_usuario(&repos, cmd, &ator.0) {
        Ok(usuario) => {
            let audit = mikella_infra_sqlite::AuditoriaRepo::novo(db.conn());
            let _ = audit.registrar(
                "usuario",
                &usuario.id.to_string(),
                "CRIACAO_USUARIO",
                Some(&ator.id().to_string()),
                None,
                None,
                Some(&format!(
                    "username: {}, papel: {:?}",
                    usuario.username, usuario.papel
                )),
            );
            (StatusCode::CREATED, Json(usuario_para_response(&usuario)))
        }
        Err(e) => {
            tracing::warn!(erro = %e, "criação de usuário rejeitada pelo domínio");
            error_para_http(e)
        }
    }
}

/// `GET /v1/usuarios` — lista usuários (ativos por padrão).
pub async fn listar_usuarios_handler(
    State(state): State<AppState>,
    ator: Ator,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    tracing::debug!(operador = %ator.0.username, "listando usuários");

    let ativos_filter = query
        .get("ativos")
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(true);

    let db = state.db.lock().await;
    let usuarios_repo = UsuarioRepo::novo(db.conn());
    let repos = UsuarioRepos {
        usuarios: &usuarios_repo,
    };

    match listar_usuarios(
        &repos,
        ListaUsuarioQuery {
            ativos: ativos_filter,
        },
        &ator.0,
    ) {
        Ok(usuarios) => {
            let lista: Vec<_> = usuarios.iter().map(usuario_para_response).collect();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "usuarios": lista })),
            )
        }
        Err(e) => {
            tracing::warn!(erro = %e, "listagem de usuários rejeitada");
            error_para_http(e)
        }
    }
}

/// `GET /v1/usuarios/:id` — busca usuário por ID.
///
/// O próprio usuário pode consultar o próprio perfil; terceiros exigem
/// `usuario.ver`.
pub async fn buscar_usuario_handler(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
    ator: Ator,
) -> impl IntoResponse {
    tracing::debug!(operador = %ator.0.username, id = %id_str, "buscando usuário");

    let usuario_id = match Uuid::parse_str(&id_str) {
        Ok(id) => UsuarioId::from_uuid(id),
        Err(_) => return bad_request("usuario_id inválido (UUID esperado)"),
    };

    if usuario_id != ator.id() {
        exigir!(ator, "usuario.ver");
    }

    let db = state.db.lock().await;
    let usuarios_repo = UsuarioRepo::novo(db.conn());
    let repos = UsuarioRepos {
        usuarios: &usuarios_repo,
    };

    match repos.usuarios.buscar_por_id(usuario_id) {
        Ok(Some(u)) => (StatusCode::OK, Json(usuario_para_response(&u))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "erro": "usuário não encontrado" })),
        ),
        Err(e) => error_para_http(e),
    }
}

/// `PUT /v1/usuarios/:id` — atualiza nome e e-mail.
pub async fn atualizar_usuario_handler(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
    ator: Ator,
    Json(form): Json<AtualizarUsuarioForm>,
) -> impl IntoResponse {
    tracing::debug!(operador = %ator.0.username, id = %id_str, "atualizando usuário");

    let usuario_id = match Uuid::parse_str(&id_str) {
        Ok(id) => UsuarioId::from_uuid(id),
        Err(_) => return bad_request("usuario_id inválido (UUID esperado)"),
    };
    if form.nome.trim().is_empty() {
        return bad_request("nome não pode ser vazio");
    }
    if let Some(email) = &form.email {
        if email.trim().is_empty() {
            return bad_request("e-mail não pode ser vazio");
        }
    }
    let revogar = form.email.is_some();

    let resultado = {
        let db = state.db.lock().await;
        let usuarios_repo = UsuarioRepo::novo(db.conn());
        let repos = UsuarioRepos {
            usuarios: &usuarios_repo,
        };
        atualizar_usuario_uc(
            &repos,
            AtualizarUsuarioCmd {
                usuario_id,
                nome: form.nome,
                email: form.email,
            },
            &ator.0,
        )
    };

    match resultado {
        Ok(usuario) => {
            {
                let db = state.db.lock().await;
                let audit = mikella_infra_sqlite::AuditoriaRepo::novo(db.conn());
                let _ = audit.registrar(
                    "usuario",
                    &usuario.id.to_string(),
                    "ATUALIZACAO_USUARIO",
                    Some(&ator.id().to_string()),
                    None,
                    None,
                    Some(&format!(
                        "username: {}, papel: {:?}",
                        usuario.username, usuario.papel
                    )),
                );
            }
            if revogar {
                revogar_sessoes(&state, &usuario.username).await;
            }
            (StatusCode::OK, Json(usuario_para_response(&usuario)))
        }
        Err(e) => {
            tracing::warn!(erro = %e, "atualização de usuário rejeitada");
            error_para_http(e)
        }
    }
}

/// `POST /v1/usuarios/:id/desativar` — desativa usuário (soft-delete).
pub async fn desativar_usuario_handler(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
    ator: Ator,
) -> impl IntoResponse {
    tracing::debug!(operador = %ator.0.username, id = %id_str, "desativando usuário");

    let usuario_id = match Uuid::parse_str(&id_str) {
        Ok(id) => UsuarioId::from_uuid(id),
        Err(_) => return bad_request("usuario_id inválido (UUID esperado)"),
    };

    let resultado = {
        let db = state.db.lock().await;
        let usuarios_repo = UsuarioRepo::novo(db.conn());
        let repos = UsuarioRepos {
            usuarios: &usuarios_repo,
        };
        desativar_usuario_uc(&repos, DesativarUsuarioCmd { usuario_id }, &ator.0)
    };

    match resultado {
        Ok(()) => {
            {
                let db = state.db.lock().await;
                let audit = mikella_infra_sqlite::AuditoriaRepo::novo(db.conn());
                let _ = audit.registrar(
                    "usuario",
                    &id_str,
                    "DESATIVACAO_USUARIO",
                    Some(&ator.id().to_string()),
                    None,
                    None,
                    None,
                );
            }
            if let Some(username) = username_por_id(&state, usuario_id).await {
                revogar_sessoes(&state, &username).await;
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({ "desativado": true, "id": id_str })),
            )
        }
        Err(e) => {
            tracing::warn!(erro = %e, "desativação rejeitada");
            error_para_http(e)
        }
    }
}

/// `POST /v1/usuarios/:id/capacidades` — substitui capacidades extras.
pub async fn gerenciar_capacidades_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    ator: Ator,
    Json(form): Json<CapacidadesForm>,
) -> impl IntoResponse {
    tracing::debug!(operador = %ator.0.username, id = %id, "gerenciando capacidades");

    let usuario_id = match Uuid::parse_str(&id) {
        Ok(uuid) => UsuarioId::from_uuid(uuid),
        Err(_) => return bad_request("usuario_id inválido (UUID esperado)"),
    };

    let agora = SystemClock.now();
    let resultado = {
        let db = state.db.lock().await;
        let usuarios_repo = UsuarioRepo::novo(db.conn());
        let repos = UsuarioRepos {
            usuarios: &usuarios_repo,
        };
        gerenciar_capacidades_uc(&repos, usuario_id, form.capacidades, &ator.0, agora)
    };

    match resultado {
        Ok(capacidades) => {
            if let Some(username) = username_por_id(&state, usuario_id).await {
                revogar_sessoes(&state, &username).await;
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({ "capacidades": capacidades, "id": id })),
            )
        }
        Err(e) => {
            tracing::warn!(erro = %e, "gerenciamento de capacidades rejeitado");
            error_para_http(e)
        }
    }
}

/// `PUT /v1/usuarios/:id/senha` — atualiza a senha de um usuário.
pub async fn atualizar_senha_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    ator: Ator,
    Json(form): Json<AtualizarSenhaForm>,
) -> impl IntoResponse {
    tracing::debug!(operador = %ator.0.username, id = %id, "atualizando senha");

    let usuario_id = match Uuid::parse_str(&id) {
        Ok(uuid) => UsuarioId::from_uuid(uuid),
        Err(_) => return bad_request("usuario_id inválido (UUID esperado)"),
    };
    if !senha_aceitavel(&form.senha) {
        return bad_request("senha fraca: use ao menos 8 caracteres e evite senhas comuns");
    }

    let agora = SystemClock.now();
    let senha_hash = hash_senha(&form.senha);

    let resultado = {
        let db = state.db.lock().await;
        let usuarios_repo = UsuarioRepo::novo(db.conn());
        let repos = UsuarioRepos {
            usuarios: &usuarios_repo,
        };
        atualizar_senha_uc(&repos, usuario_id, senha_hash, agora, &ator.0)
    };

    match resultado {
        Ok(()) => {
            if let Some(username) = username_por_id(&state, usuario_id).await {
                revogar_sessoes(&state, &username).await;
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({ "atualizado": true, "id": id })),
            )
        }
        Err(e) => {
            tracing::warn!(erro = %e, "atualização de senha rejeitada");
            error_para_http(e)
        }
    }
}

fn usuario_para_response(usuario: &Usuario) -> serde_json::Value {
    serde_json::json!({
        "id": usuario.id.to_string(),
        "nome": usuario.nome,
        "username": usuario.username,
        "email": usuario.email,
        "papel": usuario.papel.as_db(),
        "ativo": usuario.ativo,
        "capacidades": usuario.capacidades(),
        "criado_em": usuario.criado_em.to_rfc3339(),
        "atualizado_em": usuario.atualizado_em.to_rfc3339(),
    })
}

async fn username_por_id(state: &AppState, id: UsuarioId) -> Option<String> {
    let db = state.db.lock().await;
    match UsuarioRepo::novo(db.conn()).username_por_id(id) {
        Ok(username) => username,
        Err(e) => {
            tracing::warn!(erro = %e, usuario_id = %id, "falha ao resolver username do usuário");
            None
        }
    }
}

async fn revogar_sessoes(state: &AppState, username: &str) {
    let mut sessoes = state.sessoes.lock().await;
    sessoes.retain(|_, s| !s.username.eq_ignore_ascii_case(username));
}
