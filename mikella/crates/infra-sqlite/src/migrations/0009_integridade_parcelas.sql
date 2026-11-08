-- ============================================================
-- Mikella ERP — Migração 0009: Integridade Estrutural de Parcelas e Contas (DAT-004)
-- ============================================================
-- Triggers de integridade referencial e validação de valores para o módulo financeiro.
-- Garante:
--   1. Parcelas possuem valor estritamente positivo (valor_centavos > 0).
--   2. conta_id em parcelas existe na tabela correta (contas_pagar ou contas_receber).
--   3. Deleção de contas com parcelas é bloqueada.
--   4. pessoa_id em contas_receber referencia uma pessoa existente, impedindo órfãos.

-- 1. Checks de valor estritamente positivo em parcelas
CREATE TRIGGER IF NOT EXISTS trg_parcelas_valida_valor_insert
BEFORE INSERT ON parcelas
FOR EACH ROW
WHEN NEW.valor_centavos <= 0
BEGIN
    SELECT RAISE(ABORT, 'DAT-004: valor_centavos da parcela deve ser estritamente positivo (> 0)');
END;

CREATE TRIGGER IF NOT EXISTS trg_parcelas_valida_valor_update
BEFORE UPDATE OF valor_centavos ON parcelas
FOR EACH ROW
WHEN NEW.valor_centavos <= 0
BEGIN
    SELECT RAISE(ABORT, 'DAT-004: valor_centavos da parcela deve ser estritamente positivo (> 0)');
END;

-- 2. Integridade referencial polimórfica de parcelas -> contas
CREATE TRIGGER IF NOT EXISTS trg_parcelas_valida_fk_pagar_insert
BEFORE INSERT ON parcelas
FOR EACH ROW
WHEN NEW.conta_tipo = 'PAGAMENTO' AND NOT EXISTS (SELECT 1 FROM contas_pagar WHERE id = NEW.conta_id)
BEGIN
    SELECT RAISE(ABORT, 'DAT-004: conta_id inexistente em contas_pagar');
END;

CREATE TRIGGER IF NOT EXISTS trg_parcelas_valida_fk_receber_insert
BEFORE INSERT ON parcelas
FOR EACH ROW
WHEN NEW.conta_tipo = 'RECEBIMENTO' AND NOT EXISTS (SELECT 1 FROM contas_receber WHERE id = NEW.conta_id)
BEGIN
    SELECT RAISE(ABORT, 'DAT-004: conta_id inexistente em contas_receber');
END;

-- 3. Bloqueio de exclusão órfã de contas com parcelas
CREATE TRIGGER IF NOT EXISTS trg_contas_pagar_impede_delete_com_parcelas
BEFORE DELETE ON contas_pagar
FOR EACH ROW
WHEN EXISTS (SELECT 1 FROM parcelas WHERE conta_id = OLD.id AND conta_tipo = 'PAGAMENTO')
BEGIN
    SELECT RAISE(ABORT, 'DAT-004: não é permitido excluir conta_pagar com parcelas associadas');
END;

CREATE TRIGGER IF NOT EXISTS trg_contas_receber_impede_delete_com_parcelas
BEFORE DELETE ON contas_receber
FOR EACH ROW
WHEN EXISTS (SELECT 1 FROM parcelas WHERE conta_id = OLD.id AND conta_tipo = 'RECEBIMENTO')
BEGIN
    SELECT RAISE(ABORT, 'DAT-004: não é permitido excluir conta_receber com parcelas associadas');
END;

-- 4. Vínculo de cliente em contas a receber
CREATE TRIGGER IF NOT EXISTS trg_contas_receber_valida_pessoa_insert
BEFORE INSERT ON contas_receber
FOR EACH ROW
WHEN NEW.pessoa_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM pessoas WHERE id = NEW.pessoa_id)
BEGIN
    SELECT RAISE(ABORT, 'DAT-004: pessoa_id inexistente em pessoas');
END;

CREATE TRIGGER IF NOT EXISTS trg_contas_receber_valida_pessoa_update
BEFORE UPDATE OF pessoa_id ON contas_receber
FOR EACH ROW
WHEN NEW.pessoa_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM pessoas WHERE id = NEW.pessoa_id)
BEGIN
    SELECT RAISE(ABORT, 'DAT-004: pessoa_id inexistente em pessoas');
END;

CREATE TRIGGER IF NOT EXISTS trg_pessoas_impede_delete_com_contas_receber
BEFORE DELETE ON pessoas
FOR EACH ROW
WHEN EXISTS (SELECT 1 FROM contas_receber WHERE pessoa_id = OLD.id)
BEGIN
    SELECT RAISE(ABORT, 'DAT-004: não é permitido excluir cliente com contas a receber vinculadas');
END;
