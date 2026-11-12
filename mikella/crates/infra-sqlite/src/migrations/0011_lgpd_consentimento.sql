-- ============================================================
-- Mikella ERP — Migração 0011: consentimento de marketing (LGPD)
-- ============================================================
-- O caminho de anonimização (`PessoaRepo::anonimizar_cliente` e o expurgo
-- por retenção) já zera o consentimento de marketing, mas a coluna nunca
-- existiu: a atualização falhava em runtime. Aditiva e com default seguro (0).
--
-- Base legal: o consentimento é registrado de forma binária e específico para
-- marketing (Art. 8º da LGPD). A ausência de consentimento continua sendo o
-- padrão, inclusive para os clientes já cadastrados.

ALTER TABLE pessoas
    ADD COLUMN consentimento_marketing INTEGER NOT NULL DEFAULT 0
    CHECK (consentimento_marketing IN (0,1));
