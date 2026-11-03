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

// ---------------------------------------------------------------------------
// Repositório de Pessoas / Clientes (Clean Architecture)
// ---------------------------------------------------------------------------

use mikella_domain_people::ports::{ClienteResumo, DevedorItem, ParcelaDevedor, PessoaRepository};
use mikella_domain_people::{Pessoa, TipoPessoa};

pub struct PessoaRepo<'a> {
    pub conn: &'a Connection,
}

impl<'a> PessoaRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl<'a> PessoaRepository for PessoaRepo<'a> {
    fn salvar(&self, p: &Pessoa) -> DomainResult<()> {
        let alteradas = sql(self.conn.execute(
            "INSERT INTO pessoas (id, tipo, nome, telefone, email, documento, endereco, ativo, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                     strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                     strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![
                p.id,
                p.tipo.as_db(),
                p.nome,
                p.telefone,
                p.email,
                p.documento,
                p.endereco,
                bool_para_inteiro(p.ativo),
            ],
        ))?;
        if alteradas == 0 {
            return Err(DomainError::EstadoInvalido("Falha ao salvar pessoa".into()));
        }
        Ok(())
    }

    fn atualizar(&self, p: &Pessoa) -> DomainResult<()> {
        let alteradas = sql(self.conn.execute(
            "UPDATE pessoas SET
                 nome = ?2, telefone = ?3, email = ?4, documento = ?5,
                 endereco = ?6, ativo = ?7,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?1",
            params![
                p.id,
                p.nome,
                p.telefone,
                p.email,
                p.documento,
                p.endereco,
                bool_para_inteiro(p.ativo),
            ],
        ))?;
        if alteradas == 0 {
            return Err(DomainError::NaoEncontrado {
                entidade: "Pessoa",
                id: p.id.clone(),
            });
        }
        Ok(())
    }

    fn buscar_por_id(&self, id: &str) -> DomainResult<Option<Pessoa>> {
        let sql_query = "SELECT id, tipo, nome, telefone, email, documento, endereco, ativo,
                                strftime('%Y-%m-%dT%H:%M:%fZ', created_at),
                                strftime('%Y-%m-%dT%H:%M:%fZ', updated_at)
                         FROM pessoas WHERE id = ?1";
        let res = self
            .conn
            .query_row(sql_query, [id], |row| {
                let tipo_str: String = row.get(1)?;
                let tipo = TipoPessoa::from_db(&tipo_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
                })?;
                let criado_em_str: String = row.get(8)?;
                let atualizado_em_str: String = row.get(9)?;
                Ok(Pessoa {
                    id: row.get(0)?,
                    tipo,
                    nome: row.get(2)?,
                    telefone: row.get(3)?,
                    email: row.get(4)?,
                    documento: row.get(5)?,
                    endereco: row.get(6)?,
                    ativo: row.get::<_, i64>(7)? != 0,
                    criado_em: parse_data_hora(&criado_em_str, "pessoas.created_at").map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
                    })?,
                    atualizado_em: parse_data_hora(&atualizado_em_str, "pessoas.updated_at").map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(e))
                    })?,
                })
            })
            .optional()
            .map_err(|e| DomainError::EstadoInvalido(format!("Erro ao buscar pessoa: {e}")))?;
        Ok(res)
    }

    fn buscar_clientes_com_saldo(&self, busca: Option<&str>) -> DomainResult<Vec<ClienteResumo>> {
        let (sql_query, param) = match busca.map(|s| s.trim()).filter(|s| !s.is_empty()) {
            Some(b) => {
                let p = format!("%{b}%");
                (
                    "SELECT id, tipo, nome, telefone, email, documento, endereco, ativo,
                            strftime('%Y-%m-%dT%H:%M:%fZ', created_at),
                            strftime('%Y-%m-%dT%H:%M:%fZ', updated_at)
                     FROM pessoas
                     WHERE tipo = 'CLIENTE' AND ativo = 1 AND (nome LIKE ?1 OR documento LIKE ?1 OR telefone LIKE ?1)
                     ORDER BY nome ASC LIMIT 50",
                    Some(p),
                )
            }
            None => (
                "SELECT id, tipo, nome, telefone, email, documento, endereco, ativo,
                        strftime('%Y-%m-%dT%H:%M:%fZ', created_at),
                        strftime('%Y-%m-%dT%H:%M:%fZ', updated_at)
                 FROM pessoas
                 WHERE tipo = 'CLIENTE' AND ativo = 1
                 ORDER BY nome ASC LIMIT 50",
                None,
            ),
        };

        let mut stmt = self
            .conn
            .prepare(sql_query)
            .map_err(|e| DomainError::EstadoInvalido(format!("Erro ao preparar consulta: {e}")))?;

        let mut clientes = Vec::new();
        let mapper = |row: &Row<'_>| {
            let tipo_str: String = row.get(1)?;
            let tipo = TipoPessoa::from_db(&tipo_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
            })?;
            let criado_em_str: String = row.get(8)?;
            let atualizado_em_str: String = row.get(9)?;
            Ok(Pessoa {
                id: row.get(0)?,
                tipo,
                nome: row.get(2)?,
                telefone: row.get(3)?,
                email: row.get(4)?,
                documento: row.get(5)?,
                endereco: row.get(6)?,
                ativo: row.get::<_, i64>(7)? != 0,
                criado_em: parse_data_hora(&criado_em_str, "pessoas.created_at").map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
                })?,
                atualizado_em: parse_data_hora(&atualizado_em_str, "pessoas.updated_at").map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(e))
                })?,
            })
        };

        let rows = if let Some(p) = param {
            stmt.query_map([p], mapper)
        } else {
            stmt.query_map([], mapper)
        }
        .map_err(|e| DomainError::EstadoInvalido(format!("Erro na consulta de clientes: {e}")))?;

        for r in rows {
            let pessoa = r.map_err(|e| DomainError::EstadoInvalido(format!("Erro ao ler cliente: {e}")))?;

            // Calcula saldo devedor e se há parcelas vencidas no crediário
            let (saldo, tem_vencido): (i64, bool) = self
                .conn
                .query_row(
                    "SELECT COALESCE(SUM(p.valor_centavos), 0),
                            COALESCE(MAX(CASE WHEN p.vencimento < date('now') THEN 1 ELSE 0 END), 0)
                     FROM parcelas_receber p
                     INNER JOIN contas_receber c ON c.id = p.conta_id
                     WHERE c.pessoa_id = ?1 AND p.situacao = 'ABERTA'",
                    [&pessoa.id],
                    |row| Ok((row.get(0)?, row.get::<_, i64>(1)? > 0)),
                )
                .unwrap_or((0, false));

            clientes.push(ClienteResumo {
                pessoa,
                saldo_devedor_centavos: saldo,
                tem_vencido,
            });
        }

        Ok(clientes)
    }

    fn listar_devedores(&self) -> DomainResult<Vec<DevedorItem>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT pe.id, pe.nome, pe.documento, pe.telefone,
                        pa.id as parcela_id, pa.numero, pa.valor_centavos, pa.vencimento,
                        CAST(julianday('now') - julianday(pa.vencimento) AS INTEGER) as dias_atraso
                 FROM parcelas_receber pa
                 INNER JOIN contas_receber c ON c.id = pa.conta_id
                 INNER JOIN pessoas pe ON pe.id = c.pessoa_id
                 WHERE pa.situacao = 'ABERTA'
                 ORDER BY pe.nome ASC, pa.vencimento ASC",
            )
            .map_err(|e| DomainError::EstadoInvalido(format!("Erro ao preparar lista de devedores: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, u32>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            })
            .map_err(|e| DomainError::EstadoInvalido(format!("Erro ao consultar devedores: {e}")))?;

        let mut mapa: std::collections::BTreeMap<String, DevedorItem> = std::collections::BTreeMap::new();
        for r in rows {
            let (cid, nome, doc, tel, pid, num, valor, venc, atraso) =
                r.map_err(|e| DomainError::EstadoInvalido(format!("Erro ao ler parcela devedora: {e}")))?;

            let devedor = mapa.entry(nome.clone()).or_insert_with(|| DevedorItem {
                cliente_id: cid,
                nome,
                documento: doc,
                telefone: tel,
                total_devido_centavos: 0,
                parcelas: Vec::new(),
            });

            devedor.total_devido_centavos += valor;
            devedor.parcelas.push(ParcelaDevedor {
                parcela_id: pid,
                numero: num,
                valor_centavos: valor,
                vencimento: venc,
                dias_atraso: atraso.max(0),
            });
        }

        Ok(mapa.into_values().collect())
    }

    fn liquidar_parcela(&self, parcela_id: &str, meio_pagamento: &str) -> DomainResult<i64> {
        let (valor, conta_id): (i64, String) = self
            .conn
            .query_row(
                "SELECT valor_centavos, conta_id FROM parcelas_receber WHERE id = ?1 AND situacao = 'ABERTA'",
                [parcela_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| DomainError::EstadoInvalido(format!("Parcela não encontrada ou já liquidada: {e}")))?;

        sql(self.conn.execute(
            "UPDATE parcelas_receber
             SET situacao = 'LIQUIDADA',
                 pago_em = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 forma_liquidacao = ?2
             WHERE id = ?1",
            params![parcela_id, meio_pagamento],
        ))?;

        // Verifica se a conta ainda tem parcelas abertas
        let abertas: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM parcelas_receber WHERE conta_id = ?1 AND situacao = 'ABERTA'",
                [&conta_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let novo_status = if abertas == 0 { "LIQUIDADA" } else { "PARCIAL" };
        sql(self.conn.execute(
            "UPDATE contas_receber SET status = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1",
            params![&conta_id, novo_status],
        ))?;

        Ok(valor)
    }
}
