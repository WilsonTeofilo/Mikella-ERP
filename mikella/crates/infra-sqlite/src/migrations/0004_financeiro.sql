-- ============================================================
-- Mikella ERP — Migração 0004: Contas a pagar / receber (RF-030)
-- ============================================================
-- Schema base para contas a pagar/receber já existe em 0001 (contas_pagar,
-- parcelas_pagar, contas_receber, parcelas_receber) com schema mais simples.
-- Esta migração:
--   1. Cria planos_conta (nova tabela)
--   2. Recria contas_pagar, contas_receber e parcelas com schema unificado
--
-- SQLite: DROP TABLE é seguro aqui porque a migração roda uma vez e os dados
-- financeiros são sempre reimportados/local-first (sem migration de produção
-- para MVP-1).

DROP TRIGGER IF EXISTS trg_contas_pagar_updated;
DROP TRIGGER IF EXISTS trg_contas_receber_updated;

DROP TABLE IF EXISTS parcelas;
DROP TABLE IF EXISTS contas_receber;
DROP TABLE IF EXISTS contas_pagar;
DROP TABLE IF EXISTS parcelas_pagar;
DROP TABLE IF EXISTS parcelas_receber;
DROP INDEX IF EXISTS idx_parcelas_pagar_vencimento;
DROP INDEX IF EXISTS idx_parcelas_receber_vencimento;
DROP INDEX IF EXISTS idx_contas_pagar_usuario;
DROP INDEX IF EXISTS idx_contas_receber_usuario;

-- Plano de contas (codigo + tipo formam PK)
CREATE TABLE planos_conta (
    codigo         TEXT    NOT NULL,
    nome           TEXT    NOT NULL,
    tipo           TEXT    NOT NULL CHECK (tipo IN ('PAGAMENTO','RECEBIMENTO')),
    PRIMARY KEY (codigo, tipo)
) STRICT;

-- Contas a pagar
CREATE TABLE contas_pagar (
    id              TEXT    PRIMARY KEY,  -- UUID
    descricao       TEXT    NOT NULL,
    plano_codigo    TEXT    NOT NULL,
    plano_tipo      TEXT    NOT NULL,
    usuario_id      TEXT    NOT NULL REFERENCES usuarios(id),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    FOREIGN KEY (plano_codigo, plano_tipo) REFERENCES planos_conta(codigo, tipo)
) STRICT;

CREATE INDEX idx_contas_pagar_usuario ON contas_pagar(usuario_id);

-- Contas a receber
CREATE TABLE contas_receber (
    id              TEXT    PRIMARY KEY,  -- UUID
    descricao       TEXT    NOT NULL,
    plano_codigo    TEXT    NOT NULL,
    plano_tipo      TEXT    NOT NULL,
    usuario_id      TEXT    NOT NULL REFERENCES usuarios(id),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    FOREIGN KEY (plano_codigo, plano_tipo) REFERENCES planos_conta(codigo, tipo)
) STRICT;

CREATE INDEX idx_contas_receber_usuario ON contas_receber(usuario_id);

-- Parcelas unificadas (compartilhadas entre pagar e receber)
CREATE TABLE parcelas (
    id              TEXT    PRIMARY KEY,  -- UUID
    conta_id        TEXT    NOT NULL,      -- FK para contas_pagar ou contas_receber
    conta_tipo      TEXT    NOT NULL CHECK (conta_tipo IN ('PAGAMENTO','RECEBIMENTO')),
    valor_centavos  INTEGER NOT NULL CHECK (valor_centavos >= 0),
    vencimento      TEXT    NOT NULL,     -- ISO-8601
    pago_em         TEXT,                 -- NULL se aberta
    status          TEXT    NOT NULL DEFAULT 'ABERTA' CHECK (status IN ('ABERTA','PAGA','CANCELADA')),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (conta_id, id)
) STRICT;

CREATE INDEX idx_parcelas_conta ON parcelas(conta_id, conta_tipo);
CREATE INDEX idx_parcelas_vencimento ON parcelas(vencimento) WHERE status = 'ABERTA';
CREATE INDEX idx_parcelas_status ON parcelas(status);

-- Triggers para atualizar updated_at
CREATE TRIGGER trg_contas_pagar_updated
AFTER UPDATE ON contas_pagar BEGIN
    UPDATE contas_pagar SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
    WHERE id = NEW.id;
END;

CREATE TRIGGER trg_contas_receber_updated
AFTER UPDATE ON contas_receber BEGIN
    UPDATE contas_receber SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
    WHERE id = NEW.id;
END;

CREATE TRIGGER trg_parcelas_updated
AFTER UPDATE ON parcelas BEGIN
    UPDATE parcelas SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
    WHERE id = NEW.id;
END;

-- Dados iniciais de planos de conta
INSERT OR IGNORE INTO planos_conta (codigo, nome, tipo) VALUES
    ('1.1.01', 'Caixa', 'RECEBIMENTO'),
    ('1.1.02', 'Bancos', 'RECEBIMENTO'),
    ('2.1.01', 'Fornecedores', 'PAGAMENTO'),
    ('2.1.02', 'Salários', 'PAGAMENTO'),
    ('2.1.03', 'Impostos', 'PAGAMENTO'),
    ('3.1.01', 'Vendas', 'RECEBIMENTO'),
    ('3.1.02', 'Prestacao de Servicos', 'RECEBIMENTO'),
    ('4.1.01', 'Comissoes', 'PAGAMENTO'),
    ('4.1.02', 'Despesas Operacionais', 'PAGAMENTO');
