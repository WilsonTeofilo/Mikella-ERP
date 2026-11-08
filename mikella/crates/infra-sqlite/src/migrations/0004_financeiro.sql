-- ============================================================
-- Mikella ERP — Migração 0004: Contas a pagar / receber (RF-030)
-- ============================================================
-- Schema base para contas a pagar/receber já existe em 0001 (contas_pagar,
-- parcelas_pagar, contas_receber, parcelas_receber) com schema mais simples.
-- Esta migração:
--   1. Cria planos_conta (nova tabela)
--   2. Converte contas/parcelas para o schema unificado SEM perder dados
--
-- DAT-001: a versão anterior fazia DROP TABLE nas tabelas financeiras e
-- descartava contas e parcelas de bancos em uso. A conversão abaixo cria as
-- tabelas novas, copia os dados antigos e só então remove as tabelas de
-- origem. Bancos que já aplicaram a versão antiga são reconhecidos pelo
-- runner (schema presente) e não executam este SQL novamente.

-- Plano de contas (codigo + tipo formam PK)
CREATE TABLE IF NOT EXISTS planos_conta (
    codigo         TEXT    NOT NULL,
    nome           TEXT    NOT NULL,
    tipo           TEXT    NOT NULL CHECK (tipo IN ('PAGAMENTO','RECEBIMENTO')),
    PRIMARY KEY (codigo, tipo)
) STRICT;

-- Dados iniciais de planos de conta.
-- 2.1.99/3.1.99 recebem contas antigas que não tinham plano de contas.
INSERT OR IGNORE INTO planos_conta (codigo, nome, tipo) VALUES
    ('1.1.01', 'Caixa', 'RECEBIMENTO'),
    ('1.1.02', 'Bancos', 'RECEBIMENTO'),
    ('2.1.01', 'Fornecedores', 'PAGAMENTO'),
    ('2.1.02', 'Salários', 'PAGAMENTO'),
    ('2.1.03', 'Impostos', 'PAGAMENTO'),
    ('2.1.99', 'Classificação pendente', 'PAGAMENTO'),
    ('3.1.01', 'Vendas', 'RECEBIMENTO'),
    ('3.1.02', 'Prestacao de Servicos', 'RECEBIMENTO'),
    ('3.1.99', 'Classificação pendente', 'RECEBIMENTO'),
    ('4.1.01', 'Comissoes', 'PAGAMENTO'),
    ('4.1.02', 'Despesas Operacionais', 'PAGAMENTO');

-- ── Contas a pagar: tabela nova + cópia ─────────────────────
CREATE TABLE contas_pagar_novo (
    id              TEXT    PRIMARY KEY,  -- UUID
    descricao       TEXT    NOT NULL,
    plano_codigo    TEXT    NOT NULL,
    plano_tipo      TEXT    NOT NULL,
    usuario_id      TEXT    NOT NULL REFERENCES usuarios(id),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    FOREIGN KEY (plano_codigo, plano_tipo) REFERENCES planos_conta(codigo, tipo)
) STRICT;

INSERT INTO contas_pagar_novo
    (id, descricao, plano_codigo, plano_tipo, usuario_id, created_at, updated_at)
SELECT c.id,
       c.descricao,
       '2.1.99',
       'PAGAMENTO',
       (SELECT u.id FROM usuarios u ORDER BY u.rowid LIMIT 1),
       c.created_at,
       c.created_at
FROM contas_pagar c
WHERE EXISTS (SELECT 1 FROM usuarios);

-- ── Contas a receber: tabela nova + cópia ───────────────────
CREATE TABLE contas_receber_novo (
    id              TEXT    PRIMARY KEY,  -- UUID
    descricao       TEXT    NOT NULL,
    plano_codigo    TEXT    NOT NULL,
    plano_tipo      TEXT    NOT NULL,
    usuario_id      TEXT    NOT NULL REFERENCES usuarios(id),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    FOREIGN KEY (plano_codigo, plano_tipo) REFERENCES planos_conta(codigo, tipo)
) STRICT;

INSERT INTO contas_receber_novo
    (id, descricao, plano_codigo, plano_tipo, usuario_id, created_at, updated_at)
SELECT c.id,
       c.descricao,
       '3.1.99',
       'RECEBIMENTO',
       (SELECT u.id FROM usuarios u ORDER BY u.rowid LIMIT 1),
       c.created_at,
       c.created_at
FROM contas_receber c
WHERE EXISTS (SELECT 1 FROM usuarios);

-- ── Parcelas unificadas: tabela nova + cópia ────────────────
CREATE TABLE parcelas_novo (
    id              TEXT    PRIMARY KEY,  -- UUID
    conta_id        TEXT    NOT NULL,     -- FK para contas_pagar ou contas_receber
    conta_tipo      TEXT    NOT NULL CHECK (conta_tipo IN ('PAGAMENTO','RECEBIMENTO')),
    valor_centavos  INTEGER NOT NULL CHECK (valor_centavos >= 0),
    vencimento      TEXT    NOT NULL,     -- ISO-8601
    pago_em         TEXT,                 -- NULL se aberta
    status          TEXT    NOT NULL DEFAULT 'ABERTA' CHECK (status IN ('ABERTA','PAGA','CANCELADA')),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (conta_id, id)
) STRICT;

-- Parcelas a pagar. Só entram as que têm conta preservada.
INSERT INTO parcelas_novo
    (id, conta_id, conta_tipo, valor_centavos, vencimento, pago_em, status, created_at, updated_at)
SELECT p.id,
       p.conta_id,
       'PAGAMENTO',
       p.valor_centavos,
       p.vencimento,
       p.pago_em,
       CASE p.estado
           WHEN 'PENDENTE' THEN 'ABERTA'
           WHEN 'PAGO'     THEN 'PAGA'
           ELSE 'CANCELADA'
       END,
       p.created_at,
       p.created_at
FROM parcelas_pagar p
WHERE EXISTS (SELECT 1 FROM contas_pagar_novo c WHERE c.id = p.conta_id);

-- Parcelas a receber.
INSERT INTO parcelas_novo
    (id, conta_id, conta_tipo, valor_centavos, vencimento, pago_em, status, created_at, updated_at)
SELECT p.id,
       p.conta_id,
       'RECEBIMENTO',
       p.valor_centavos,
       p.vencimento,
       p.recebido_em,
       CASE p.estado
           WHEN 'PENDENTE'  THEN 'ABERTA'
           WHEN 'RECEBIDO'  THEN 'PAGA'
           ELSE 'CANCELADA'
       END,
       p.created_at,
       p.created_at
FROM parcelas_receber p
WHERE EXISTS (SELECT 1 FROM contas_receber_novo c WHERE c.id = p.conta_id);

-- ── Troca atômica: remove origem somente depois da cópia ────
DROP TRIGGER IF EXISTS trg_contas_pagar_updated;
DROP TRIGGER IF EXISTS trg_contas_receber_updated;
DROP TABLE IF EXISTS parcelas_pagar;
DROP TABLE IF EXISTS parcelas_receber;
DROP TABLE IF EXISTS contas_pagar;
DROP TABLE IF EXISTS contas_receber;
DROP TABLE IF EXISTS parcelas;

ALTER TABLE contas_pagar_novo RENAME TO contas_pagar;
ALTER TABLE contas_receber_novo RENAME TO contas_receber;
ALTER TABLE parcelas_novo RENAME TO parcelas;

CREATE INDEX idx_contas_pagar_usuario ON contas_pagar(usuario_id);
CREATE INDEX idx_contas_receber_usuario ON contas_receber(usuario_id);
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
