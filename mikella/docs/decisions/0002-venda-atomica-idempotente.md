# ADR-0002 — Venda atômica e idempotente

## Contexto

A venda é o caminho crítico: envolve cabeçalho, itens, pagamentos, baixa de
estoque e lançamentos no livro do caixa. Uma falha no meio não pode deixar
estoque baixado sem venda, nem venda sem lançamento no caixa. Além disso, a
rede local pode cair entre o envio e a resposta; o operador pode reenviar.

## Decisão

- O cliente gera o `venda_id` (UUID v4) **antes** de enviar. Esse ID é a chave
  de idempotência: se a venda já existe, o caso de uso devolve a venda gravada
  sem repetir efeitos.
- O caso de uso `finalizar_venda` não abre transação. Quem chama (servidor ou
  shell) abre, passa os repositórios e dá commit/rollback. Todos os
  repositórios emprestam a mesma `Transaction`.
- Preço unitário e custo vêm do banco, nunca da UI. A UI envia apenas
  `produto_id`, `lote_id` e `quantidade`.
- A soma dos pagamentos tem de fechar exatamente com o total calculado pelo
  servidor; `VendaBuilder` recusa diferença.
- Estoque é registrado como movimentação imutável (`INSERT` only). As projeções
  `lotes.quantidade` e `sessoes_caixa.saldo_esperado_centavos` são atualizadas
  por triggers na mesma transação.
- A política padrão de estoque é bloquear venda acima do saldo. A loja pode
  optar por permitir saldo negativo global, mas lote nunca fica negativo
  (CHECK no banco).

## Consequências

- Teste de integração cobre falha simulada no meio da venda: nada fica gravado.
- Reenvio do mesmo `venda_id` não duplica itens, movimentações nem livro.
- O servidor precisa serializar escritas (ver ADR-0001); concorrência real
  entre dois caixas no mesmo banco não faz parte deste desenho.
- Uma venda cancelada/estornada exige caso de uso próprio; não se faz `UPDATE`
  em venda finalizada sem registrar o estorno.
