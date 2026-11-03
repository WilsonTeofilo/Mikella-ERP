-- ============================================================
-- Mikella ERP — Migração 0001: schema inicial
-- Banco da loja (local, no PC servidor da loja)
-- ============================================================
-- REGRAS INVIOLÁVEIS
--   • Todo dinheiro em CENTAVOS (INTEGER). Zero float.
--   • Movimentações são IMUTÁVEIS. UPDATE/DELETE bloqueados por trigger.
--   • Saldos são PROJEÇÕES calculadas na mesma transação.
--   • Toda tabela é STRICT (tipos checados pelo SQLite).
--   • Nenhum dado é apagado fisicamente; produto arquiva.
--
-- PRAGMAs de conexão (WAL, synchronous=FULL, foreign_keys, busy_timeout)
-- NÃO ficam aqui: são aplicados a cada conexão em `db.rs`.
-- ============================================================

-- ============================================================
-- METADADOS DA INSTÂNCIA (identidade criptográfica da loja)
-- ============================================================
CREATE TABLE instance_identity (
    id              INTEGER PRIMARY KEY CHECK (id = 1), -- única linha
    account_id      TEXT    NOT NULL,   -- UUID da conta na nuvem
    store_id        TEXT    NOT NULL,   -- UUID da loja
    db_epoch        INTEGER NOT NULL DEFAULT 1, -- incrementa a cada migração de servidor
    manifest_json   TEXT    NOT NULL,   -- manifesto assinado pela nuvem (JSON+Ed25519)
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- ============================================================
-- CONTROLE DE MIGRAÇÕES
-- ============================================================
CREATE TABLE IF NOT EXISTS schema_migrations (
    versao          TEXT    PRIMARY KEY,
    checksum        TEXT    NOT NULL,   -- blake3 do arquivo de migração
    aplicada_em     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- ============================================================
-- USUÁRIOS E PAPÉIS
-- ============================================================
CREATE TABLE usuarios (
    id              TEXT    PRIMARY KEY,  -- UUID
    nome            TEXT    NOT NULL,
    username        TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    senha_hash      TEXT    NOT NULL,     -- Argon2id; nunca texto claro em log
    papel           TEXT    NOT NULL CHECK (papel IN ('DONO','GERENTE','OPERADOR_CAIXA','ESTOQUISTA','VENDEDOR')),
    pin_hash        TEXT,                 -- PIN de 4-6 dígitos (Argon2id) para aprovação no balcão
    ativo           INTEGER NOT NULL DEFAULT 1 CHECK (ativo IN (0,1)),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- Capacidades granulares (ex: 'sale.cancel', 'stock.adjust', 'finance.view')
CREATE TABLE usuario_capacidades (
    usuario_id      TEXT    NOT NULL REFERENCES usuarios(id),
    capacidade      TEXT    NOT NULL,
    PRIMARY KEY (usuario_id, capacidade)
) STRICT;

-- Dispositivos pareados (máx. 8 por loja; regra aplicada no servidor/nuvem)
CREATE TABLE dispositivos (
    id              TEXT    PRIMARY KEY,  -- UUID gerado no pareamento
    nome            TEXT    NOT NULL,     -- hostname do SO
    usuario_so      TEXT    NOT NULL,
    fingerprint     TEXT    NOT NULL,     -- hash de identificadores (sinal, não identidade)
    papel           TEXT    NOT NULL CHECK (papel IN ('SERVIDOR','CAIXA')),
    ativo           INTEGER NOT NULL DEFAULT 1 CHECK (ativo IN (0,1)),
    ultimo_acesso   TEXT,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- Sessões de admin (sessão única por admin — RF-042)
CREATE TABLE sessoes_admin (
    id              TEXT    PRIMARY KEY,  -- UUID do token de sessão
    usuario_id      TEXT    NOT NULL REFERENCES usuarios(id),
    dispositivo_id  TEXT    NOT NULL REFERENCES dispositivos(id),
    criada_em       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    expira_em       TEXT    NOT NULL,
    revogada        INTEGER NOT NULL DEFAULT 0 CHECK (revogada IN (0,1)),
    revogada_em     TEXT
) STRICT;

CREATE INDEX idx_sessoes_admin_ativas ON sessoes_admin(usuario_id) WHERE revogada = 0;

-- ============================================================
-- LICENÇA / ENTITLEMENTS
-- ============================================================
CREATE TABLE licenca_ativa (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    token_raw       TEXT    NOT NULL,    -- token assinado pela nuvem (Ed25519)
    plano           TEXT    NOT NULL CHECK (plano IN ('GRATUITO','PAGO')),
    estado          TEXT    NOT NULL CHECK (estado IN ('ATIVO','EM_TOLERANCIA','SUSPENSO','CANCELADO','RETENCAO')),
    valid_until     TEXT    NOT NULL,    -- ISO 8601
    offline_grace   TEXT    NOT NULL,    -- até quando opera offline
    entitlements    TEXT    NOT NULL,    -- JSON: cotas, módulos, limites
    ultimo_server_time TEXT NOT NULL,    -- server_time assinado mais recente
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- Cotas mensais de produto — encadeadas por hash (FRD-020)
CREATE TABLE quota_events (
    id              TEXT    PRIMARY KEY,
    store_id        TEXT    NOT NULL,
    tipo            TEXT    NOT NULL CHECK (tipo IN ('CRIACAO','EDICAO','ARQUIVAMENTO')),
    periodo         TEXT    NOT NULL,    -- 'AAAA-MM'
    usuario_id      TEXT    NOT NULL REFERENCES usuarios(id),
    produto_id      TEXT,
    prev_hash       TEXT    NOT NULL,
    hash            TEXT    NOT NULL UNIQUE,
    criado_em       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE TRIGGER quota_events_no_update BEFORE UPDATE ON quota_events
BEGIN SELECT RAISE(ABORT, 'quota_events: UPDATE proibido'); END;
CREATE TRIGGER quota_events_no_delete BEFORE DELETE ON quota_events
BEGIN SELECT RAISE(ABORT, 'quota_events: DELETE proibido'); END;
CREATE INDEX idx_quota_periodo ON quota_events(store_id, periodo);

-- ============================================================
-- CATEGORIAS
-- ============================================================
CREATE TABLE categorias (
    id              TEXT    PRIMARY KEY,
    nome            TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    pai_id          TEXT    REFERENCES categorias(id),  -- máx. 2 níveis
    ativo           INTEGER NOT NULL DEFAULT 1 CHECK (ativo IN (0,1)),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- ============================================================
-- PRODUTOS
-- ============================================================
CREATE TABLE produtos (
    id              TEXT    PRIMARY KEY,
    codigo_barras   TEXT    UNIQUE,
    sku             TEXT    UNIQUE,
    nome            TEXT    NOT NULL,
    categoria_id    TEXT    REFERENCES categorias(id),
    unidade         TEXT    NOT NULL DEFAULT 'UN',  -- UN, KG, L, CX, PCT
    preco_centavos  INTEGER NOT NULL CHECK (preco_centavos >= 0),
    custo_centavos  INTEGER NOT NULL DEFAULT 0 CHECK (custo_centavos >= 0),
    estoque_minimo  INTEGER NOT NULL DEFAULT 0 CHECK (estoque_minimo >= 0),
    perecivel       INTEGER NOT NULL DEFAULT 0 CHECK (perecivel IN (0,1)),
    alerta_validade_dias INTEGER NOT NULL DEFAULT 60 CHECK (alerta_validade_dias >= 0),
    arquivado       INTEGER NOT NULL DEFAULT 0 CHECK (arquivado IN (0,1)),
    arquivado_em    TEXT,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE INDEX idx_produtos_codigo_barras ON produtos(codigo_barras) WHERE arquivado = 0;
CREATE INDEX idx_produtos_sku           ON produtos(sku)           WHERE arquivado = 0;
CREATE INDEX idx_produtos_categoria     ON produtos(categoria_id)  WHERE arquivado = 0;
CREATE INDEX idx_produtos_nome          ON produtos(nome)          WHERE arquivado = 0;

-- FTS5 para busca por nome (PERF-024). Tabela virtual não aceita STRICT.
CREATE VIRTUAL TABLE produtos_fts USING fts5(
    id UNINDEXED,
    nome,
    codigo_barras,
    sku,
    content='produtos',
    content_rowid='rowid'
);

CREATE TRIGGER produtos_fts_insert AFTER INSERT ON produtos BEGIN
    INSERT INTO produtos_fts(rowid, id, nome, codigo_barras, sku)
    VALUES (new.rowid, new.id, new.nome, new.codigo_barras, new.sku);
END;

CREATE TRIGGER produtos_fts_update AFTER UPDATE ON produtos BEGIN
    INSERT INTO produtos_fts(produtos_fts, rowid, id, nome, codigo_barras, sku)
    VALUES ('delete', old.rowid, old.id, old.nome, old.codigo_barras, old.sku);
    INSERT INTO produtos_fts(rowid, id, nome, codigo_barras, sku)
    VALUES (new.rowid, new.id, new.nome, new.codigo_barras, new.sku);
END;

CREATE TRIGGER produtos_fts_delete AFTER DELETE ON produtos BEGIN
    INSERT INTO produtos_fts(produtos_fts, rowid, id, nome, codigo_barras, sku)
    VALUES ('delete', old.rowid, old.id, old.nome, old.codigo_barras, old.sku);
END;

-- ============================================================
-- LOTES E VALIDADE
-- ============================================================
CREATE TABLE lotes (
    id              TEXT    PRIMARY KEY,
    produto_id      TEXT    NOT NULL REFERENCES produtos(id),
    numero_lote     TEXT,
    validade        TEXT,               -- 'AAAA-MM-DD'; NULL = sem validade
    quantidade      INTEGER NOT NULL DEFAULT 0 CHECK (quantidade >= 0), -- projeção
    custo_centavos  INTEGER NOT NULL DEFAULT 0 CHECK (custo_centavos >= 0),
    ativo           INTEGER NOT NULL DEFAULT 1 CHECK (ativo IN (0,1)),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE INDEX idx_lotes_produto  ON lotes(produto_id) WHERE ativo = 1;
CREATE INDEX idx_lotes_validade ON lotes(validade)   WHERE ativo = 1 AND validade IS NOT NULL;

-- Alertas de validade (persistem até confirmação explícita — RF-023)
CREATE TABLE alertas_validade (
    id              TEXT    PRIMARY KEY,
    lote_id         TEXT    NOT NULL REFERENCES lotes(id),
    produto_id      TEXT    NOT NULL REFERENCES produtos(id),
    marco_dias      INTEGER NOT NULL,   -- 60, 30, 7, 0 (vencido)
    lida            INTEGER NOT NULL DEFAULT 0 CHECK (lida IN (0,1)),
    lida_por        TEXT    REFERENCES usuarios(id),
    lida_em         TEXT,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (lote_id, marco_dias)
) STRICT;

CREATE INDEX idx_alertas_pendentes ON alertas_validade(produto_id) WHERE lida = 0;

-- ============================================================
-- MOVIMENTAÇÕES DE ESTOQUE (imutáveis)
-- ============================================================
CREATE TABLE movimentacoes_estoque (
    id              TEXT    PRIMARY KEY,
    produto_id      TEXT    NOT NULL REFERENCES produtos(id),
    lote_id         TEXT    REFERENCES lotes(id),
    tipo            TEXT    NOT NULL CHECK (tipo IN (
                        'ENTRADA_COMPRA','ENTRADA_DEVOLUCAO','ENTRADA_AJUSTE',
                        'SAIDA_VENDA','SAIDA_PERDA','SAIDA_AJUSTE',
                        'INVENTARIO'
                    )),
    quantidade      INTEGER NOT NULL CHECK (
                        (tipo IN ('ENTRADA_COMPRA','ENTRADA_DEVOLUCAO','ENTRADA_AJUSTE') AND quantidade > 0)
                        OR (tipo IN ('SAIDA_VENDA','SAIDA_PERDA','SAIDA_AJUSTE') AND quantidade < 0)
                        OR (tipo = 'INVENTARIO' AND quantidade <> 0)
                    ),
    custo_unitario_centavos INTEGER NOT NULL DEFAULT 0 CHECK (custo_unitario_centavos >= 0),
    referencia_id   TEXT,
    referencia_tipo TEXT,
    usuario_id      TEXT    NOT NULL REFERENCES usuarios(id),
    motivo          TEXT,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE TRIGGER movimentacoes_estoque_no_update BEFORE UPDATE ON movimentacoes_estoque
BEGIN SELECT RAISE(ABORT, 'movimentacoes_estoque: UPDATE proibido'); END;
CREATE TRIGGER movimentacoes_estoque_no_delete BEFORE DELETE ON movimentacoes_estoque
BEGIN SELECT RAISE(ABORT, 'movimentacoes_estoque: DELETE proibido'); END;

CREATE INDEX idx_mov_estoque_produto ON movimentacoes_estoque(produto_id, created_at DESC);
CREATE INDEX idx_mov_estoque_ref     ON movimentacoes_estoque(referencia_id) WHERE referencia_id IS NOT NULL;

-- Projeção do saldo do lote, mantida na mesma transação. O CHECK (>= 0)
-- em lotes.quantidade aborta a transação se a saída deixar o lote negativo.
CREATE TRIGGER mov_estoque_atualiza_lote
AFTER INSERT ON movimentacoes_estoque
WHEN new.lote_id IS NOT NULL
BEGIN
    UPDATE lotes
       SET quantidade = quantidade + new.quantidade
     WHERE id = new.lote_id;
END;

-- ============================================================
-- SESSÕES DE CAIXA
-- ============================================================
CREATE TABLE sessoes_caixa (
    id              TEXT    PRIMARY KEY,
    numero_caixa    INTEGER NOT NULL CHECK (numero_caixa BETWEEN 1 AND 8),
    dispositivo_id  TEXT    NOT NULL REFERENCES dispositivos(id),
    operador_id     TEXT    NOT NULL REFERENCES usuarios(id),
    aberta_em       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    fechada_em      TEXT,
    estado          TEXT    NOT NULL DEFAULT 'ABERTA' CHECK (estado IN ('ABERTA','EM_FECHAMENTO','FECHADA')),
    -- Projeções reconciliáveis com o livro (RES-010)
    saldo_inicial_centavos   INTEGER NOT NULL DEFAULT 0 CHECK (saldo_inicial_centavos >= 0),
    saldo_esperado_centavos  INTEGER NOT NULL DEFAULT 0,
    saldo_contado_centavos   INTEGER,
    diferenca_centavos       INTEGER,
    fechamento_cego          INTEGER NOT NULL DEFAULT 0 CHECK (fechamento_cego IN (0,1))
) STRICT;

-- No máximo UMA sessão aberta/em fechamento por caixa físico (RF-012/RES-012)
CREATE UNIQUE INDEX idx_sessoes_caixa_uma_aberta
    ON sessoes_caixa(dispositivo_id, numero_caixa)
    WHERE estado <> 'FECHADA';

CREATE INDEX idx_sessoes_caixa_aberta ON sessoes_caixa(estado) WHERE estado = 'ABERTA';

-- ============================================================
-- LIVRO DO CAIXA — MOVIMENTAÇÕES IMUTÁVEIS (RES-010)
-- ============================================================
CREATE TABLE livro_caixa (
    id              TEXT    PRIMARY KEY,
    sessao_id       TEXT    NOT NULL REFERENCES sessoes_caixa(id),
    sequencia       INTEGER NOT NULL CHECK (sequencia > 0), -- ordem determinística da cadeia
    tipo            TEXT    NOT NULL CHECK (tipo IN (
                        'ABERTURA','VENDA_DINHEIRO','VENDA_DEBITO','VENDA_CREDITO',
                        'VENDA_PIX','VENDA_OUTRO','SUPRIMENTO','SANGRIA',
                        'ESTORNO','CANCELAMENTO','FECHAMENTO'
                    )),
    valor_centavos  INTEGER NOT NULL,   -- + entrada, - saída
    troco_centavos  INTEGER NOT NULL DEFAULT 0 CHECK (troco_centavos >= 0),
    referencia_id   TEXT,
    referencia_tipo TEXT,
    usuario_id      TEXT    NOT NULL REFERENCES usuarios(id),
    aprovador_id    TEXT    REFERENCES usuarios(id),
    motivo          TEXT,
    prev_hash       TEXT    NOT NULL,
    hash            TEXT    NOT NULL UNIQUE,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (sessao_id, sequencia)
) STRICT;

CREATE TRIGGER livro_caixa_no_update BEFORE UPDATE ON livro_caixa
BEGIN SELECT RAISE(ABORT, 'livro_caixa: UPDATE proibido'); END;
CREATE TRIGGER livro_caixa_no_delete BEFORE DELETE ON livro_caixa
BEGIN SELECT RAISE(ABORT, 'livro_caixa: DELETE proibido'); END;

CREATE INDEX idx_livro_sessao ON livro_caixa(sessao_id, sequencia ASC);

-- Só uma abertura por sessão, e nenhuma entrada depois do fechamento.
CREATE UNIQUE INDEX idx_livro_uma_abertura
    ON livro_caixa(sessao_id) WHERE tipo = 'ABERTURA';

CREATE TRIGGER livro_sem_entrada_apos_fechamento
BEFORE INSERT ON livro_caixa
WHEN EXISTS (
    SELECT 1 FROM livro_caixa
    WHERE sessao_id = new.sessao_id AND tipo = 'FECHAMENTO'
)
BEGIN
    SELECT RAISE(ABORT, 'livro_caixa: sessão já foi fechada');
END;

-- Projeção do saldo da sessão, atualizada na MESMA transação do insert.
-- ABERTURA e FECHAMENTO ficam de fora: a abertura já está em saldo_inicial,
-- e o fechamento é informativo (contagem), não dinheiro novo.
CREATE TRIGGER livro_atualiza_saldo_sessao
AFTER INSERT ON livro_caixa
WHEN new.tipo IN ('VENDA_DINHEIRO','SUPRIMENTO','SANGRIA','ESTORNO','CANCELAMENTO')
BEGIN
    UPDATE sessoes_caixa
       SET saldo_esperado_centavos =
           saldo_esperado_centavos + new.valor_centavos - new.troco_centavos
     WHERE id = new.sessao_id;
END;

-- ============================================================
-- VENDAS
-- ============================================================
CREATE TABLE vendas (
    id              TEXT    PRIMARY KEY,  -- UUID gerado ANTES da gravação (idempotência)
    sessao_id       TEXT    NOT NULL REFERENCES sessoes_caixa(id),
    dispositivo_id  TEXT    NOT NULL REFERENCES dispositivos(id),
    operador_id     TEXT    NOT NULL REFERENCES usuarios(id),
    vendedor_id     TEXT    REFERENCES usuarios(id),
    comanda_id      TEXT,
    estado          TEXT    NOT NULL DEFAULT 'FINALIZADA' CHECK (estado IN ('FINALIZADA','CANCELADA','ESTORNADA')),
    forma_pagamento TEXT    NOT NULL CHECK (forma_pagamento IN ('DINHEIRO','DEBITO','CREDITO','PIX','MISTO','OUTRO')),
    subtotal_centavos  INTEGER NOT NULL CHECK (subtotal_centavos >= 0),
    desconto_centavos  INTEGER NOT NULL DEFAULT 0 CHECK (desconto_centavos >= 0),
    total_centavos     INTEGER NOT NULL CHECK (total_centavos > 0),
    recebido_centavos  INTEGER NOT NULL DEFAULT 0 CHECK (recebido_centavos >= 0),
    troco_centavos     INTEGER NOT NULL DEFAULT 0 CHECK (troco_centavos >= 0),
    calculado_em    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE INDEX idx_vendas_sessao    ON vendas(sessao_id, created_at DESC);
CREATE INDEX idx_vendas_operador  ON vendas(operador_id, created_at DESC);
CREATE INDEX idx_vendas_estado    ON vendas(estado, created_at DESC);
CREATE INDEX idx_vendas_data      ON vendas(created_at);

-- Itens da venda (imutáveis — correção via estorno)
CREATE TABLE venda_itens (
    id                  TEXT    PRIMARY KEY,
    venda_id            TEXT    NOT NULL REFERENCES vendas(id),
    produto_id          TEXT    NOT NULL REFERENCES produtos(id),
    lote_id             TEXT    REFERENCES lotes(id),
    quantidade          INTEGER NOT NULL CHECK (quantidade > 0),
    preco_unitario_centavos INTEGER NOT NULL CHECK (preco_unitario_centavos >= 0),
    desconto_centavos   INTEGER NOT NULL DEFAULT 0 CHECK (desconto_centavos >= 0),
    subtotal_centavos   INTEGER NOT NULL CHECK (subtotal_centavos >= 0),
    comissao_regra_id   TEXT,
    comissao_centavos   INTEGER NOT NULL DEFAULT 0 CHECK (comissao_centavos >= 0),
    created_at          TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE TRIGGER venda_itens_no_update BEFORE UPDATE ON venda_itens
BEGIN SELECT RAISE(ABORT, 'venda_itens: UPDATE proibido'); END;
CREATE TRIGGER venda_itens_no_delete BEFORE DELETE ON venda_itens
BEGIN SELECT RAISE(ABORT, 'venda_itens: DELETE proibido'); END;

CREATE INDEX idx_venda_itens_venda   ON venda_itens(venda_id);
CREATE INDEX idx_venda_itens_produto ON venda_itens(produto_id, created_at DESC);

-- Pagamentos da venda (RF-011: pagamento misto é obrigatório)
CREATE TABLE venda_pagamentos (
    id                  TEXT    PRIMARY KEY,
    venda_id            TEXT    NOT NULL REFERENCES vendas(id),
    forma               TEXT    NOT NULL CHECK (forma IN ('DINHEIRO','DEBITO','CREDITO','PIX','OUTRO')),
    valor_centavos      INTEGER NOT NULL CHECK (valor_centavos > 0),
    recebido_centavos   INTEGER CHECK (recebido_centavos IS NULL OR recebido_centavos > 0),
    troco_centavos      INTEGER NOT NULL DEFAULT 0 CHECK (troco_centavos >= 0),
    created_at          TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    -- Só dinheiro tem valor recebido; troco nunca é negativo
    CHECK (
        (forma = 'DINHEIRO' AND recebido_centavos IS NOT NULL AND recebido_centavos >= valor_centavos)
        OR (forma <> 'DINHEIRO' AND recebido_centavos IS NULL AND troco_centavos = 0)
    )
) STRICT;

CREATE TRIGGER venda_pagamentos_no_update BEFORE UPDATE ON venda_pagamentos
BEGIN SELECT RAISE(ABORT, 'venda_pagamentos: UPDATE proibido'); END;
CREATE TRIGGER venda_pagamentos_no_delete BEFORE DELETE ON venda_pagamentos
BEGIN SELECT RAISE(ABORT, 'venda_pagamentos: DELETE proibido'); END;

CREATE INDEX idx_venda_pagamentos_venda ON venda_pagamentos(venda_id);

-- Rascunho de venda em andamento no caixa (RES-020/021 — sobrevive a queda de energia)
CREATE TABLE rascunhos_venda (
    id              TEXT    PRIMARY KEY,  -- mesmo UUID que será a venda final
    dispositivo_id  TEXT    NOT NULL REFERENCES dispositivos(id),
    sessao_id       TEXT    NOT NULL REFERENCES sessoes_caixa(id),
    operador_id     TEXT    NOT NULL REFERENCES usuarios(id),
    dados_json      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- ============================================================
-- IDEMPOTÊNCIA GENÉRICA (RES-004)
-- ============================================================
CREATE TABLE operacoes_idempotentes (
    chave           TEXT    PRIMARY KEY,  -- UUID gerado no cliente antes do envio
    tipo            TEXT    NOT NULL,     -- 'VENDA', 'SANGRIA', 'ESTORNO'
    resultado_json  TEXT    NOT NULL,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- ============================================================
-- OUTBOX DE EVENTOS DE DOMÍNIO (03 — eventos + outbox)
-- ============================================================
CREATE TABLE outbox_eventos (
    id              TEXT    PRIMARY KEY,
    tipo            TEXT    NOT NULL,     -- 'VendaFinalizada', 'ProdutoCriado'
    agregado_id     TEXT    NOT NULL,
    payload_json    TEXT    NOT NULL,
    publicado_em    TEXT,
    tentativas      INTEGER NOT NULL DEFAULT 0 CHECK (tentativas >= 0),
    ultimo_erro     TEXT,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE INDEX idx_outbox_pendentes ON outbox_eventos(created_at) WHERE publicado_em IS NULL;

-- ============================================================
-- CLIENTES E FORNECEDORES
-- ============================================================
CREATE TABLE pessoas (
    id              TEXT    PRIMARY KEY,
    tipo            TEXT    NOT NULL CHECK (tipo IN ('CLIENTE','FORNECEDOR','AMBOS')),
    nome            TEXT    NOT NULL,
    documento_tipo  TEXT    CHECK (documento_tipo IS NULL OR documento_tipo IN ('CPF','CNPJ')),
    documento_blind TEXT    UNIQUE,     -- HMAC-SHA256(documento, chave_da_loja)
    telefone        TEXT,
    email           TEXT,
    ativo           INTEGER NOT NULL DEFAULT 1 CHECK (ativo IN (0,1)),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- ============================================================
-- FINANCEIRO
-- ============================================================
CREATE TABLE contas_pagar (
    id              TEXT    PRIMARY KEY,
    descricao       TEXT    NOT NULL,
    fornecedor_id   TEXT    REFERENCES pessoas(id),
    referencia_id   TEXT,
    referencia_tipo TEXT,
    total_centavos  INTEGER NOT NULL CHECK (total_centavos > 0),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE TABLE parcelas_pagar (
    id              TEXT    PRIMARY KEY,
    conta_id        TEXT    NOT NULL REFERENCES contas_pagar(id),
    numero          INTEGER NOT NULL CHECK (numero > 0),
    vencimento      TEXT    NOT NULL,   -- 'AAAA-MM-DD'
    valor_centavos  INTEGER NOT NULL CHECK (valor_centavos > 0),
    estado          TEXT    NOT NULL DEFAULT 'PENDENTE' CHECK (estado IN ('PENDENTE','PAGO','CANCELADO')),
    pago_em         TEXT,
    pago_por        TEXT    REFERENCES usuarios(id),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (conta_id, numero)
) STRICT;

CREATE INDEX idx_parcelas_pagar_vencimento ON parcelas_pagar(vencimento, estado);

CREATE TABLE contas_receber (
    id              TEXT    PRIMARY KEY,
    descricao       TEXT    NOT NULL,
    cliente_id      TEXT    REFERENCES pessoas(id),
    referencia_id   TEXT,
    referencia_tipo TEXT,
    total_centavos  INTEGER NOT NULL CHECK (total_centavos > 0),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE TABLE parcelas_receber (
    id              TEXT    PRIMARY KEY,
    conta_id        TEXT    NOT NULL REFERENCES contas_receber(id),
    numero          INTEGER NOT NULL CHECK (numero > 0),
    vencimento      TEXT    NOT NULL,
    valor_centavos  INTEGER NOT NULL CHECK (valor_centavos > 0),
    estado          TEXT    NOT NULL DEFAULT 'PENDENTE' CHECK (estado IN ('PENDENTE','RECEBIDO','CANCELADO')),
    recebido_em     TEXT,
    recebido_por    TEXT    REFERENCES usuarios(id),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (conta_id, numero)
) STRICT;

CREATE INDEX idx_parcelas_receber_vencimento ON parcelas_receber(vencimento, estado);

-- ============================================================
-- AUDITORIA (encadeada por hash — FRD-053)
-- ============================================================
CREATE TABLE auditoria (
    id              TEXT    PRIMARY KEY,
    entidade        TEXT    NOT NULL,
    entidade_id     TEXT    NOT NULL,
    acao            TEXT    NOT NULL,
    usuario_id      TEXT    REFERENCES usuarios(id),
    dispositivo_id  TEXT    REFERENCES dispositivos(id),
    dados_antes     TEXT,
    dados_depois    TEXT,
    prev_hash       TEXT    NOT NULL,
    hash            TEXT    NOT NULL UNIQUE,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE TRIGGER auditoria_no_update BEFORE UPDATE ON auditoria
BEGIN SELECT RAISE(ABORT, 'auditoria: UPDATE proibido'); END;
CREATE TRIGGER auditoria_no_delete BEFORE DELETE ON auditoria
BEGIN SELECT RAISE(ABORT, 'auditoria: DELETE proibido'); END;

CREATE INDEX idx_auditoria_entidade ON auditoria(entidade, entidade_id, created_at DESC);
CREATE INDEX idx_auditoria_usuario  ON auditoria(usuario_id, created_at DESC);

-- ============================================================
-- BACKUP / SYNC
-- ============================================================
CREATE TABLE backup_outbox (
    id              TEXT    PRIMARY KEY,
    periodo         TEXT    NOT NULL,   -- 'AAAA-MM'
    chunk_hash      TEXT    NOT NULL,
    estado          TEXT    NOT NULL DEFAULT 'PENDENTE' CHECK (estado IN ('PENDENTE','ENVIANDO','ENVIADO','ERRO')),
    tentativas      INTEGER NOT NULL DEFAULT 0 CHECK (tentativas >= 0),
    proximo_retry   TEXT,
    enviado_em      TEXT,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

CREATE INDEX idx_backup_pendentes ON backup_outbox(estado, proximo_retry);

CREATE TABLE backup_manifest (
    id              TEXT    PRIMARY KEY,
    periodo         TEXT    NOT NULL UNIQUE,
    chunk_hash      TEXT    NOT NULL,
    manifest_hash   TEXT    NOT NULL,
    prev_hash       TEXT    NOT NULL,
    r2_key          TEXT,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;

-- ============================================================
-- VIEWS (não substituem o cálculo no domínio)
-- ============================================================

-- Saldo atual de estoque por produto (derivado das movimentações)
CREATE VIEW vw_estoque_atual AS
SELECT
    p.id            AS produto_id,
    p.nome          AS produto_nome,
    p.codigo_barras,
    p.sku,
    p.estoque_minimo,
    COALESCE(SUM(me.quantidade), 0) AS saldo_atual,
    CASE WHEN COALESCE(SUM(me.quantidade), 0) <= p.estoque_minimo THEN 1 ELSE 0 END AS abaixo_minimo
FROM produtos p
LEFT JOIN movimentacoes_estoque me ON me.produto_id = p.id
WHERE p.arquivado = 0
GROUP BY p.id;

-- Saldo esperado do caixa (derivado do livro — RES-010).
-- Somente movimentações que afetam dinheiro físico; FECHAMENTO é informativo.
CREATE VIEW vw_saldo_caixa AS
SELECT
    sc.id           AS sessao_id,
    sc.numero_caixa,
    sc.operador_id,
    sc.estado,
    sc.aberta_em,
    COALESCE(SUM(
        CASE WHEN lc.tipo IN ('ABERTURA','VENDA_DINHEIRO','SUPRIMENTO','SANGRIA','ESTORNO','CANCELAMENTO')
             THEN lc.valor_centavos - lc.troco_centavos
             ELSE 0
        END
    ), 0) AS saldo_esperado_centavos
FROM sessoes_caixa sc
LEFT JOIN livro_caixa lc ON lc.sessao_id = sc.id
GROUP BY sc.id;

-- Resumo financeiro do dia
CREATE VIEW vw_resumo_dia AS
SELECT
    DATE(v.created_at, 'localtime') AS dia,
    COUNT(*)            AS total_vendas,
    SUM(v.total_centavos)    AS faturamento_centavos,
    SUM(v.desconto_centavos) AS descontos_centavos
FROM vendas v
WHERE v.estado = 'FINALIZADA'
GROUP BY DATE(v.created_at, 'localtime');

-- Estoque por lote (FEFO) para o caixa escolher o lote que vence primeiro
CREATE VIEW vw_estoque_lote AS
SELECT
    l.id            AS lote_id,
    l.produto_id,
    l.validade,
    COALESCE(SUM(me.quantidade), 0) AS saldo_atual
FROM lotes l
LEFT JOIN movimentacoes_estoque me ON me.lote_id = l.id
WHERE l.ativo = 1
GROUP BY l.id;
