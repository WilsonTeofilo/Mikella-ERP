# Mikella ERP — índice e contexto para continuidade

**Criado em:** 28/10/2026
**Status:** plano inicial. **Nada está implementado.** Todo requisito, número e meta aqui é decisão de projeto ou hipótese a validar, não fato comprovado.
**Escopo desta pasta:** produto, requisitos, arquitetura, segurança, anti-fraude, resiliência e desempenho. A **identidade visual ainda não foi decidida** e não faz parte deste pacote; restrições de desempenho que afetam design (motion, efeitos, fontes) estão em `07_OTIMIZACAO_PC_FRACO.md`.

## Como usar estes documentos (humanos e agentes de IA)

1. Leia este arquivo inteiro antes de qualquer tarefa.
2. Consulte o documento específico da área tocada pela tarefa.
3. Em conflito entre documentos, vale a ordem: `04_SEGURANCA` e `05_ANTI_FRAUDE` > `06_RESILIENCIA` > `03_ARQUITETURA` > `02_ESPECIFICACAO` > `07_OTIMIZACAO` > `01_PITCH`.
4. Se um requisito estiver marcado **[DECISÃO PENDENTE]**, não invente a resposta: implemente atrás de uma configuração/porta e pergunte.
5. Nunca marque algo como concluído sem evidência (teste executado, medição). Registre o que **não** foi provado.

## Índice

| Arquivo | Conteúdo |
|---|---|
| `00_README_INDICE_E_CONTEXTO.md` | Este arquivo: visão, componentes, decisões, roadmap, regras invioláveis, glossário |
| `01_PITCH_DO_PRODUTO.md` | Pitch detalhado: problema, público, proposta, diferenciais, planos, objeções, estrutura da landing |
| `02_ESPECIFICACAO_FUNCIONAL_E_PLANOS.md` | Módulos, papéis, requisitos numerados, planos Gratuito/Pago, fluxos |
| `03_ARQUITETURA_CLEAN_E_PADROES.md` | Clean Architecture, anti-monolito, design patterns, estrutura de pastas, clean code |
| `04_SEGURANCA.md` | Ameaças, autenticação, rede local, criptografia, Tauri, cadeia de suprimentos, LGPD |
| `05_ANTI_FRAUDE_E_LICENCIAMENTO.md` | Licença, cotas, vínculo banco↔conta, sessão única, limite de dispositivos, fraude de caixa/comissão |
| `06_RESILIENCIA_E_BACKUP.md` | Queda de luz/internet, livro do caixa, backup local e na nuvem, restauração, desastre |
| `07_OTIMIZACAO_PC_FRACO.md` | Orçamentos de RAM/CPU/tempo, SQLite, React, Tauri, motion, plano de medição |

## Resumo do produto

**Mikella ERP** é um sistema de gestão comercial **desktop e local-first** para pequenos comércios: frente de caixa (PDV), estoque, contas a pagar e a receber, financeiro, usuários com permissões, comissões e backup cifrado na nuvem. Roda em computadores fracos (meta: i5 de 2ª geração, 4 GB de RAM, Lubuntu/Linux Mint/Linux Lite e Windows 10+). A loja opera sem depender da internet; a nuvem serve para conta, licença, backup e restauração. Há plano **Gratuito** (generoso, com limites) e plano **Pago**.

**Sobre o termo "ERP":** o produto cobre venda, estoque, financeiro e equipe de uma loja, o que o qualifica como ERP leve / sistema de gestão comercial. Não cobre (nesta fase) produção industrial, RH/folha, contabilidade completa nem emissão fiscal de saída. A marca pode usar "ERP" desde que as páginas públicas deixem claro o escopo. **Verificar disponibilidade do nome "Mikella" no INPI e em domínios antes de investir em marca.**

## Componentes do ecossistema

| Componente | O que é | Onde roda |
|---|---|---|
| **Landing page** | Apresentação, cadastro de conta, download | Web (estática/pré-renderizada) |
| **Portal da conta** | Login no navegador (usuário/e-mail + senha + 2FA por e-mail), plano, cobrança, dispositivos, download, início de restauração | Web |
| **Mikella Cloud** | API de conta, licença, cotas, cobrança, armazenamento de backup cifrado | Servidor do vendor |
| **Mikella Servidor da Loja** | Processo local que é **dono único** do banco SQLite da loja; expõe API na rede local; fala com a nuvem | 1 PC por loja |
| **Mikella Caixa (cliente)** | Interface dos demais PCs; sem banco próprio, só rascunhos locais | Até 8 PCs por loja (incluindo o servidor, que também pode operar caixa) |

O app desktop é um só instalador com dois papéis (servidor ou cliente), escolhidos na instalação.

## Stack decidida

- **Desktop:** Tauri 2 (núcleo Rust) + React + TypeScript + Tailwind.
- **Banco da loja:** SQLite (WAL), cifrado em repouso (decisão em `04_SEGURANCA`).
- **API local:** Rust (axum) dentro do Servidor da Loja, TLS na rede local.
- **Nuvem, portal e landing:** stack **[DECISÃO PENDENTE]**. Recomendação: serviço próprio e pequeno, reaproveitando os desenhos do Peditto (seção abaixo). Landing pré-renderizada (ex.: Astro).
- **Plataformas:** Windows 10+, Linux com WebKitGTK 4.1 (Ubuntu 22.04+ e derivadas), macOS depois (instalação manual sem assinatura Apple no início).
- **Gate técnico obrigatório:** spike de desempenho em hardware fraco real (ver Roadmap, MVP-0). Plano B se reprovar: Qt/QML.

## Decisões fechadas

| # | Decisão |
|---|---|
| D1 | Um **PC servidor por loja** é o banco oficial; os demais PCs acessam em tempo real pela rede local |
| D2 | Vários caixas vendendo ao mesmo tempo é requisito; nunca compartilhar arquivo SQLite por rede |
| D3 | Só o Servidor da Loja conversa com a nuvem |
| D4 | Backup cifrado **no cliente**; a nuvem não lê o conteúdo |
| D5 | Queda de luz/internet não pode perder nem reiniciar a sessão de caixa; ela retoma exatamente de onde parou |
| D6 | O servidor da loja também opera caixa (confirmar no spike) |
| D7 | Conta administrativa: usuário/e-mail + senha + 2FA por e-mail |
| D8 | Excluir produto = arquivar; **não devolve cota** |
| D9 | Limites de plano vêm de **configuração assinada pelo servidor (entitlements)**, nunca constantes no código |
| D10 | Foco inicial: Windows e Linux; macOS depois |
| D11 | Sem emissão fiscal de saída no MVP; importação de XML de NF-e de compra entra |

## Decisões em aberto

| # | Pergunta | Recomendação |
|---|---|---|
| P1 | Estoque negativo: bloquear ou permitir com alerta? | Permitir com alerta por padrão, configurável por loja |
| P2 | O que ocorre após 15 dias sem pagar? | **Não apagar o banco local.** Bloquear vendas, permitir leitura/exportação, apagar só a cópia na nuvem após aviso prévio. Exige termos de uso e revisão jurídica |
| P3 | Chave do backup: derivada de frase de recuperação do lojista (nuvem não consegue ler nem recuperar) ou recuperável pelo vendor? | Frase de recuperação (zero-knowledge) com aviso claro de que perdê-la perde o backup |
| P4 | Janela offline da licença (dias) | Parâmetro; sugestão inicial 7 a 14 dias, calibrar |
| P5 | 2FA por e-mail no desktop quando offline | 2FA na ativação, em novo dispositivo, na restauração e a cada N dias com internet; offline vale a janela da licença. Ver `04_SEGURANCA` |
| P6 | Cotas de **edição** e **exclusão** do plano Gratuito | Valores não definidos; parametrizar. Só criação (85/mês) está definida |
| P7 | Relatórios consolidados entre várias lojas (plano Pago) sem a nuvem ler dados | Opt-in de métricas agregadas ou consolidação no PC do dono. Decidir antes do MVP-6 |
| P8 | Limite de funcionários não administrativos (caixa, estoquista, vendedor) | Não definido; limitado só pelos 8 dispositivos simultâneos |
| P9 | O limite de 8 computadores vale por loja ou por conta? | Por loja (interpretação atual) |
| P10 | Histórico acima de 6 meses no plano Gratuito: como ocultar sem escrow de chave? | Ver `05_ANTI_FRAUDE_E_LICENCIAMENTO`, seção "Histórico bloqueado" |
| P11 | Stack da nuvem/portal | Decidir no MVP-0 |

## Roadmap por fases (MVP-0 a MVP-6)

Cada fase só termina quando seu gate for **comprovado** (teste ou medição), não quando o código existir.

| Fase (MVP) | Objetivo | Gate |
|---|---|---|
| **MVP-0 Esqueleto** | Domínio, SQLite, livro do caixa, estoque, venda atômica, **modo de 1 computador**, caixa operável, impressão de comprovante | Testes de queda de energia em HDD (kill -9 / corte de energia) sem perda nem divergência |
| **MVP-1 Vendável (piloto)** | MVP-0 + produtos e importação de planilha, validade com alerta, contas a pagar e a receber básicas, fechamento do caixa, backup **local** (segundo disco) e exportação, primeira execução, pacote de suporte | Sem nuvem. Licença manual (arquivo de licença assinado, emitido por você) |
| **MVP-2 Rede da loja** | Modo multi-PC (servidor, pareamento, TLS), concorrência de caixas | 3 a 8 caixas simultâneos sem corrupção nem venda duplicada |
| **MVP-3 Nuvem** | Conta, 2FA, ativação, licença automática, backup cifrado, restauração, kit de recuperação, landing e portal | Cadastro → download → ativação ponta a ponta; restauração em máquina limpa |
| **MVP-4 Planos** | Entitlements, cotas em cadeia, janela de histórico, sessão única, limite de dispositivos | Testes de bypass da lista de `05`; freemium 1 loja/85 produtos/6 meses/owner+1 manager |
| **MVP-5 Crescimento** | Comissões e comandas, promoções, anti-fraude de comissão | Cenários de fraude de `05` cobertos |
| **MVP-6 Multi-loja/NF-e** | Importação de XML de NF-e, conferência de recebimento, multi-loja | Conferência gera entrada de estoque e contas a pagar corretas |
| **Pós-MVP** | Hardening, assinatura de código, atualização automática, revisão independente | Checklist de `04` completo |

## Regras invioláveis

1. A **UI React nunca é autoridade**. Preço, total, desconto, estoque, papel, plano e cota são decididos no domínio (Rust), no servidor local ou na nuvem.
2. Todo dinheiro é **inteiro em centavos**. Nunca `float`.
3. Dados financeiros e de estoque são **movimentações imutáveis**; saldos são derivados.
4. Nada de limite de plano, URL, chave ou parâmetro operacional **hardcoded**.
5. Nada de segredo, PII, CPF ou token em log.
6. Todo comando recebido da UI passa por DTO com campos conhecidos (`deny_unknown_fields`) e validação.
7. Nenhuma operação de caixa pode ser perdida por queda de energia; escrita de valor usa `synchronous=FULL`.
8. Camadas só dependem para dentro (ver `03`). O domínio não importa Tauri, axum, SQLite nem HTTP.
9. Nenhuma biblioteca nova sem checar manutenção, licença, tamanho e impacto em RAM.
10. LGPD é requisito de desenho, não etapa final (ver `04`).

## Glossário

- **Loja:** unidade comercial com um banco e um Servidor da Loja.
- **Conta:** titular (CPF/CNPJ) na nuvem; pode ter 1 loja (Gratuito) ou até 8 (Pago).
- **Dono (host):** administrador máximo da conta.
- **Gerente/Admin:** administrador delegado com permissões configuradas pelo dono.
- **Entitlement:** conjunto de limites e permissões do plano, assinado pela nuvem.
- **Cota:** contador mensal de criações/edições/exclusões de produto.
- **Livro do caixa:** sequência imutável de movimentações (abertura, venda, sangria, suprimento, estorno, fechamento).
- **Comanda:** registro aberto pelo vendedor que liga itens ao vendedor para fins de comissão.
- **Lote:** quantidade de um produto com a mesma validade.

## Reaproveitamento do Peditto

| Do Peditto | Uso no Mikella |
|---|---|
| Cadastro por CPF/CNPJ, validação de dígitos, cifra AES-GCM + blind index HMAC | Conta na nuvem; um documento = uma conta gratuita |
| OTP/2FA, sessão com rotação e detecção de reuso, rate limit HMAC, anti-enumeração | Login na nuvem e ativação do desktop |
| Máquina de estados de acesso (trial, ativo, tolerância, suspenso, cancelado, retenção) | Estados da assinatura, com durações do Mikella |
| Faturas, checkout hospedado, webhook HMAC, ledger append-only, reconciliação | Cobrança do plano Pago |
| Outbox cifrado com retry | Lembretes e fila de sincronização |
| Padrões de código, ADRs, threat model, disciplina de "limite declarado" | Mesmo processo |
| **Não reaproveitar:** RLS/TenantContext no app local, cookie/CSRF/CORS no Tauri, rate limit por IP (CGNAT) | Substituídos por desenhos próprios |

## Convenções de documentação

- Cada documento declara o que **não** foi comprovado.
- Decisões relevantes viram ADR numerado (pasta `docs/decisions/`), com contexto, decisão, alternativas e consequências.
- Requisitos têm ID estável: `RF-` (funcional), `RNF-` (não funcional), `SEG-`, `FRD-` (fraude), `RES-` (resiliência), `PERF-`.
