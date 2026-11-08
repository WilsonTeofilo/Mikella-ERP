-- ============================================================
-- Mikella ERP — Migração 0010: Detalhes de Pagamento e Configuração da Loja
-- ============================================================
-- 1. Detalhes de cartão e maquininha em venda_pagamentos
--    Permite registrar nº de parcelas, maquininha, modalidade e taxa aplicada.
-- 2. Tabela loja_config para fuso horário IANA (ex: America/Sao_Paulo, America/Manaus)
-- 3. Índices por prefixo em pessoas (nome e telefone) para busca rápida (PERF-024)

-- Colunas de detalhe em venda_pagamentos
ALTER TABLE venda_pagamentos ADD COLUMN parcelas INTEGER NOT NULL DEFAULT 1;
ALTER TABLE venda_pagamentos ADD COLUMN maquininha TEXT;
ALTER TABLE venda_pagamentos ADD COLUMN modalidade TEXT;
ALTER TABLE venda_pagamentos ADD COLUMN taxa_centavos INTEGER NOT NULL DEFAULT 0;

-- Tabela de configuração da loja (fuso horário IANA, dados cadastrais)
CREATE TABLE IF NOT EXISTS loja_config (
    chave       TEXT    PRIMARY KEY,
    valor       TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

INSERT OR IGNORE INTO loja_config (chave, valor) VALUES ('fuso_horario', 'America/Sao_Paulo');
INSERT OR IGNORE INTO loja_config (chave, valor) VALUES ('nome_loja', 'Mikella Loja');

-- Índices de prefixo para busca de clientes (PERF-024)
CREATE INDEX IF NOT EXISTS idx_pessoas_nome_prefixo ON pessoas(nome COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_pessoas_telefone_prefixo ON pessoas(telefone);
