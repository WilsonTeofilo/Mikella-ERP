# ADR-0004 — Pagamento misto como lista de pagamentos

## Contexto

Uma venda pode ser paga com dinheiro + Pix, ou dois cartões. Guardar apenas
uma `forma_pagamento` no cabeçalho (como no modelo antigo) impedia saber quanto
entrou em cada forma e distorcia o fechamento do caixa.

## Decisão

- `Venda` carrega `pagamentos: Vec<Pagamento>`, com `forma`, `valor` e
  `recebido` (apenas para dinheiro).
- A soma dos `valor` tem de ser exatamente o total da venda.
- `Misto` existe apenas como classificação derivada: não é um pagamento
  concreto e é recusado se aparecer em `pagamentos`.
- Troco é calculado somente sobre pagamentos em dinheiro; eletrônicos não
  geram troco.
- O banco grava um registro por pagamento em `venda_pagamentos`, com
  `recebido_centavos` e `troco_centavos` quando a forma é dinheiro.
- O livro do caixa recebe um lançamento `VENDA_DINHEIRO` agregado (soma dos
  recebidos, soma dos trocos) e um lançamento por pagamento eletrônico.

## Consequências

- Fechamento de caixa e conciliação por forma ficam corretos.
- A UI precisa calcular o total com as mesmas regras do servidor para montar os
  pagamentos; o servidor revalida de qualquer forma.
- Relatórios por forma de pagamento passam a somar `venda_pagamentos`, não o
  cabeçalho.
