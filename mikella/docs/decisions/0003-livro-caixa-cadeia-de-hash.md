# ADR-0003 — Livro do caixa com cadeia de hashes

## Contexto

O caixa não pode fechar sozinho e o histórico precisa ser auditável. Se alguém
alterar o banco diretamente para esconder uma sangria, o fechamento deve
detectar. SQLite sozinho não impede `UPDATE` de quem tem o arquivo.

## Decisão

- Cada sessão tem uma cadeia própria: `sequencia` começa em 1, `prev_hash` da
  primeira entrada é `GENESIS` e cada entrada guarda o blake3 do próprio
  conteúdo (incluindo `prev_hash`).
- O hash usa prefixo de tamanho por campo, para evitar colisão de fronteira
  entre campos concatenados.
- `LivroCaixa::adicionar` recusa sequência fora de ordem e `prev_hash` que não
  corresponde ao último hash. A verificação completa é feita no fechamento
  (`verificar_integridade`), não a cada insert.
- Triggers no SQLite bloqueiam `UPDATE` e `DELETE` em `livro_caixa` e em
  `movimentacoes_estoque`.
- `FECHAMENTO` é informativo: registra contagem, esperado e diferença, mas não
  entra no saldo físico. O mesmo vale para Pix, débito, crédito e outros
  eletrônicos.
- A projeção `sessoes_caixa.saldo_esperado_centavos` é atualizada por trigger
  apenas para movimentos de dinheiro físico, com `ABERTURA` fora do trigger
  (ela já está em `saldo_inicial`).

## Consequências

- Fechar o caixa recalcula a cadeia inteira; adulteração direta no banco é
  recusada.
- A cadeia é local. Um atacante com a chave do banco pode reescrever a cadeia
  inteira; a proteção definitiva é o backup cifrado + âncora na nuvem (fase
  seguinte), não prometida para o MVP local.
- `livro_caixa` cresce em `INSERT` only. Índice por `(sessao_id, sequencia)`
  mantém o fechamento barato mesmo em sessões longas.
