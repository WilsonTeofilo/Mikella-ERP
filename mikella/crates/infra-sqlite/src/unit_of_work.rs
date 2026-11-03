//! Unidade de trabalho para operações atômicas no domínio.
//!
//! Fornece um wrapper em torno de uma transação SQLite que permite registrar
//! múltiplas operações que devem ser executadas em conjunto: commit se todas
//! tiverem sucesso, rollback em caso de qualquer falha. Isso garante a
//! consistência do banco de dados para operações complexas do domínio.

use std::fmt;

use rusqlite::{Connection, Transaction};

use crate::error::InfraError;
use mikella_shared_kernel::error::DomainError;

/// Unit of work para operações atômicas.
pub struct UnitOfWork {
    conn: Connection,
    operations: Vec<Box<dyn Fn(&Connection) -> InfraError<()> + Send + Sync>>,
}

impl UnitOfWork {
    /// Inicia uma nova unidade de trabalho.
    pub fn new(conn: Connection) -> Self {
        Self {
            conn,
            operations: Vec::new(),
        }
    }
    
    /// Registra uma operação para execução durante o commit.
    pub fn execute<F>(&mut self, operation: F)
    where
        F: Fn(&Connection) -> InfraError<()> + Send + Sync,
    {
        self.operations.push(Box::new(operation));
    }
    
    /// Executa todas as operações registradas dentro de uma transação.
    ///
    /// # Retorna
    /// `Ok(())` se todas as operações forem bem-sucedidas e a transação for
    /// confirmada.
    ///
    /// # Erros
    /// Retorna o primeiro erro encontrado ou um erro de transação
    /// caso a transação não possa ser iniciada ou confirmada.
    pub fn commit(&mut self) -> InfraError<()> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| InfraError::from(e))?;
        
        for operation in &self.operations {
            operation(&tx).map_err(|e| {
                tx.rollback().ok();
                e
            })?;
        }
        
        tx.commit().map_err(|e| InfraError::from(e))
    }
    
    /// Obtém uma referência à conexão subjacente.
    ///
    /// Útil para repositórios que precisam de acesso direto ao SQLite.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

impl fmt::Display for UnitOfWork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UnitOfWork with {} operations", self.operations.len())
    }
}

/// Extensão para tipos `DomainResult` facilitar o uso.
pub trait UnitOfWorkExt {
    /// Registra uma operação no unit of work e executa imediatamente.
    fn execute(&mut self, operation: impl Fn(&Connection) -> DomainResult<()> + Send + Sync) -> DomainResult<()>;
}

impl<T> UnitOfWorkExt for T
where
    T: std::ops::Deref<Target = UnitOfWork>,
{
    fn execute(&mut self, operation: impl Fn(&Connection) -> DomainResult<()> + Send + Sync) -> DomainResult<()> {
        // Converter DomainResult para InfraError
        let infra_operation = move |conn: &Connection| {
            operation(conn).map_err(|e| InfraError::from(e))
        };
        
        self.operations.push(Box::new(infra_operation));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    
    #[tokio::test]
    async fn test_unit_of_work_success() {
        let conn = Connection::open(":memory:").unwrap();
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT)", ()).unwrap();
        
        let mut uow = UnitOfWork::new(conn);
        
        uow.execute(|conn| {
            conn.execute("INSERT INTO test (id, value) VALUES (1, 'test')", ()
        }).unwrap()
        });
        
        uow.commit().unwrap();
        
        // Verify data was inserted
        let conn = uow.connection();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM test").unwrap();
        let count: i64 = stmt.query_row([], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
    }
    
    #[tokio::test]
    async fn test_unit_of_work_failure_rollback() {
        let conn = Connection::open(":memory:").unwrap();
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT)", ()).unwrap();
        conn.execute("INSERT INTO test (id, value) VALUES (1, 'initial')", ()).unwrap();
        
        let mut uow = UnitOfWork::new(conn);
        
        uow.execute(|conn| {
            conn.execute("INSERT INTO test (id, value) VALUES (2, 'first')", ()
        }).unwrap()
        });
        
        uow.execute(|conn| {
            conn.execute("INSERT INTO test (id, value) VALUES (3, 'second')", ()
        }).unwrap()
        });
        
        // Fail on third operation
        let result = uow.execute(|conn| {
            conn.execute("INVALID SQL STATEMENT", ())
        });
        assert!(result.is_err());
        
        // Verify no data was inserted
        let conn = uow.connection();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM test").unwrap();
        let count: i64 = stmt.query_row([], |row| row.get(0)).unwrap();
        assert_eq!(count, 1); // Only initial row
    }
    
    #[tokio::test]
    async fn test_unit_of_work_conversion() {
        let conn = Connection::open(":memory:").unwrap();
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY)", ()).unwrap();
        
        let mut uow = UnitOfWork::new(conn);
        
        uow.execute(|conn| {
            conn.execute("INSERT INTO test (id) VALUES (1)", ()) // DomainResult para InfraResult
        }).unwrap();
        
        uow.commit().unwrap();
        
        // Verify
        let conn = uow.connection();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM test").unwrap();
        let count: i64 = stmt.query_row([], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}