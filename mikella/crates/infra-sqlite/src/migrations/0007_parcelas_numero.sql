-- ============================================================
-- Mikella ERP — Migração 0007: numeração das parcelas migradas
-- ============================================================
-- A 0004 copia as parcelas antigas para a tabela unificada. A 0006 adiciona
-- `numero` com DEFAULT 1, então tudo que veio da migração ficaria "Parcela 1".
-- Aqui renumeramos pelo vencimento (ordem estável usada pela UI e pela
-- liquidação), sem tocar em parcelas já numeradas acima de 1.
--
-- DAT-001: preserva a leitura do crediário de bancos que migraram dados.

UPDATE parcelas
SET numero = (
    SELECT COUNT(*)
    FROM parcelas p2
    WHERE p2.conta_id = parcelas.conta_id
      AND p2.conta_tipo = parcelas.conta_tipo
      AND (
          p2.vencimento < parcelas.vencimento
          OR (p2.vencimento = parcelas.vencimento AND p2.rowid <= parcelas.rowid)
      )
)
WHERE numero = 1
  AND (
      SELECT COUNT(*)
      FROM parcelas p3
      WHERE p3.conta_id = parcelas.conta_id
        AND p3.conta_tipo = parcelas.conta_tipo
  ) > 1;
