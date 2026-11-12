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
use crate::hash::blind_index_documento;
use crate::tempo::data_local_iso;

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

    /// Username do usuário (para revogar sessões por id sem SQL no handler).
    pub fn username_por_id(&self, id: UsuarioId) -> InfraResult<Option<String>> {
        let username = self
            .conn
            .query_row(
                "SELECT username FROM usuarios WHERE id = ?1",
                params![id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(username)
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
    /// Chave estável da loja para blind index de CPF (SEC-014 / LGPD-003).
    /// `None` em contextos sem chave (testes legados) — blind index fica NULL.
    chave_loja: Option<[u8; 32]>,
}

impl<'a> PessoaRepo<'a> {
    pub fn novo(conn: &'a Connection) -> Self {
        Self {
            conn,
            chave_loja: None,
        }
    }

    /// Variante com chave da loja para habilitar blind index de CPF/CNPJ.
    pub fn com_chave(conn: &'a Connection, chave_loja: &[u8; 32]) -> Self {
        Self {
            conn,
            chave_loja: Some(*chave_loja),
        }
    }

    /// Calcula o blind index do documento, se chave disponível.
    ///
    /// Normaliza para dígitos — a mesma forma gravada por `Pessoa::novo_cliente`
    /// — para que registros antigos com pontuação gerem o mesmo índice.
    fn blind_documento(&self, documento: &str) -> Option<String> {
        let digits: String = documento.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        self.chave_loja
            .as_ref()
            .map(|k| crate::hash::blind_index_documento(&digits, k))
    }

    /// Cifra um dado pessoal (PII) em repouso se houver chave configurada.
    fn cifrar_campo(&self, texto: Option<&str>, contexto: &str) -> Option<String> {
        match (texto, &self.chave_loja) {
            (Some(t), Some(k)) if !t.trim().is_empty() => {
                mikella_infra_crypto::cifrar_pii(t.trim(), k, contexto).ok()
            }
            (Some(t), _) => Some(t.to_string()),
            (None, _) => None,
        }
    }

    /// Decifra um dado pessoal lido do banco (ou preserva texto legado).
    pub fn decifrar_campo(&self, texto: Option<String>) -> Option<String> {
        match (texto, &self.chave_loja) {
            (Some(t), Some(k)) => mikella_infra_crypto::decifrar_texto(&t, k).ok(),
            (Some(t), None) => Some(t),
            (None, _) => None,
        }
    }

    /// Anonimiza os dados de um cliente para atender à solicitação de exclusão (Art. 16/18 LGPD).
    /// Mantém o ID e registros históricos de vendas/contas para fins fiscais e legais.
    pub fn anonimizar_cliente(&self, id: &str) -> InfraResult<bool> {
        let tem_debito: bool = self.conn.query_row(
            "SELECT EXISTS(
                 SELECT 1
                   FROM parcelas pa
                   JOIN contas_receber cr ON cr.id = pa.conta_id
                  WHERE cr.pessoa_id = ?1
                    AND pa.conta_tipo = 'RECEBIMENTO'
                    AND pa.status = 'ABERTA'
             )",
            params![id],
            |r| r.get(0),
        )?;
        if tem_debito {
            return Err(crate::error::InfraError::DadoInvalido {
                campo: "crediario",
                detalhe: "Cliente possui débitos de crediário em aberto e não pode ser anonimizado até quitação".into(),
            });
        }

        let linhas = self.conn.execute(
            "UPDATE pessoas 
             SET nome = '[CLIENTE ANONIMIZADO]',
                 documento = NULL,
                 documento_blind = 'ANON_' || id,
                 telefone = NULL,
                 email = NULL,
                 endereco = NULL,
                 ativo = 0,
                 consentimento_marketing = 0
             WHERE id = ?1",
            params![id],
        )?;
        Ok(linhas > 0)
    }
}

impl<'a> PessoaRepository for PessoaRepo<'a> {
    fn salvar(&self, p: &Pessoa) -> DomainResult<()> {
        let blind = p.documento.as_deref().and_then(|d| self.blind_documento(d));
        let doc_salvo = self.cifrar_campo(p.documento.as_deref(), "documento");
        let tel_salvo = self.cifrar_campo(p.telefone.as_deref(), "telefone");
        let email_salvo = self.cifrar_campo(p.email.as_deref(), "email");
        let end_salvo = self.cifrar_campo(p.endereco.as_deref(), "endereco");

        let alteradas = sql(self.conn.execute(
            "INSERT INTO pessoas (id, tipo, nome, telefone, email, documento, documento_blind, endereco, ativo, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                     strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                     strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![
                p.id,
                p.tipo.as_db(),
                p.nome,
                tel_salvo,
                email_salvo,
                doc_salvo,
                blind,
                end_salvo,
                bool_para_inteiro(p.ativo),
            ],
        ))?;
        if alteradas == 0 {
            return Err(DomainError::EstadoInvalido("Falha ao salvar pessoa".into()));
        }
        Ok(())
    }

    fn atualizar(&self, p: &Pessoa) -> DomainResult<()> {
        let blind = p.documento.as_deref().and_then(|d| self.blind_documento(d));
        let doc_salvo = self.cifrar_campo(p.documento.as_deref(), "documento");
        let tel_salvo = self.cifrar_campo(p.telefone.as_deref(), "telefone");
        let email_salvo = self.cifrar_campo(p.email.as_deref(), "email");
        let end_salvo = self.cifrar_campo(p.endereco.as_deref(), "endereco");

        let alteradas = sql(self.conn.execute(
            "UPDATE pessoas SET
                 nome = ?2, telefone = ?3, email = ?4, documento = ?5,
                 documento_blind = ?6, endereco = ?7, ativo = ?8,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?1",
            params![
                p.id,
                p.nome,
                tel_salvo,
                email_salvo,
                doc_salvo,
                blind,
                end_salvo,
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
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                let criado_em_str: String = row.get(8)?;
                let atualizado_em_str: String = row.get(9)?;
                let tel_raw: Option<String> = row.get(3)?;
                let email_raw: Option<String> = row.get(4)?;
                let doc_raw: Option<String> = row.get(5)?;
                let end_raw: Option<String> = row.get(6)?;

                Ok(Pessoa {
                    id: row.get(0)?,
                    tipo,
                    nome: row.get(2)?,
                    telefone: self.decifrar_campo(tel_raw),
                    email: self.decifrar_campo(email_raw),
                    documento: self.decifrar_campo(doc_raw),
                    endereco: self.decifrar_campo(end_raw),
                    ativo: row.get::<_, i64>(7)? != 0,
                    criado_em: parse_data_hora(&criado_em_str, "pessoas.created_at").map_err(
                        |e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                8,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        },
                    )?,
                    atualizado_em: parse_data_hora(&atualizado_em_str, "pessoas.updated_at")
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                9,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?,
                })
            })
            .optional()
            .map_err(|e| DomainError::EstadoInvalido(format!("Erro ao buscar pessoa: {e}")))?;
        Ok(res)
    }

    fn telefone_em_uso(&self, telefone: &str, ignorar_id: Option<&str>) -> DomainResult<bool> {
        let tel_cifrado = self
            .cifrar_campo(Some(telefone), "telefone")
            .unwrap_or_else(|| telefone.to_string());
        let existe: i64 = sql(self.conn.query_row(
            "SELECT COUNT(*) FROM pessoas
             WHERE (telefone = ?1 OR telefone = ?2) AND (?3 IS NULL OR id <> ?3)",
            params![tel_cifrado, telefone, ignorar_id],
            |row| row.get(0),
        ))?;
        Ok(existe > 0)
    }

    fn documento_em_uso(&self, documento: &str, ignorar_id: Option<&str>) -> DomainResult<bool> {
        // SEC-014: se chave disponível, busca via blind index (coluna UNIQUE indexada);
        // caso contrário, busca por texto claro como fallback seguro para dev/testes.
        let existe: i64 = if let Some(blind) = self.blind_documento(documento) {
            sql(self.conn.query_row(
                "SELECT COUNT(*) FROM pessoas
                 WHERE documento_blind = ?1 AND (?2 IS NULL OR id <> ?2)",
                params![blind, ignorar_id],
                |row| row.get(0),
            ))?
        } else {
            sql(self.conn.query_row(
                "SELECT COUNT(*) FROM pessoas
                 WHERE documento = ?1 AND (?2 IS NULL OR id <> ?2)",
                params![documento, ignorar_id],
                |row| row.get(0),
            ))?
        };
        Ok(existe > 0)
    }

    fn buscar_clientes_com_saldo(&self, busca: Option<&str>) -> DomainResult<Vec<ClienteResumo>> {
        use rusqlite::types::Value;

        let termo = busca.map(str::trim).filter(|s| !s.is_empty());
        let (sql_query, param) = match termo {
            Some(b) => {
                // SEC-014: CPF/CNPJ completo é buscado pelo blind index (exato).
                // Nome, telefone e busca parcial continuam em texto claro até a
                // criptografia em repouso da F5.
                let digits: String = b.chars().filter(|c| c.is_ascii_digit()).collect();
                let blind = if digits.len() == 11 || digits.len() == 14 {
                    self.chave_loja
                        .as_ref()
                        .map(|chave| blind_index_documento(&digits, chave))
                } else {
                    None
                };

                match blind {
                    Some(hash) => (
                        "SELECT id, tipo, nome, telefone, email, documento, endereco, ativo,
                                strftime('%Y-%m-%dT%H:%M:%fZ', created_at),
                                strftime('%Y-%m-%dT%H:%M:%fZ', updated_at)
                         FROM pessoas
                         WHERE tipo = 'CLIENTE' AND ativo = 1
                           AND (nome LIKE ?1 OR telefone LIKE ?1 OR documento_blind = ?2)
                         ORDER BY nome ASC LIMIT 50",
                        vec![Value::Text(format!("%{b}%")), Value::Text(hash)],
                    ),
                    None => (
                        "SELECT id, tipo, nome, telefone, email, documento, endereco, ativo,
                                strftime('%Y-%m-%dT%H:%M:%fZ', created_at),
                                strftime('%Y-%m-%dT%H:%M:%fZ', updated_at)
                         FROM pessoas
                         WHERE tipo = 'CLIENTE' AND ativo = 1
                           AND (nome LIKE ?1 OR documento LIKE ?1 OR telefone LIKE ?1)
                         ORDER BY nome ASC LIMIT 50",
                        vec![Value::Text(format!("%{b}%"))],
                    ),
                }
            }
            None => (
                "SELECT id, tipo, nome, telefone, email, documento, endereco, ativo,
                        strftime('%Y-%m-%dT%H:%M:%fZ', created_at),
                        strftime('%Y-%m-%dT%H:%M:%fZ', updated_at)
                 FROM pessoas
                 WHERE tipo = 'CLIENTE' AND ativo = 1
                 ORDER BY nome ASC LIMIT 50",
                Vec::new(),
            ),
        };

        let mut stmt = self
            .conn
            .prepare(sql_query)
            .map_err(|e| DomainError::EstadoInvalido(format!("Erro ao preparar consulta: {e}")))?;

        let mut clientes = Vec::new();
        let hoje_s =
            data_local_iso(&crate::repos::sistema::DiagnosticoRepo::novo(self.conn).fuso_horario());
        let mapper = |row: &Row<'_>| {
            let tipo_str: String = row.get(1)?;
            let tipo = TipoPessoa::from_db(&tipo_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let criado_em_str: String = row.get(8)?;
            let atualizado_em_str: String = row.get(9)?;
            let tel_raw: Option<String> = row.get(3)?;
            let email_raw: Option<String> = row.get(4)?;
            let doc_raw: Option<String> = row.get(5)?;
            let end_raw: Option<String> = row.get(6)?;

            Ok(Pessoa {
                id: row.get(0)?,
                tipo,
                nome: row.get(2)?,
                telefone: self.decifrar_campo(tel_raw),
                email: self.decifrar_campo(email_raw),
                documento: self.decifrar_campo(doc_raw),
                endereco: self.decifrar_campo(end_raw),
                ativo: row.get::<_, i64>(7)? != 0,
                criado_em: parse_data_hora(&criado_em_str, "pessoas.created_at").map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        8,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                atualizado_em: parse_data_hora(&atualizado_em_str, "pessoas.updated_at").map_err(
                    |e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            9,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    },
                )?,
            })
        };

        let rows = stmt
            .query_map(rusqlite::params_from_iter(param), mapper)
            .map_err(|e| {
                DomainError::EstadoInvalido(format!("Erro na consulta de clientes: {e}"))
            })?;

        for r in rows {
            let pessoa =
                r.map_err(|e| DomainError::EstadoInvalido(format!("Erro ao ler cliente: {e}")))?;

            // Calcula saldo devedor e se há parcelas vencidas no crediário.
            // As parcelas de crediário vivem na tabela unificada `parcelas`
            // (conta_tipo = 'RECEBIMENTO'), criada pela migração 0004.
            let (saldo, tem_vencido): (i64, bool) = self
                .conn
                .query_row(
                    "SELECT COALESCE(SUM(p.valor_centavos), 0),
                            COALESCE(MAX(CASE WHEN date(p.vencimento) < ?2 THEN 1 ELSE 0 END), 0)
                     FROM parcelas p
                     INNER JOIN contas_receber c ON c.id = p.conta_id
                     WHERE c.pessoa_id = ?1
                       AND p.conta_tipo = 'RECEBIMENTO'
                       AND p.status = 'ABERTA'",
                    params![&pessoa.id, &hoje_s],
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
        let hoje_s =
            data_local_iso(&crate::repos::sistema::DiagnosticoRepo::novo(self.conn).fuso_horario());
        let mut stmt = self
            .conn
            .prepare(
                "SELECT pe.id, pe.nome, pe.documento, pe.telefone,
                        pa.id as parcela_id, pa.numero, pa.valor_centavos, pa.vencimento,
                        CAST(julianday(?1) - julianday(pa.vencimento) AS INTEGER) as dias_atraso
                 FROM parcelas pa
                 INNER JOIN contas_receber c ON c.id = pa.conta_id
                 INNER JOIN pessoas pe ON pe.id = c.pessoa_id
                 WHERE pa.conta_tipo = 'RECEBIMENTO' AND pa.status = 'ABERTA'
                 ORDER BY pe.nome ASC, pa.vencimento ASC",
            )
            .map_err(|e| {
                DomainError::EstadoInvalido(format!("Erro ao preparar lista de devedores: {e}"))
            })?;

        let rows = stmt
            .query_map(params![hoje_s], |row| {
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
            .map_err(|e| {
                DomainError::EstadoInvalido(format!("Erro ao consultar devedores: {e}"))
            })?;

        // Agrupa por cliente (id), não por nome: homônimos são pessoas distintas.
        let mut mapa: std::collections::BTreeMap<String, DevedorItem> =
            std::collections::BTreeMap::new();
        for r in rows {
            let (cid, nome, doc, tel, pid, num, valor, venc, atraso) = r.map_err(|e| {
                DomainError::EstadoInvalido(format!("Erro ao ler parcela devedora: {e}"))
            })?;

            let doc_dec = self.decifrar_campo(doc);
            let tel_dec = self.decifrar_campo(tel);

            let devedor = mapa.entry(cid.clone()).or_insert_with(|| DevedorItem {
                cliente_id: cid,
                nome,
                documento: doc_dec,
                telefone: tel_dec,
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

        let mut devedores: Vec<DevedorItem> = mapa.into_values().collect();
        devedores.sort_by_key(|d| d.nome.to_lowercase());
        Ok(devedores)
    }

    fn liquidar_parcela(&self, parcela_id: &str, meio_pagamento: &str) -> DomainResult<i64> {
        let (valor, conta_id): (i64, String) = self
            .conn
            .query_row(
                "SELECT valor_centavos, conta_id
                 FROM parcelas
                 WHERE id = ?1 AND conta_tipo = 'RECEBIMENTO' AND status = 'ABERTA'",
                [parcela_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| {
                DomainError::EstadoInvalido(format!("Parcela não encontrada ou já liquidada: {e}"))
            })?;

        let alteradas = sql(self.conn.execute(
            "UPDATE parcelas
             SET status = 'PAGA',
                 pago_em = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 forma_liquidacao = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?1 AND conta_tipo = 'RECEBIMENTO' AND status = 'ABERTA'",
            params![parcela_id, meio_pagamento],
        ))?;
        if alteradas == 0 {
            return Err(DomainError::EstadoInvalido(
                "Parcela já foi liquidada por outra operação".into(),
            ));
        }

        // A conta em si não tem coluna de status; o estado é derivado das
        // parcelas. Só tocamos `updated_at` para o histórico.
        sql(self.conn.execute(
            "UPDATE contas_receber SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1",
            [&conta_id],
        ))?;

        Ok(valor)
    }
}

/// Substitui o sentinela `PENDENTE_<id>` da migração `0008` pelo blind index
/// real de cada documento, usando a chave estável da loja.
///
/// Roda no boot, depois de abrir o banco e antes de aceitar requisições. É
/// idempotente: linhas já migradas (blind index real) não são tocadas.
/// Documentos legados com pontuação são normalizados para dígitos, a mesma
/// forma usada por `Pessoa::novo_cliente`.
///
/// Retorna a quantidade de linhas efetivamente atualizadas. Um duplicado
/// legado que viole o `UNIQUE` é mantido com o sentinela e registrado no log,
/// sem derrubar o boot nem expor o documento.
pub fn backfill_blind_index(conn: &Connection, chave_loja: &[u8; 32]) -> InfraResult<usize> {
    let pendentes: Vec<(String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, documento FROM pessoas
             WHERE documento IS NOT NULL
               AND (documento_blind IS NULL OR documento_blind LIKE 'PENDENTE_%')",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut atualizados = 0usize;
    for (id, documento) in pendentes {
        let digits: String = documento.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            tracing::warn!(
                pessoa_id = %id,
                "documento sem dígitos; blind index permanece pendente"
            );
            continue;
        }

        let blind = blind_index_documento(&digits, chave_loja);
        let doc_cifrado = if !documento.starts_with(mikella_infra_crypto::PREFIXO_CIFRADO) {
            mikella_infra_crypto::cifrar_pii(&documento, chave_loja, "documento").ok()
        } else {
            None
        };

        // `OR IGNORE` protege o boot contra documentos duplicados anteriores
        // ao UNIQUE: a linha problemática fica com o sentinela e é registrada.
        let alteradas = if let Some(ref dc) = doc_cifrado {
            conn.execute(
                "UPDATE OR IGNORE pessoas SET documento_blind = ?1, documento = ?2 WHERE id = ?3",
                params![blind, dc, id],
            )?
        } else {
            conn.execute(
                "UPDATE OR IGNORE pessoas SET documento_blind = ?1 WHERE id = ?2",
                params![blind, id],
            )?
        };

        if alteradas == 0 {
            tracing::warn!(
                pessoa_id = %id,
                "blind index não aplicado (documento duplicado ou já migrado)"
            );
        }
        atualizados += alteradas;
    }

    // Criptografia em repouso retroativa para telefones, e-mails e endereços legados (LGPD Art. 46)
    let legados: Vec<(String, Option<String>, Option<String>, Option<String>)> = {
        let mut stmt = conn.prepare(
            "SELECT id, telefone, email, endereco FROM pessoas
             WHERE (telefone IS NOT NULL AND telefone NOT LIKE 'ENC:%')
                OR (email IS NOT NULL AND email NOT LIKE 'ENC:%')
                OR (endereco IS NOT NULL AND endereco NOT LIKE 'ENC:%')",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for (id, tel, em, end) in legados {
        let tc = tel
            .as_deref()
            .and_then(|t| mikella_infra_crypto::cifrar_pii(t, chave_loja, "telefone").ok());
        let ec = em
            .as_deref()
            .and_then(|e| mikella_infra_crypto::cifrar_pii(e, chave_loja, "email").ok());
        let endc = end
            .as_deref()
            .and_then(|e| mikella_infra_crypto::cifrar_pii(e, chave_loja, "endereco").ok());
        let _ = conn.execute(
            "UPDATE pessoas SET
                telefone = COALESCE(?1, telefone),
                email = COALESCE(?2, email),
                endereco = COALESCE(?3, endereco)
             WHERE id = ?4",
            params![tc, ec, endc, id],
        );
    }

    if atualizados > 0 {
        tracing::info!(
            atualizados,
            "blind index de documentos atualizado a partir do sentinela"
        );
    }
    Ok(atualizados)
}

#[cfg(test)]
mod testes_blind_index {
    use super::*;
    use crate::Database;

    fn chave() -> [u8; 32] {
        [7u8; 32]
    }

    fn inserir_pendente(db: &Database, id: &str, documento: &str) {
        db.conn()
            .execute(
                "INSERT INTO pessoas (id, tipo, nome, documento, documento_blind, ativo)
                 VALUES (?1, 'CLIENTE', 'Cliente Teste', ?2, ?3, 1)",
                params![id, documento, format!("PENDENTE_{id}")],
            )
            .expect("inserir pessoa de teste");
    }

    #[test]
    fn backfill_substitui_sentinela_por_blind_index() {
        let db = Database::em_memoria().expect("banco em memória");
        inserir_pendente(&db, "p1", "123.456.789-01");

        let migrados = backfill_blind_index(db.conn(), &chave()).expect("backfill");
        assert_eq!(migrados, 1);

        let blind: String = db
            .conn()
            .query_row(
                "SELECT documento_blind FROM pessoas WHERE id = 'p1'",
                [],
                |row| row.get(0),
            )
            .expect("ler blind index");
        assert_eq!(blind, blind_index_documento("12345678901", &chave()));
    }

    #[test]
    fn backfill_e_idempotente() {
        let db = Database::em_memoria().expect("banco em memória");
        inserir_pendente(&db, "p1", "12345678901");

        assert_eq!(
            backfill_blind_index(db.conn(), &chave()).expect("1º backfill"),
            1
        );
        assert_eq!(
            backfill_blind_index(db.conn(), &chave()).expect("2º backfill"),
            0
        );
    }

    #[test]
    fn busca_por_cpf_completo_usa_blind_index() {
        let db = Database::em_memoria().expect("banco em memória");
        inserir_pendente(&db, "p1", "12345678901");
        backfill_blind_index(db.conn(), &chave()).expect("backfill");

        let repo = PessoaRepo::com_chave(db.conn(), &chave());
        let encontrados = repo
            .buscar_clientes_com_saldo(Some("12345678901"))
            .expect("busca por CPF");
        assert_eq!(encontrados.len(), 1);
        assert_eq!(encontrados[0].pessoa.id, "p1");

        let nao_encontrados = repo
            .buscar_clientes_com_saldo(Some("99999999999"))
            .expect("busca por CPF inexistente");
        assert!(nao_encontrados.is_empty());
    }

    #[test]
    fn criptografia_em_repouso_salva_cifrado_e_le_decifrado() {
        let db = Database::em_memoria().expect("banco em memória");
        let repo = PessoaRepo::com_chave(db.conn(), &chave());

        let p = Pessoa::novo_cliente(
            "p_crip".into(),
            "Cliente Criptografado".into(),
            Some("11988887777".into()),
            Some("teste@cliente.com".into()),
            Some("52998224725".into()),
            Some("Rua das Flores, 123".into()),
            chrono::Utc::now(),
        )
        .expect("novo cliente");

        repo.salvar(&p).expect("salvar com chave");

        // 1. Verifica no SQLite cru que os dados NÃO estão em texto claro
        let (doc_cru, tel_cru, email_cru, end_cru): (String, String, String, String) = db
            .conn()
            .query_row(
                "SELECT documento, telefone, email, endereco FROM pessoas WHERE id = 'p_crip'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("ler linha crua");

        assert!(doc_cru.starts_with("ENC:"));
        assert!(!doc_cru.contains("52998224725"));
        assert!(tel_cru.starts_with("ENC:"));
        assert!(!tel_cru.contains("11988887777"));
        assert!(email_cru.starts_with("ENC:"));
        assert!(!email_cru.contains("teste@cliente.com"));
        assert!(end_cru.starts_with("ENC:"));
        assert!(!end_cru.contains("Rua das Flores"));

        // 2. Verifica que o repositório lê e decifra transparentemente
        let lido = repo
            .buscar_por_id("p_crip")
            .expect("buscar")
            .expect("encontrado");
        assert_eq!(lido.documento.as_deref(), Some("52998224725"));
        assert_eq!(lido.telefone.as_deref(), Some("11988887777"));
        assert_eq!(lido.email.as_deref(), Some("teste@cliente.com"));
        assert_eq!(lido.endereco.as_deref(), Some("Rua das Flores, 123"));
    }

    #[test]
    fn anonimizar_cliente_apaga_pii_sem_deixar_sentinela_ilegivel() {
        let db = Database::em_memoria().expect("banco em memória");
        let repo = PessoaRepo::com_chave(db.conn(), &chave());

        let p = Pessoa::novo_cliente(
            "p_anon".into(),
            "Cliente a Anonimizar".into(),
            Some("11977776666".into()),
            Some("apagar@cliente.com".into()),
            Some("52998224725".into()),
            Some("Rua das Flores, 123".into()),
            chrono::Utc::now(),
        )
        .expect("novo cliente");
        repo.salvar(&p).expect("salvar");

        assert!(repo.anonimizar_cliente("p_anon").expect("anonimizar"));

        let (nome, doc, blind, tel, email, end): (
            String,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = db
            .conn()
            .query_row(
                "SELECT nome, documento, documento_blind, telefone, email, endereco
                   FROM pessoas WHERE id = 'p_anon'",
                [],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .expect("ler anonimizado");

        assert_eq!(nome, "[CLIENTE ANONIMIZADO]");
        assert!(
            doc.is_none(),
            "documento deve ser NULL, não ciphertext falso"
        );
        assert_eq!(blind, "ANON_p_anon");
        assert!(tel.is_none());
        assert!(email.is_none());
        assert!(end.is_none());

        // A leitura pelo repositório também não inventa dado nenhum.
        let lido = repo
            .buscar_por_id("p_anon")
            .expect("buscar")
            .expect("existe");
        assert!(lido.documento.is_none());
        assert!(lido.telefone.is_none());
    }
}
