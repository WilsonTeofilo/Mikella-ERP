-- Migration 0003: adiciona coluna `email` à tabela `usuarios`.
--
-- Suporta 2FA por e-mail em produção (SEG-011). Até o provider ser
-- definido, o store-server usa `username` como destino (ConsoleEmailBackend).
--
-- ALTER TABLE ... ADD COLUMN é idempotente por natureza em SQLite:
-- se a coluna já existe, a instrução falha. A migration runner
-- verifica o checksum antes de aplicar, então este arquivo só roda
-- uma vez.
ALTER TABLE usuarios ADD COLUMN email TEXT;
