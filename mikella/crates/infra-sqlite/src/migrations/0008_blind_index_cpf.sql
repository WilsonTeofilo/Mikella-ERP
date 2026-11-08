-- Migration 0008: popula documento_blind com espaço reservador e cria índice auxiliar.
--
-- `documento_blind` já existe na tabela inicial (0001) com restrição UNIQUE.
-- SQLite não permite adicionar restrição UNIQUE depois do fato em ALTER TABLE,
-- mas a coluna foi criada com UNIQUE na DDL original — a restrição já está ativa.
--
-- Esta migração apenas:
--   1. Garante que linhas antigas com `documento` preenchido recebam um valor
--      sentinela distinto para evitar colisão de UNIQUE no primeiro boot pós-deploy.
--      O valor real é preenchido por `backfill_blind_index` no boot do servidor,
--      assim que a chave da loja estiver disponível. Registros sem documento
--      permanecem NULL.
--   2. Não cria índice adicional: a restrição UNIQUE da coluna criada em 0001
--      já mantém o índice usado nas buscas exatas por blind index.
--
-- NOTA: o sentinela é 'PENDENTE_<id>' — o backend substitui por BLAKE3 keyed
-- (HMAC da chave da loja) no boot.
UPDATE pessoas
   SET documento_blind = 'PENDENTE_' || id
 WHERE documento IS NOT NULL
   AND documento_blind IS NULL;
