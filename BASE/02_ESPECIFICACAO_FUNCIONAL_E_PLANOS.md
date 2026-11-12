# Mikella ERP — especificação funcional e planos

> **Nota de atualização (09/11/2026):** especificação histórica. Vários itens
> já foram entregues (vendas, caixa, financeiro, clientes/crediário, CSV,
> backup e LGPD). O piso de hardware vigente é i3/i5 de 5ª geração, 6 GB e HDD
> 5400 rpm. Consulte `PLANO E FASES/00_PLANO_MESTRE.md` para o estado real.

**Status:** especificação inicial. Itens **[DECISÃO PENDENTE]** não podem ser implementados com valor fixo; devem ler de configuração (entitlements). Termos em `00_README_INDICE_E_CONTEXTO.md`.

## 1. Papéis e hierarquia

| Papel | Descrição |
|---|---|
| **DONO** (host) | Titular da conta. Controle total. Define permissões dos gerentes. Pode remover gerentes/admins. Não pode ser removido por ninguém abaixo dele |
| **GERENTE / ADMIN** | Administrador delegado. Permissões configuradas pelo dono. Gerencia apenas papéis **estritamente abaixo** do seu |
| **OPERADOR_CAIXA** | Opera a frente de caixa |
| **ESTOQUISTA** | Opera estoque, recebimento, validade |
| **VENDEDOR** | Atende, abre comandas, recebe comissão |
| Outros papéis | Cadastráveis por modelo de permissões |

**Regra de hierarquia (RF-001):** um usuário só cria, edita, desativa ou remove usuários de papel estritamente inferior. Gerente não remove gerente nem dono. Só o dono remove gerentes. O dono não pode ser removido nem rebaixado por outro usuário.

**Regra de permissões (RF-002):** autorização por **capacidades** (ex.: `sale.cancel`, `stock.adjust`, `finance.view`, `product.create`, `user.manage`, `cash.close`). Papéis são conjuntos de capacidades. O dono escolhe quais capacidades cada gerente recebe. Ações de risco exigem **PIN de aprovador** (ver `05`).

## 2. Módulos

### 2.1 Frente de caixa (PDV)

- RF-010: venda por código de barras ou busca; quantidade; desconto dentro do limite do papel; observação.
- RF-011: formas de pagamento: dinheiro (com troco), débito, crédito, Pix, outros configuráveis; pagamento misto. Integração com maquininha/TEF é **fora do MVP**; registro manual do meio de pagamento.
- RF-012: vários caixas simultâneos na mesma loja; cada sessão de caixa pertence a um operador e um dispositivo.
- RF-013: sessão de caixa: **abertura** (valor inicial), **vendas**, **sangria**, **suprimento**, **estorno/cancelamento** (com aprovação), **fechamento** (contagem e diferença).
- RF-014: **a sessão nunca é encerrada automaticamente** por queda de energia, rede ou travamento. Só operador ou gerente fecham, com conferência.
- RF-015: rascunho de venda em andamento persiste localmente no PC do caixa e sobrevive a reinício.
- RF-016: cada venda tem identificador único gerado antes da gravação (idempotência).
- RF-017: preço, desconto, total e estoque são **recalculados no servidor da loja**; a UI envia apenas identificadores e quantidades.
- RF-018: venda com estoque insuficiente: **[DECISÃO PENDENTE P1]**, configurável por loja (bloquear ou permitir com alerta).
- RF-019: impressão de comprovante não fiscal em impressora térmica (ESC/POS) e gaveta de dinheiro; leitor de código de barras como teclado. Hardware por Adapter.

### 2.2 Estoque

- RF-020: estoque calculado por **movimentações** (entrada, venda, ajuste, perda, devolução, inventário). Saldo é derivado.
- RF-021: produtos com código de barras, SKU, categoria, unidade, preço de venda, custo, estoque mínimo, flag de perecível.
- RF-022: **lotes e validade** para perecíveis; venda sugere o lote que vence primeiro (FEFO), configurável.
- RF-023: **alerta de vencimento**: prazo configurável por produto/categoria (padrão sugerido: 60 dias). O alerta é entregue a dono/gerentes/estoquistas e **persiste até que alguém com permissão de estoque abra o item e confirme a leitura**. A confirmação é registrada (quem, quando). Novos marcos (ex.: 30, 7 dias, vencido) reabrem o alerta. Sugestão de promoção exibida no alerta.
- RF-024: inventário com contagem e geração de ajustes auditáveis.
- RF-025: produto **nunca é excluído fisicamente**; "excluir" é arquivar. Produto com histórico permanece consultável.

### 2.3 Financeiro

- RF-030: **contas a pagar** e **contas a receber** com vencimento, parcelas, status e baixa.
- RF-031: **fluxo de caixa**, **faturamento**, **lucro e prejuízo** (resultado simplificado por período), derivados de movimentações e baixas.
- RF-032: relatórios por período, forma de pagamento, categoria, operador.
- RF-033: janela de visualização por plano (seção 4). Dados fora da janela **não são apagados**; ficam selados (ver `05`).
- RF-034: valores sempre em centavos inteiros; arredondamento definido e testado.

### 2.4 Pessoas e permissões

- RF-040: cadastro de usuários, papéis e capacidades (seção 1).
- RF-041: login local: usuário + senha (ou PIN para operações de balcão). 2FA conforme `04`.
- RF-042: **sessão única por administrador**: se o mesmo administrador entrar em outro PC, o sistema pergunta: *"O computador (nome do usuário do sistema) está conectado nesta conta. Deseja encerrar a sessão lá e iniciar aqui?"*. Se confirmado, a sessão anterior é revogada.
- RF-043: limite de **8 computadores pareados por loja** (registro de dispositivos no servidor da loja e na nuvem).

### 2.4.1 Comissões e comandas (Plano Pago)

- RF-050: regras de comissão por **produto** ou por **categoria inteira**, por vendedor ou para todos; percentual ou valor fixo; vigência de datas; versionadas (alteração não muda vendas passadas). Exemplo: todos os produtos da marca/categoria "X" vendidos pelo vendedor 23 geram comissão.
- RF-051: **comanda**: o vendedor abre uma comanda, registra os itens que vendeu e ela chega ao caixa. O caixa carrega a comanda, e a atribuição ao vendedor é automática.
- RF-052: item comissionável bipado **sem comanda**: o caixa precisa informar o **código do vendedor** (digitado ou bipado de crachá). Trocar o vendedor depois exige aprovação de gerente e fica registrado.
- RF-053: a comissão é gravada como movimentação no momento da venda, com a regra aplicada (snapshot). Devolução ou cancelamento gera estorno de comissão.
- RF-054: relatório de comissão por vendedor, produto, categoria, período.
- RF-055: indicadores de anomalia de atribuição (ver `05`, seção de fraude de comissão).

### 2.5 Recebimento por NF-e (importação de XML)

- RF-060: importar XML de NF-e (modelo 55) de **compra**: lê fornecedor, itens, quantidades, valores e parcelas.
- RF-061: criar **recebimento** com itens esperados. O funcionário conta cada item e marca **conferido** ou **divergente** (falta, sobra, avaria). Resolução de divergência registrada.
- RF-062: ao concluir, gera entrada de estoque do que foi conferido e **contas a pagar** a partir das parcelas do XML.
- RF-063: **fora do MVP:** baixar notas automaticamente da SEFAZ (exige certificado digital A1 e integração). Emissão fiscal de saída (NFC-e/SAT) também fora. **Risco de escopo:** a obrigatoriedade de emissão fiscal varia por estado e regime tributário; validar com contador antes de definir público.

### 2.6 Contas, licença e sincronização

- RF-070: **cadastro na landing:** plano, nome, CPF/CNPJ, telefone, nome da loja, login, senha. Login e senha representam a conta/loja do dono.
- RF-071: **ativação do servidor:** o instalador em modo servidor autentica o dono (com 2FA), recebe licença assinada e cria o banco da loja vinculado à conta.
- RF-072: **backup e sincronização:** backup local automático e backup cifrado na nuvem; restauração em novo computador (ver `06`).
- RF-073: **cotas de produto** (seção 4) com contador **visível ao usuário** ("você criou 61 de 85 produtos neste mês").
- RF-074: ao reativar após formatação, a restauração entrega **somente o que o plano permite** (seção 4).

### 2.7 Portal web da conta

- RF-080: login no navegador com usuário/e-mail + senha + 2FA por e-mail.
- RF-081: gerenciar plano e cobrança, ver dispositivos pareados, baixar o instalador, iniciar restauração, revogar dispositivo.
- RF-082: **a loja não é operada pelo navegador.** O portal não acessa dados de venda.

## 3. Requisitos não funcionais (resumo; detalhes nos documentos próprios)

- RNF-001: rodar em i3/i5 de 5ª geração, 6 GB de RAM, HDD 5400 rpm, Lubuntu/Linux Mint/Linux Lite (WebKitGTK 4.1) e Windows 10+ (`07` e `09`).
- RNF-002: sem perda de movimentação em queda de energia (`06`).
- RNF-003: segurança e LGPD por desenho (`04`).
- RNF-004: modularidade, testabilidade e ausência de acoplamento rígido (`03`).
- RNF-005: interface com motion discreto e fluido, sem efeitos caros (`07`).

## 4. Planos e entitlements

### 4.1 Gratuito

| Item | Regra |
|---|---|
| Lojas | 1 |
| Administradores | Dono + **1** gerente/admin com permissões configuráveis |
| Computadores por loja | até 8 (1 servidor + clientes) |
| Estoque | Incluído |
| Contas a pagar e a receber | Incluídas |
| Relatórios e financeiro | **Últimos 6 meses**; histórico anterior fica selado e oculto |
| Produtos **criados** por mês | **85** (cota mensal; contador visível) |
| Edições e exclusões (arquivamentos) de produto | Limitadas **[DECISÃO PENDENTE P6]**; contador visível |
| Comissões e comandas | Não |
| Backup e restauração | Sim, restaurando só a janela de 6 meses |

### 4.2 Pago

| Item | Regra |
|---|---|
| Lojas | Até **8** por conta |
| Administradores por loja | Dono + até **4** admins/gerentes por loja |
| Computadores por loja | até 8 |
| Produtos **criados** por mês | **200 por loja** |
| Relatórios e financeiro | **Histórico completo** desde a abertura da loja |
| Comissões e comandas | Sim |
| Backup e restauração | Histórico completo |

**Definição de "produto novo":** nova entidade de produto, **não** unidades em estoque (200 produtos novos ≠ 200 unidades de um produto).

### 4.3 Regras transversais de plano

- RF-090: cada conta tem **limite próprio e definitivo**; não existe transferência de capacidade entre contas.
- RF-091: excluir (arquivar) produto **não devolve** cota; editar e excluir consomem suas próprias cotas.
- RF-092: ao assinar o Pago, dados selados são liberados; ao voltar ao Gratuito, voltam a ser selados (nunca apagados por isso).
- RF-093: estados de assinatura (derivados no servidor): `TRIAL` (se houver), `ATIVO`, `EM_TOLERANCIA`, `SUSPENSO`, `CANCELADO`, `RETENCAO`. Duração de trial/tolerância são parâmetros. **O comportamento ao suspender segue P2** (não apagar banco local sem decisão jurídica explícita).
- RF-094: ao detectar plano insuficiente para uma ação, a interface explica o limite e o contador, sem bloquear o que já existe.

## 5. Fluxos principais

### 5.1 Cadastro e ativação
1. Landing: usuário escolhe plano e cadastra dados. Servidor valida CPF/CNPJ (dígitos), impede segundo cadastro gratuito do mesmo documento, envia código de e-mail.
2. Portal: login com 2FA, download do instalador.
3. Instalador (modo servidor): autentica o dono, recebe licença e manifesto assinado, cria banco cifrado, gera **frase de recuperação**, exibe e exige confirmação de que foi guardada.
4. Instalador (modo cliente): pareamento por código exibido no servidor.

### 5.2 Venda
Abrir sessão de caixa → bipar itens → (comanda/vendedor se comissionável) → pagamento → servidor valida e grava venda, baixa de estoque, movimentação de caixa e comissão numa **única transação** → comprovante.

### 5.3 Troca de computador do administrador
Login em novo PC → servidor da loja detecta sessão ativa em outro → pergunta → revoga a antiga → inicia a nova.

### 5.4 Formatação do servidor / desastre
Instalar → modo servidor → login com 2FA → informar frase de recuperação → nuvem entrega **apenas os pedaços permitidos pelo plano** → servidor valida manifesto, reconstrói banco → reparear clientes.

## 6. Critérios de aceite de alto nível

- Cortar a energia durante uma venda e durante uma sangria não perde nem duplica movimentação, e o saldo do caixa confere.
- Três a oito caixas vendendo simultaneamente não geram erro de banco nem estoque incorreto.
- Gerente não consegue remover dono nem outro gerente.
- Criar, arquivar e recriar produtos em laço não permite exceder a cota mensal.
- Banco de uma conta não abre em outra conta.
- Plano Gratuito restaurado não entrega dados fora da janela de 6 meses.
