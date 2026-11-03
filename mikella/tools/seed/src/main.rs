use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::Argon2;
use rand::rngs::OsRng;
use rusqlite::Connection;
use uuid::Uuid;

fn main() {
    let db = std::env::args().nth(1).unwrap();
    let argon2 = Argon2::default();

    // Gera salt aleatório usando OsRng (CSPRNG)
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2.hash_password(b"teste123", &salt).unwrap();
    let hash_str = hash.to_string();

    // Cria/o DB e roda migrations primeiro
    {
        let _db = mikella_infra_sqlite::Database::abrir(&db).unwrap();
    }

    let conn = Connection::open(&db).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON; BEGIN;")
        .unwrap();

    // UUIDs para teste E2E
    let account_id = Uuid::new_v4().to_string();
    let store_id = Uuid::new_v4().to_string();
    let usuario_id = Uuid::new_v4().to_string();
    let dispositivo_id = Uuid::new_v4().to_string();
    let produto_id = Uuid::new_v4().to_string();
    let lote_id = Uuid::new_v4().to_string();

    // Busca usuário existente ou cria novo
    let user_row: Option<(String,)> = conn
        .query_row(
            "SELECT id FROM usuarios WHERE username = 'teste'",
            [],
            |row| Ok((row.get(0)?,)),
        )
        .ok();

    let final_usuario_id = if let Some((uid,)) = user_row {
        conn.execute(
            "UPDATE usuarios SET senha_hash = ?1, ativo = 1 WHERE id = ?2",
            [&hash_str, &uid],
        ).unwrap();
        uid
    } else {
        conn.execute(
            "INSERT INTO usuarios (id, nome, username, senha_hash, email, papel, ativo) \
             VALUES (?1, 'Teste', 'teste', ?2, 'teste@test.local', 'DONO', 1)",
            [&usuario_id, &hash_str],
        ).unwrap();
        usuario_id
    };

    // Popula instance_identity para simular onboarding concluído
    conn.execute(
        "INSERT OR IGNORE INTO instance_identity (id, account_id, store_id, db_epoch, manifest_json) \
         VALUES (1, ?1, ?2, 1, '{}')",
        [&account_id, &store_id],
    )
    .unwrap();

    // dispositivo de caixa (para E2E de abertura/fechamento)
    conn.execute(
        "INSERT OR IGNORE INTO dispositivos \
         (id, nome, usuario_so, fingerprint, papel) \
         VALUES (?1, 'caixa-teste', 'test', 'seed', 'CAIXA')",
        [&dispositivo_id],
    )
    .unwrap();

    conn.execute_batch("COMMIT;").unwrap();
    println!("Seed OK - Usuário configurado:");
    println!("USER: teste");
    println!("SENHA: teste123");
    println!("PAPEL: DONO");
    println!("id: {final_usuario_id}");
}
