# Mikella ERP — Auditoria do fluxo de pagamento do PDV

*07/11/2026. Para o dono do produto e para o agente de implementação (Gemini/Antigravity). Complementa a auditoria de UX e cadastros.*

**Status:** auditoria baseada em 3 imagens (tela de pagamento, tela de venda e o esboço do dono) e nas explicações do dono. **Não tive acesso ao código.** O que está marcado como "provável" é hipótese. Itens **\[DECISÃO PENDENTE\]** pedem resposta do dono.

**Precedência:** a ideia do dono (esboço e texto) manda. Visual segue o `08`, desempenho o `07`, regras de caixa e livro imutável o `06`, aprovações o `05`.

> **Atualização de implementação (09/11/2026):**
> - **Resolvidos e verificados no código:** PAG-01 (pagamento parcial), PAG-02
>   (grade separada no F3), PAG-03 (parcelas com preview), PAG-04 (um campo de
>   valor), PAG-05 (`Quitado` em verde), PAG-06 (`Finalizar venda`), PAG-07
>   (sombra preta), PAG-08 (rótulos em caixa de frase), PAG-09 (botão
>   “Remover” com confirmação), PAG-10 (hover neutro; seleção amarela é estado
>   intencional), PAG-11 (toast à esquerda, sem cobrir a ação primária) e
>   VND-03 (cabeçalhos sem caixa alta). VND-01/VND-02 também foram corrigidos,
>   mas dependem de validação visual no hardware real.
> - **Resolvido:** parcelas de cartão e prazo de crediário configuráveis por
>   loja. A migração `0012_config_pagamento.sql` grava os limites em
>   `loja_config`, `GET /v1/config/pagamento` alimenta o PDV e
>   `PUT /v1/sistema/config-pagamento` (DONO, auditado) altera os valores. A
>   venda revalida no servidor; a UI não é fronteira confiável.
> - O PDV é **não fiscal**: a menção “CPF na nota” foi substituída por vínculo
>   do CPF à venda.

---

## 1. A ideia do dono, organizada

O pagamento tem **duas etapas separadas**, e o sistema atual as mistura:

1. **Escolher a forma de pagamento (tecla F3).** É uma tela só de seleção: Dinheiro, Débito, Crédito, Voucher, Pix, Crediário.
2. **Pagar (tecla F1).** É uma tela só de confirmação, que mostra **apenas**: valor total (ou restante), forma de pagamento escolhida e **quando vai ser debitado**. Nada mais.

**Regra do pagamento misto:** o valor digitado pode ser **menor** que o restante. Isso **não é erro**. O sistema registra aquele pagamento, calcula o que falta e **volta para a escolha da forma de pagamento**, mostrando o restante. Repete quantas vezes for preciso (segundo, terceiro, quarto meio, e assim por diante) até o restante chegar a zero.

**Exemplo 1 (do dono):** compra de R$ 15,00. O operador escolhe Crédito e digita R$ 10,00. O sistema aceita, mostra "Falta R$ 5,00" e pergunta a próxima forma. O operador escolhe Dinheiro e recebe R$ 10,00. O sistema mostra o troco de R$ 5,00 e finaliza.

**Exemplo 2 (esboço):** compra de R$ 55,90. A tela de pagamento abre em Dinheiro com R$ 55,90. O operador troca para R$ 45,90 e dá Enter. O sistema cai direto na escolha da forma (F3) mostrando o restante de R$ 10,00.

**Interpretação a confirmar:** entendi "quando vai ser debitado" como o prazo da cobrança: à vista no Débito, número de parcelas no Crédito (1x como padrão) e vencimento no Crediário. **\[DECISÃO PENDENTE D1\]** confirmar.

---

## 2. O que outros PDVs fazem (pesquisa)

Consultei páginas de ajuda de PDVs brasileiros e de lojas virtuais. Não vi as telas, só a descrição dos passos, então use como referência de **fluxo**, não de visual.

- **PDV da Alterdata:** o operador aciona a forma de pagamento por tecla de função, informa o **valor a ser pago naquela forma** e a tela de fechamento vai listando as formas já lançadas. Para pagar só parte no cartão, digita o valor e escolhe a forma. O parcelamento tem tecla própria e uma tela de prazo. Depois, uma tecla finaliza a venda. É o mesmo modelo do dono: **escolhe a forma, informa o valor, repete, finaliza.**
- **Omie PDV:** o cupom com mais de uma forma de pagamento segue a mesma lógica de lançar uma forma por vez, e Enter ou Esc retornam à tela inicial para a próxima venda.
- **Nuvemshop (PDV):** lista as combinações que os lojistas realmente usam: dinheiro e cartão, dois cartões de crédito (comum em ticket alto), e Pix mais cartão (quando o Pix tem desconto). No dinheiro, o operador escolhe se o cliente pagou o valor exato ou outro valor, o que gera o troco.
- **TOTVS PDV (registro de correção):** houve um defeito em que escolher **dinheiro duas vezes** na mesma venda somava o valor na primeira linha e gerava troco e valores errados. É um caso de teste obrigatório para o Mikella (ver seção 8).

**Padrão que se repete:** forma de pagamento escolhida antes, valor digitado depois, e **valor menor que o total é aceito como pagamento parcial**. Rejeitar valor menor, como o Mikella está fazendo, não existe em PDV de mercado.

---

## 3. Defeitos constatados

Severidade: **B** = bloqueia uso, **A** = alto (dinheiro ou dado errado), **M** = médio (usabilidade).

| ID | Defeito | Sev. | Observação |
| --- | --- | --- | --- |
| PAG-01 | Valor menor que o total no crédito gera erro | B | Deveria ser pagamento parcial |
| PAG-02 | Tela de pagamento mostra a grade de formas de pagamento (F1 a F6) junto com o pagamento | M | A escolha pertence ao F3, não à tela de pagar |
| PAG-03 | Não existe parcelamento no crédito | A | Faltam campo de parcelas e registro no pagamento |
| PAG-04 | Dinheiro mostra dois campos ("Valor" e "Recebido") | M | Confunde o operador; na prática é um só (ver seção 5) |
| PAG-05 | "Restante" em vermelho quando é R$ 0,00 | M | Vermelho indica erro. Zero restante é estado de sucesso, mostrar `Quitado` |
| PAG-06 | Botão "Concluir e Emitir Nota" | A | O produto não emite nota fiscal (D11). Usar `Finalizar venda` |
| PAG-07 | Sombra amarela deslocada nos botões | M | Viola VIS-072 (sombra só preta e só no primário do modo Rua). Amarelo é cor de aviso (`08`, 5.2) |
| PAG-08 | Rótulos em caixa alta e com barra ("FALTAM / RESTANTE") | M | Viola VIS-020. Usar `Falta pagar` |
| PAG-09 | Único meio de remover um pagamento lançado é um X vermelho sem rótulo | M | Precisa de ação com texto e confirmação |
| PAG-10 | Card de Débito destacado em amarelo claro sem motivo | M | Foco ambíguo: o operador não sabe o que está selecionado |
| PAG-11 | Aviso (toast) "O total da venda já foi..." sobreposto ao canto da tela | M | Confirmação temporária não pode cobrir ação |
| VND-01 | Campo de bipagem desalinhado: rótulo e input em duas caixas sobrepostas, com rótulo repetido e placeholder | M | Um rótulo só, uma caixa só |
| VND-02 | "1 itens na comanda" | M | Plural errado, e o texto "comanda" aparece em venda sem comanda |
| VND-03 | Rótulos de cabeçalho em caixa alta | M | Caixa de frase, exceto Display |

---

## 4. Fluxo correto, passo a passo

### 4.1 Tela de venda

- `F3` **Forma de pagamento:** abre a tela de seleção (seção 4.2). Pode ser acionada em qualquer momento da venda, inclusive antes de acabar de bipar.
- `F1` **Pagar:** abre a tela de pagamento (seção 4.3) com a forma já escolhida.
- **Se o operador apertar F1 sem ter escolhido a forma,** o sistema abre primeiro a seleção e, ao escolher, segue para o pagamento. Assim `F1`, `F1` (Dinheiro), `Enter` paga em dinheiro o valor exato. **\[DECISÃO PENDENTE D2\]** confirmar que não deve existir forma padrão.
- O painel direito mostra, quando houver, a lista **Pagamentos lançados** (forma, valor, parcelas) e o **Falta pagar**.
- Continuar bipando depois de lançar pagamento parcial é permitido, conforme o dono. O restante é recalculado a cada alteração (ver 6.3).

### 4.2 Tela de seleção de forma de pagamento (F3)

- Mostra o **total** e o **falta pagar** no topo.
- Seis opções: `F1` Dinheiro, `F2` Débito, `F3` Crédito, `F4` Voucher, `F5` Pix, `F6` Crediário.
- Escolher uma forma **fecha a seleção** e volta à tela de venda (ou segue para o pagamento, se veio do F1).
- `Esc` volta sem alterar nada.
- Formas desativadas pela loja não aparecem (configuração, não código).

### 4.3 Tela de pagamento (F1)

Mostra **somente**:

1. **Total da venda** (e, havendo pagamentos lançados, **Já pago** e **Falta pagar**).
2. **Forma de pagamento** escolhida, com a dica `F3 Mudar forma`.
3. **Valor** a pagar nesta forma, já preenchido com o falta pagar.
4. **Quando será debitado**, conforme a forma:

| Forma | Campo de prazo | Padrão |
| --- | --- | --- |
| Dinheiro | Nenhum. O campo de valor é **"Valor recebido"** | Falta pagar |
| Débito | Texto fixo `À vista` | Falta pagar |
| Crédito | **Parcelas** | `1x` (à vista). Ao digitar mais de 1, mostra `3x de R$ 33,33` |
| Voucher | Nenhum | Falta pagar |
| Pix | Nenhum, com aviso para conferir o recebimento no app do banco | Falta pagar |
| Crediário | **Vencimento** e cliente identificado | Prazo padrão da loja |

5. Rodapé: `Enter Confirmar`, `F3 Mudar forma`, `Esc Voltar`.

**Não aparece** nesta tela: a grade de formas de pagamento, lista de produtos, botões de outras funções.

### 4.4 O que o Enter faz

| Situação | Resultado |
| --- | --- |
| Valor **menor** que o falta pagar | Registra o pagamento parcial, calcula o restante e **abre a seleção F3** com a mensagem `Falta R$ 5,00. Escolha a próxima forma de pagamento.` |
| Valor **igual** ao falta pagar | Quita e finaliza a venda |
| Dinheiro com valor **maior** | Mostra o **troco exato, em destaque**, e finaliza |
| Débito, Crédito, Voucher, Pix ou Crediário com valor **maior** | Erro no campo: `O valor passa do que falta pagar. Máximo: R$ 10,00.` (não existe troco fora do dinheiro) |
| Valor zero, negativo ou vazio | Erro no campo, sem fechar |

---

## 5. Regras de dinheiro e troco

- PAG-R01: no dinheiro existe **um único campo**: `Valor recebido`. O padrão é o falta pagar.
- PAG-R02: se o recebido for menor que o falta pagar, ele é **aplicado integralmente** e o restante vai para a próxima forma.
- PAG-R03: se for maior, o troco é `recebido − falta pagar`, mostrado em destaque antes de concluir.
- PAG-R04: o **livro do caixa** registra o dinheiro líquido (recebido menos troco), conforme `06`, seção 4.
- PAG-R05: dinheiro lançado **duas vezes** na mesma venda não pode somar errado nem gerar troco duplicado (defeito conhecido em outro PDV, seção 2). Cada lançamento é uma linha; o troco é calculado **uma vez, no final**.
- PAG-R06: a gaveta de dinheiro abre **só** se houve dinheiro na venda.
- PAG-R07: valores digitados em formato brasileiro (`55,90`); aceitar também `.` como separador decimal e normalizar. Dinheiro sempre em **centavos inteiros** (`00`, regra inviolável 2).

---

## 6. Regras gerais do pagamento misto

### 6.1 Lançamentos

- PAG-M01: cada pagamento é uma **linha** (forma, valor, parcelas/vencimento, hora). Não há limite fixo de linhas; o limite técnico (por exemplo 10) vem de configuração.
- PAG-M02: a **mesma forma pode repetir**: dois cartões de crédito (comum em ticket alto), dois Pix, etc.
- PAG-M03: antes de finalizar, o operador pode **remover** uma linha lançada, com ação rotulada (`Remover pagamento`) e confirmação. O restante volta a subir.
- PAG-M04: a venda só finaliza com **falta pagar = 0**. Enquanto houver restante, `F1` continua levando ao próximo pagamento.

### 6.2 Parcelamento no crédito

- PAG-P01: **uma única opção Crédito** com campo Parcelas, padrão `1`. Não existe pergunta "deseja parcelar?".
- PAG-P02: limite de parcelas e **valor mínimo da parcela** vêm da configuração da loja.
- PAG-P03: a divisão usa centavos inteiros; a diferença de centavos vai na **última parcela**. O total nunca muda.
- PAG-P04: em pagamento misto, as parcelas valem **só para o valor lançado no crédito**.
- PAG-P05: o número de parcelas é **gravado** no pagamento e impresso (`Crédito 3x de R$ 33,33`). Serve ao financeiro e à conciliação com a operadora.
- PAG-P06: como não há integração com a maquininha (RF-011), mostrar aviso: `Confira se a maquininha está com o mesmo número de parcelas.`
- PAG-P07: parcelar no cartão **não é** crediário. A operadora financia no cartão; a loja financia no crediário.

### 6.3 Alterações depois de lançar pagamento

- PAG-A01: se itens forem adicionados, o falta pagar sobe. Se itens forem cancelados e o total ficar **menor que o já pago**, o sistema bloqueia a finalização e exige remover ou ajustar linhas: `Os pagamentos passam do total em R$ X. Remova ou ajuste um pagamento.`
- PAG-A02: **cartão aprovado na maquininha e venda interrompida:** os pagamentos lançados fazem parte do **rascunho persistente** (RES-020). Queda de energia no meio do pagamento misto retoma com as linhas já lançadas, sem duplicar ao reenviar (idempotência, RES-004).
- PAG-A03: cancelar a compra inteira (F5) com pagamento já lançado exige aprovação com PIN, porque houve valor recebido.

### 6.4 Crediário

- PAG-C01: exige cliente identificado e mostra limite e saldo devedor.
- PAG-C02: ultrapassar o limite exige PIN de aprovador (PROD-021).
- PAG-C03: o valor lançado vira conta a receber e **aparece na lista de devedores na hora** (F6 do menu).

### 6.5 Finalização

- PAG-F01: uma **única transação** grava venda, itens, pagamentos, baixa de estoque, movimentações de caixa, título do crediário e comissão, com chave de idempotência.
- PAG-F02: o botão se chama `Finalizar venda`. O comprovante é impresso como **comprovante não fiscal**.
- PAG-F03: depois de finalizar, volta ao menu do caixa.

---

## 7. Telas: ajustes de interface

**Tela de pagamento**

- Título uma vez. Rótulos em caixa de frase (`Total`, `Falta pagar`, `Valor`, `Parcelas`).
- Falta pagar: neutro enquanto houver valor; `Quitado` em verde quando for zero. Nunca vermelho para zero.
- Troco em destaque (número grande, Display), com a palavra `Troco`.
- Botão primário `Finalizar venda` (ou `Confirmar` quando ainda houver restante) em cobalto; secundário `Voltar`. Sombra apenas conforme o `08`.
- Campo de valor com foco automático e texto selecionado, para sobrescrever digitando.
- Pagamentos lançados em lista pequena (fita), com `Remover` por linha.

**Tela de venda**

- Um rótulo e uma caixa de bipagem, alinhados (VND-01).
- `1 item na venda` / `3 itens na venda`; `na comanda` só quando houver comanda (VND-02).
- Painel direito com **Pagamentos lançados** e **Falta pagar** quando houver.

---

## 8. Testes de aceite (evidência obrigatória)

Usar valores sintéticos. Cada item exige captura de tela ou saída de teste.

1. Compra de R$ 15,00. Crédito com R$ 10,00 (1x). O sistema aceita, mostra `Falta R$ 5,00` e abre a seleção.
2. Em seguida, Dinheiro com recebido R$ 10,00: troco de R$ 5,00 e finalização.
3. Compra de R$ 55,90. Em Dinheiro, trocar para R$ 45,90 e Enter: cai na seleção com `Falta R$ 10,00`. Escolher Pix, valor R$ 10,00: finaliza.
4. Compra de R$ 100,00 com dois cartões: Crédito A R$ 60,00 em 3x e Crédito B R$ 40,00 em 1x. Os dois ficam gravados, cada um com suas parcelas.
5. Compra de R$ 100,00 com cinco formas diferentes em sequência: o restante diminui a cada passo e finaliza em zero.
6. Crédito: o campo Parcelas nasce em `1`. Digitar 3 em R$ 100,00 mostra `3x de R$ 33,33` com a diferença de centavos na última parcela.
7. Pix com valor maior que o restante: erro, nada lançado.
8. Dinheiro de R$ 150,00 em compra de R$ 100,00: troco R$ 50,00 em destaque.
9. Dinheiro lançado duas vezes na mesma venda: total e troco corretos, sem somar errado.
10. Valor zero, negativo e vazio: erro no campo.
11. Remover um pagamento lançado: o falta pagar volta a subir.
12. Lançar R$ 50,00 em uma venda de R$ 100,00, depois cancelar itens até o total ficar R$ 30,00: a finalização é bloqueada com a mensagem de excesso.
13. Lançar pagamento parcial, matar o aplicativo e reabrir: a venda retoma com as linhas lançadas, sem duplicar ao finalizar.
14. `F1` sem forma escolhida abre a seleção; `F1`, `F1`, `Enter` paga em dinheiro o valor exato.
15. Falta pagar em zero aparece como `Quitado`, não em vermelho.
16. A tela de pagamento **não** contém a grade de formas de pagamento.
17. O botão final diz `Finalizar venda` e o comprovante sai como não fiscal.
18. `Esc` na tela de pagamento volta à venda sem apagar pagamentos já lançados.
19. Gaveta abre só quando houve dinheiro.
20. Cancelar a compra com pagamento lançado pede PIN de aprovador.

---

## 9. Regras de trabalho para o agente

1. **Corrigir a classe, não a instância:** se um valor menor foi rejeitado no crédito, verifique todas as formas de pagamento.
2. **Evidência antes de "concluído"**, e registre o que não foi testado.
3. **Ambiguidade vira pergunta curta;** padrão universal de UI não é ambiguidade.
4. **Nada de texto fiscal** ("nota", "cupom fiscal", "COO") enquanto o produto não emite documento fiscal.
5. **Nada de limite, prazo ou parcela máxima fixos no código.** Tudo vem de configuração da loja.
6. **Dado de teste sintético.**

---

## 10. Decisões pendentes para o dono

| # | Pergunta | Recomendação |
| --- | --- | --- |
| D1 | "Quando vai ser debitado" é: à vista, parcelas e vencimento? | Sim, conforme a tabela 4.3 |
| D2 | `F1` sem forma escolhida: abrir a seleção ou assumir Dinheiro? | Abrir a seleção (F1, F1, Enter resolve o caso comum) |
| D3 | Limite padrão de parcelas e valor mínimo da parcela | Configuráveis por loja; sugerir 12x e R$ 5,00 como ponto de partida |
| D4 | Quantas linhas de pagamento por venda no máximo | 10, configurável |
| D5 | Cancelar compra com pagamento já lançado | Exigir PIN de aprovador |

---

## 11. O que não foi comprovado

- Nenhuma causa de defeito foi confirmada no código.
- A pesquisa vem de páginas de ajuda de fornecedores; não vi as telas. O fluxo dos PDVs de mercado é descrito por texto e pode ter variações.
- Metas de tempo (100 ms, 300 ms) são hipóteses do `07`, ainda não medidas no hardware de referência.
- Nada aqui é parecer fiscal ou contábil. Taxas de maquininha e conciliação com operadoras ficam para fase posterior.

---

## Fontes consultadas

- Alterdata, ajuda do PDV (formas de pagamento): https://ajuda.alterdata.com.br/pam/primeiros-passos/lancar-vendas/formas-de-pagamento
- Alterdata, ajuda (pagamento parcial e parcelamento): https://ajuda.alterdata.com.br/x/wJ0fC
- Omie, PDV com mais de uma forma de pagamento: https://ajuda.omie.com.br/pt-BR/articles/8017599-omie-pdv-como-emitir-um-cupom-com-mais-de-uma-forma-de-pagamento
- Nuvemshop, registro de vendas no PDV: https://atendimento.nuvemshop.com.br/pt\_BR/ponto-de-venda/como-registrar-suas-vendas-no-ponto-de-venda-pdv-da-nuvemshop
- TOTVS, correção do PDV (dinheiro lançado duas vezes): https://tdn.totvs.com/x/uaBbDQ
