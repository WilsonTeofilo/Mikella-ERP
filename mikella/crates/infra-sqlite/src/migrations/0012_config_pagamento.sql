-- ============================================================
-- Mikella ERP — Migração 0012: limites de pagamento por loja
-- ============================================================
-- Até aqui o máximo de parcelas no cartão (12) e o prazo do crediário
-- (30 dias) estavam fixos no código. Os dois passam a viver em
-- `loja_config`, com os mesmos defaults para não alterar vendas existentes.
--
-- A leitura/validação é feita em Rust (`ConfigLojaRepo`); o frontend só
-- renderiza o que a API devolve. Valores fora da faixa são ignorados na
-- leitura e rejeitados na escrita (SEC-017: erro sem SQL/caminho).

INSERT OR IGNORE INTO loja_config (chave, valor) VALUES ('max_parcelas_cartao', '12');
INSERT OR IGNORE INTO loja_config (chave, valor) VALUES ('prazo_crediario_dias', '30');
