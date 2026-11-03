//! Repositório de usuários (SQLite).

use rusqlite::{params, Connection, OptionalExtension, Row};

use mikella_domain_people::ports::UsuarioRepository;
use mikella_domain_people::{Papel, Usuario};
use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::UsuarioId,
};

use super::{bool_para_inteiro, inteiro_para_bool, parse_data_hora, parse_id, sql, ParaDominio};
use crate::error::InfraResult;

/// Repositório de usuários.
///
/// Empresta `&Connection` — quando chamado dentro de uma `Transaction`,
/// participa da mesma transação (atomicidade).
pub struct UsuarioRepo<'a> {
    pub conn: &'a Connection,
}

impl<'a> UsuarioRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn usuario_bruto_para_dominio(conn: &Connection, bruto: UsuarioBruto) -> InfraResult<Usuario> {
        let papel = Papel::from_db(&bruto.papel)
            .map_err(|e| crate::error::erro_campo("usuarios.papel", e))?;

        // Carrega capacidades extras da tabela usuario_capacidades
        let capacidades_extras: Vec<String> = conn
            .prepare("SELECT capacidade FROM usuario_capacidades WHERE usuario_id = ?1")?
            .query_map([bruto.id.clone()], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(Usuario {
            id: parse_id(&bruto.id, "usuarios.id")?,
            nome: bruto.nome,
            username: bruto.username,
            email: bruto.email,
            senha_hash: bruto.senha_hash,
            papel,
            pin_hash: bruto.pin_hash,
            capacidades_extras,
            ativo: inteiro_para_bool(bruto.ativo, "usuarios.ativo")?,
            criado_em: parse_data_hora(&bruto.criado_em, "usuarios.created_at")?,
            atualizado_em: parse_data_hora(&bruto.atualizado_em, "usuarios.updated_at")?,
        })
    }

    fn buscar_por_id_inner(&self, id: UsuarioId) -> InfraResult<Option<Usuario>> {
        let id_str = id.to_string();
        let bruto = self
            .conn
            .query_row(SQL_SELECT_USUARIO, params![id_str], UsuarioBruto::da_linha)
            .optional()?;
        bruto
            .map(|b| Self::usuario_bruto_para_dominio(self.conn, b))
            .transpose()
    }

    fn buscar_por_username_inner(&self, username: &str) -> InfraResult<Option<Usuario>> {
        let bruto = self
            .conn
            .query_row(
                SQL_SELECT_BY_USERNAME,
                params![username],
                UsuarioBruto::da_linha,
            )
            .optional()?;
        bruto
            .map(|b| Self::usuario_bruto_para_dominio(self.conn, b))
            .transpose()
    }

    fn listar(&self, ativo_apenas: bool) -> InfraResult<Vec<Usuario>> {
        let where_clause = if ativo_apenas { "WHERE ativo = 1" } else { "" };
        let query = format!("{} {}", SQL_SELECT_ALL, where_clause);
        let mut stmt = self.conn.prepare(&query)?;
        let linhas = stmt.query_map([], UsuarioBruto::da_linha)?;
        let mut usuarios = Vec::new();
        for linha in linhas {
            let bruto = linha?;
            usuarios.push(Self::usuario_bruto_para_dominio(self.conn, bruto)?);
        }
        Ok(usuarios)
    }
}

const SQL_SELECT_USUARIO: &str = "\
    SELECT id, nome, username, email, senha_hash, papel, pin_hash, ativo,
           strftime('%Y-%m-%dT%H:%M:%fZ', created_at) as criado_em,
           strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) as atualizado_em
    FROM usuarios
    WHERE id = ?1
";

const SQL_SELECT_BY_USERNAME: &str = "\
    SELECT id, nome, username, email, senha_hash, papel, pin_hash, ativo,
           strftime('%Y-%m-%dT%H:%M:%fZ', created_at) as criado_em,
           strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) as atualizado_em
    FROM usuarios WHERE username = ?1 COLLATE NOCASE
";

const SQL_SELECT_ALL: &str = "\
    SELECT id, nome, username, email, senha_hash, papel, pin_hash, ativo,
           strftime('%Y-%m-%dT%H:%M:%fZ', created_at) as criado_em,
           strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) as atualizado_em
    FROM usuarios
";

impl<'a> UsuarioRepository for UsuarioRepo<'a> {
    fn salvar(&self, usuario: &Usuario) -> DomainResult<()> {
        let alteradas = sql(self.conn.execute(
            "INSERT INTO usuarios
                 (id, nome, username, senha_hash, papel, email, pin_hash, ativo,
                  created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                    strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                    strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![
                usuario.id.to_string(),
                usuario.nome,
                usuario.username,
                usuario.senha_hash,
                usuario.papel.as_db(),
                usuario.email,
                usuario.pin_hash,
                bool_para_inteiro(usuario.ativo),
            ],
        ))?;
        if alteradas == 0 {
            return Err(DomainError::EstadoInvalido(
                "Falha ao inserir usuário".into(),
            ));
        }

        // Salva capacidades extras
        for cap in &usuario.capacidades_extras {
            sql(self.conn.execute(
                "INSERT OR IGNORE INTO usuario_capacidades (usuario_id, capacidade) VALUES (?1, ?2)",
                params![usuario.id.to_string(), cap],
            ))?;
        }

        Ok(())
    }

    fn atualizar(&self, usuario: &Usuario) -> DomainResult<()> {
        let alteradas = sql(self.conn.execute(
            "UPDATE usuarios SET
                 nome = ?2, email = ?3, senha_hash = ?4, pin_hash = ?5,
                 ativo = ?6,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?1",
            params![
                usuario.id.to_string(),
                usuario.nome,
                usuario.email,
                usuario.senha_hash,
                usuario.pin_hash,
                bool_para_inteiro(usuario.ativo),
            ],
        ))?;
        if alteradas == 0 {
            return Err(DomainError::NaoEncontrado {
                entidade: "Usuario",
                id: usuario.id.to_string(),
            });
        }

        // Sincroniza capacidades extras
        sql(self.conn.execute(
            "DELETE FROM usuario_capacidades WHERE usuario_id = ?1",
            params![usuario.id.to_string()],
        ))?;

        for cap in &usuario.capacidades_extras {
            sql(self.conn.execute(
                "INSERT OR IGNORE INTO usuario_capacidades (usuario_id, capacidade) VALUES (?1, ?2)",
                params![usuario.id.to_string(), cap],
            ))?;
        }

        Ok(())
    }

    fn buscar_por_id(&self, id: UsuarioId) -> DomainResult<Option<Usuario>> {
        self.buscar_por_id_inner(id).dominio()
    }

    fn buscar_por_username(&self, username: &str) -> DomainResult<Option<Usuario>> {
        self.buscar_por_username_inner(username).dominio()
    }

    fn listar_ativos(&self) -> DomainResult<Vec<Usuario>> {
        self.listar(true).dominio()
    }

    fn listar_todos(&self) -> DomainResult<Vec<Usuario>> {
        self.listar(false).dominio()
    }
}

/// Estrutura bruta do banco, convertida para `Usuario` via `usuario_bruto_para_dominio`.
struct UsuarioBruto {
    id: String,
    nome: String,
    username: String,
    email: Option<String>,
    senha_hash: String,
    papel: String,
    pin_hash: Option<String>,
    ativo: i64,
    criado_em: String,
    atualizado_em: String,
}

impl UsuarioBruto {
    fn da_linha(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            nome: row.get(1)?,
            username: row.get(2)?,
            email: row.get(3)?,
            senha_hash: row.get(4)?,
            papel: row.get(5)?,
            pin_hash: row.get(6)?,
            ativo: row.get(7)?,
            criado_em: row.get(8)?,
            atualizado_em: row.get(9)?,
        })
    }
}
