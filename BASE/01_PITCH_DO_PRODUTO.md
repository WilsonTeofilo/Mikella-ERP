# Mikella ERP — pitch do produto

**Status:** texto de posicionamento e estrutura de conteúdo. Não define identidade visual. Toda promessa abaixo só pode ir ao ar quando o recurso existir e tiver sido testado.

## Em uma frase

Mikella ERP é o sistema de gestão para pequenas lojas que **vende, controla estoque e dinheiro mesmo sem internet, roda em computador velho e não te cobra por funcionalidade que você nunca vai usar**.

## O problema

O pequeno lojista hoje costuma escolher entre:

1. **Sistemas web de gestão** que dependem de internet o tempo todo. Caiu a conexão, o caixa para.
2. **ERPs generalistas** com dezenas de módulos que ele nunca abre, interface pesada e preço que cresce a cada recurso.
3. **PDVs simples** que vendem, mas não controlam estoque, validade, contas a pagar nem comissão.
4. **Planilha e caderno**, que funcionam até o dia em que alguém erra uma conta.

A dor mais grave é a do **operador de caixa**: se a luz cai ou o sistema trava no meio do dia e o caixa "volta ao valor inicial", o operador responde pela diferença. Isso gera conflito, desconfiança e perda de dinheiro.

## Para quem é

Comércios de pequeno porte: lojas de suplementos, mercadinhos, farmácias de manipulação sem escopo regulatório complexo, papelarias, lojas de roupa, distribuidoras pequenas. Perfil típico: 1 a 8 caixas, poucos funcionários, computador antigo, internet instável, dono que também atende.

## A proposta

1. **Funciona sem internet.** O banco da loja fica no computador da loja. A nuvem existe para backup e licença, não para vender.
2. **Não perde o caixa.** Cada movimentação é gravada de forma permanente no instante em que acontece. Faltou luz? Ao religar, o caixa volta exatamente de onde parou, com o mesmo valor.
3. **Roda em computador fraco.** Piso de hardware: Intel Core i3 ou i5 de 5ª geração, 6 GB de RAM (DDR3-1333), HDD 5400 rpm no pior caso. Linux leve ou Windows 10/11. Windows 7 proibido.
4. **Só o que a loja usa.** Caixa, estoque, financeiro, equipe, comissão e validade. Sem módulo inútil.
5. **Seus dados são seus.** Backup cifrado antes de sair do computador; nem o Mikella consegue ler seu conteúdo.
6. **Gratuito de verdade.** Plano gratuito com estoque, financeiro dos últimos 6 meses, contas a pagar e a receber, caixa e um gerente.

## Principais recursos

| Área | O que entrega |
|---|---|
| **Frente de caixa** | Venda rápida com leitor de código de barras, várias formas de pagamento, múltiplos caixas ao mesmo tempo, abertura, sangria, suprimento e fechamento com conferência |
| **Caixa à prova de queda** | Livro de movimentações imutável; sessão retoma do ponto exato após queda de luz |
| **Estoque** | Entradas, saídas, ajustes, inventário, histórico por movimentação |
| **Validade** | Cadastro de lote e vencimento; aviso configurável (ex.: 2 meses antes) que persiste até alguém confirmar a leitura no estoque; sugestão de promoção |
| **Financeiro** | Contas a pagar e a receber, fluxo de caixa, faturamento, lucro e prejuízo |
| **Equipe** | Dono, gerentes e funcionários com permissões configuráveis; hierarquia que impede gerente de remover o dono |
| **Comissão (Pago)** | Comissão por produto ou categoria, por vendedor; comandas para atribuir a venda ao vendedor certo, com proteção contra atribuição indevida |
| **Recebimento por NF-e** | Importa o XML da nota de compra, lista os itens esperados, o funcionário confere e marca divergências; gera entrada de estoque e contas a pagar |
| **Backup e restauração** | Cópia local automática e cópia cifrada na nuvem; restauração em computador novo |
| **Várias lojas (Pago)** | Até 8 lojas por conta |

## Diferenciais, em termos verificáveis

- **Local-first:** a venda não depende de internet. (Verificável: desconectar a rede e vender.)
- **Retomada de caixa:** teste de corte de energia durante venda sem perda de movimentação. (Será publicado quando o teste existir.)
- **Leve:** consumo de memória e tempo de abertura medidos em hardware fraco, publicados com a metodologia.
- **Backup zero-knowledge:** o conteúdo do backup é cifrado no computador do lojista com chave que o Mikella não possui.
- **Honestidade de limites:** o plano gratuito declara claramente cotas e janela de histórico.

## Planos (resumo para o público)

| | Gratuito | Pago |
|---|---|---|
| Lojas | 1 | Até 8 |
| Administradores por loja | Dono + 1 gerente | Dono + até 4 admins/gerentes por loja |
| Produtos novos por mês | 85 | 200 por loja |
| Histórico financeiro visível | Últimos 6 meses | Desde a abertura da loja |
| Estoque, contas a pagar/receber, validade | Sim | Sim |
| Comissões e comandas | Não | Sim |
| Backup cifrado e restauração | Sim (restaura os últimos 6 meses) | Sim (histórico completo) |
| Computadores por loja | Até 8 | Até 8 |

Ao assinar o Pago, o histórico antigo que já estava guardado é liberado. **Nenhum dado é apagado por estar no plano gratuito.**

*Valores de preço, cotas de edição/exclusão e prazos de tolerância são parâmetros e ainda não foram definidos.*

## O que prometemos e o que não prometemos

| Podemos prometer (após teste) | Não prometemos |
|---|---|
| Vender sem internet | "Sistema inquebrável" |
| Retomar o caixa após queda de energia | Proteção contra disco com defeito físico **sem** nobreak e backup |
| Backup cifrado e restauração | Recuperação de backup se a frase de recuperação for perdida |
| Rodar em PC antigo dentro das metas medidas | Desempenho idêntico em qualquer máquina |
| Importar XML de NF-e de compra | Emissão de nota fiscal de venda (fora do escopo inicial) |

**Recomendação ao lojista, que entra na instalação:** nobreak no computador servidor e segundo disco ou pendrive para cópia automática.

## Objeções e respostas

- **"E se meu computador quebrar?"** O backup cifrado na nuvem restaura a loja em outro computador, com a sua frase de recuperação.
- **"E se eu perder a internet?"** Você continua vendendo. A licença tem uma janela offline de alguns dias.
- **"Vocês veem meus dados?"** Não. O backup é cifrado no seu computador. O Mikella guarda dados de conta e cobrança, não o conteúdo da sua loja.
- **"Por que gratuito?"** O plano gratuito atende quem é pequeno. Quem cresce, tem muito produto e precisa de histórico completo e comissão, paga.
- **"E se eu parar de pagar?"** *[Regra pendente: ver P2 no índice.]* Intenção: suas vendas são bloqueadas após o prazo de tolerância, mas seus dados continuam seus e exportáveis.
- **"Dá para usar em mais de um computador?"** Sim, até 8 por loja, um deles é o servidor.

## Jornada do cliente

1. Entra na landing, entende o produto em menos de um minuto.
2. Escolhe o plano, cadastra nome, CPF/CNPJ, telefone, nome da loja, login e senha (a conta representa a loja/marca).
3. Confirma e-mail (2FA), baixa o instalador.
4. Instala no computador principal, ativa com a conta, recebe e guarda a **frase de recuperação**.
5. Cadastra a loja, produtos e equipe; abre o caixa.
6. Instala nos demais caixas e pareia com o computador principal.

## Estrutura de conteúdo da landing page (sem decisão visual)

1. Promessa principal + CTA "Criar conta grátis".
2. O problema do caixa que "volta ao início do dia".
3. Como funciona offline e como o backup protege.
4. Recursos essenciais (lista curta, sem rolagem infinita).
5. Planos lado a lado, com limites claros.
6. Segurança e privacidade (LGPD) em linguagem simples.
7. Requisitos mínimos e compatibilidade (Windows 10+, Linux leve).
8. Perguntas frequentes.
9. CTA final + download.

**Regras de copy:** português do Brasil, direto e humano; explicar valor antes de pedir cadastro; nenhuma alegação que o produto não cumpra; nenhuma comparação que cite concorrente com afirmação não verificada; respeitar `prefers-reduced-motion`; página leve (a landing também é vitrine de desempenho).

## SEO e medição (para a MVP-3)

Páginas públicas pré-renderizadas; título e descrição únicos; `robots.txt` e sitemap; páginas de login/portal com `noindex`; metas de Core Web Vitals; medição com consentimento. Conteúdo útil sobre rotina de loja (controle de caixa, validade, comissão) em vez de preenchimento de palavra-chave.
