-- ============================================================
-- Mikella ERP — Migração 0002: app_preferences (key-value)
-- ============================================================
-- Tabela simples key-value para preferências da aplicação local.
-- Usada pelo store-server para tracking de prompts (ex: donation).
-- ============================================================

CREATE TABLE app_preferences (
    chave       TEXT    PRIMARY KEY,
    valor       TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- Indice de atualização mais recente para invalidação de cache.
CREATE INDEX IF NOT EXISTS idx_app_preferences_updated
    ON app_preferences(updated_at);
