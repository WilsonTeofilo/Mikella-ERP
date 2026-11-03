-- ============================================================
-- Mikella ERP — Migração 0006: crediário e detalhe da forma de pagamento
-- ============================================================
-- Aditiva de propósito: a base local já pode conter vendas e contas em
-- produção/piloto. Nenhuma tabela é recriada.
--
-- 1. `contas_receber` ganha o vínculo com a pessoa (cliente do crediário) e
--    com o documento de origem (venda). As parcelas continuam na tabela
--    unificada `parcelas` (conta_tipo = 'RECEBIMENTO').
-- 2. VOUCHER e CREDIARIO são formas de pagamento distintas na operação, mas o
--    CHECK histórico de `vendas`/`venda_pagamentos` só aceita OUTRO. Em vez de
--    recriar tabelas com dados existentes (arriscado), gravamos `forma` como
--    OUTRO (valor legal no schema) e o detalhe nesta coluna nova.

ALTER TABLE contas_receber ADD COLUMN pessoa_id TEXT;
ALTER TABLE contas_receber ADD COLUMN referencia_id TEXT;
ALTER TABLE contas_receber ADD COLUMN referencia_tipo TEXT;

CREATE INDEX IF NOT EXISTS idx_contas_receber_pessoa ON contas_receber(pessoa_id);

ALTER TABLE vendas ADD COLUMN forma_pagamento_detalhe TEXT;
ALTER TABLE venda_pagamentos ADD COLUMN forma_detalhe TEXT;

-- A tabela unificada `parcelas` (migração 0004) não tinha número da parcela
-- nem a forma usada na liquidação. São colunas de apresentação/auditoria.
ALTER TABLE parcelas ADD COLUMN numero INTEGER NOT NULL DEFAULT 1;
ALTER TABLE parcelas ADD COLUMN forma_liquidacao TEXT;
