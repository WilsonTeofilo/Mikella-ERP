# Mikella ERP — Plano de Fases, Arquitetura & Estado de Implementação

> **DOCUMENTO MESTRE DE PRODUTO, SEGURANÇA & ARQUITETURA**
>
> **Última Atualização:** 09 de Novembro de 2026
> **Baseline de código:** `2507916` (merge do PR #1, `develop` → `main`; branches sincronizadas) + P0 local ainda não commitado
> **Status Vigente:** Fases **F0 a F7 concluídas e verificadas**. **F8 em andamento**: backup local `VACUUM INTO` com hash BLAKE3, diagnóstico sem PII, teste de impressora e pacote de suporte `.zip` entregues. Os P0 de **restauração de backup**, **teste de queda de energia**, **fusos IANA**, **CSP sem estilo inline** e **expurgo LGPD operacional** estão implementados no código; faltam a homologação em hardware real, a revisão jurídica e o rebuild do shell. Auditoria de UX e Cadastros do PDV (AUD-01 a AUD-13) 100% resolvida. Auditoria de código/UI de 09/11 corrigiu XSS armazenado no resumo executivo, idempotência da venda no retry, crediário sem cliente cadastrado e pendências visuais do PDV (PAG-07/08/09/10/11 e VND-03). Em 11/11, os limites de parcelamento no cartão e o prazo do crediário passaram a ser configuráveis por loja (`loja_config`, migração `0012`).
> **Próxima Fila Imediata:** piloto de hardware (P0-5), parecer jurídico LGPD (P0-6), rebuild de `dist/` e commit/push dos P0.
> **Modelo Operacional:** Desktop 100% Local-First (Tauri 2 + Rust Axum + HTML/CSS/JS ES Modules + SQLite WAL). **O backend Rust é o cérebro**: autoridade total de preço, parcela, permissão e regra de negócio; a nuvem não processa vendas.
> **Documento de engenharia vinculante:** `mikella/AGENTS.md` (estrutura de pastas, proibições e checklist de entrega).

---

## 1. Visão Geral da Stack & Invariantes Inegociáveis

### 1.1. Arquitetura de Software
* **Desktop Shell:** Tauri 2 (Rust), sem decorações nativas, área de arraste customizada (`data-tauri-drag-region`), controles de janela oficiais via IPC e modo tela cheia inteligente (fade-out automático e fade-in na borda superior).
* **Backend Local (Loopback 127.0.0.1:47823):** Axum, Tokio, Tower-HTTP com allowlist estrita de CORS, timeouts e limites de requisição.
* **Persistência Relacional:** SQLite em modo WAL (`synchronous=FULL`, foreign keys ativas, escritor único serializado via Mutex).
* **Frontend Balcão:** HTML5 semântico, CSS Modular com Design Tokens (paleta Cobalto `#0040D8`, Preto `#000`, Branco `#FFF`, Amarelo `#FFD21F`, corte angular, zero border-radius) e JavaScript ES Modules organizado por feature. Proibidos frameworks pesados, monólitos HTML ou handlers inline.
* **Autoridade:** toda decisão de negócio vive em `domain-*` / `app-*`; `store-server` é adaptador HTTP e `infra-*` é detalhe plugável.

### 1.2. Invariantes de Negócio (LEI NO PROJETO)
1. **Dinheiro é SEMPRE Inteiro (`i64` centavos):** proibido float/double para valores monetários. Toda matemática financeira é exata e validada pelo tipo `Money`.
2. **Autoridade Server-Side:** a interface nunca calcula total oficial, preço de item, parcela ou saldo de caixa; tudo é calculado e revalidado pelo Rust.
3. **Livro-Caixa e Vendas Imutáveis (Append-Only):** movimentações de caixa e itens de venda são gravados em transação única e protegidos por triggers SQLite contra `UPDATE` e `DELETE`.
4. **Hierarquia Rígida de Papéis:** `DONO` > `GERENTE` > `OPERADOR_CAIXA` / `ESTOQUISTA` / `VENDEDOR`. Um papel só gerencia papéis de nível estritamente inferior.
5. **Autenticação com 2FA Mandatório:** login em duas etapas com OTP de 6 dígitos, TTL de 5 minutos e uso único.
6. **Zero Monólitos no Frontend:** nenhum HTML com `<style>` extenso, `<script>` inline ou `onclick=`. Comportamento em `features/<feature>/*.js`, estilo em `*.css`, HTTP em `shared/services/api.js`.
7. **Erro ao cliente não é diagnóstico:** resposta genérica e acionável; SQL, caminho, stack e token só no log (`SEC-017`).

### 1.3. Padrões de Projeto e Fronteiras (design patterns)
| Padrão | Onde vive | Regra prática |
|---|---|---|
| **Ports & Adapters** | `domain-*/ports` + `infra-sqlite` | domínio não conhece SQL, HTTP nem arquivo |
| **Use Case / Command** | `app-*` | uma operação por caso de uso; handler não decide regra |
| **Repository** | `infra-sqlite/src/repos` | SQL só aqui; nunca no handler |
| **Unit of Work** | composition root + `&Transaction` | quem grava mais de uma linha abre transação explícita (ADR-0002) |
| **Value Object** | `shared-kernel` (`Money`, `Documento`, IDs) | sem float, sem string solta carregando regra |
| **Builder** | `VendaBuilder` | soma dos pagamentos fecha exata; preço vem do banco |
| **State Machine** | `domain-cash` (`SessaoCaixa`) | transição inválida é recusada no domínio |
| **Strategy** | geradores ESC/POS, formas de pagamento | nova forma não vira `if` no handler |
| **Append-only Ledger + Hash Chain** | `livro_caixa` (ADR-0003) | `UPDATE`/`DELETE` bloqueados por trigger |
| **CQRS (read model)** | dashboard, livro, lista de vendas | leitura não recalcula regra de escrita |
| **DTO / Anti-Corruption Layer** | `store-server/src/dto` | `deny_unknown_fields`; DTO não vaza domínio |
| **Composition Root** | `store-server/state.rs`, `desktop-shell/lib.rs` | dependências montadas uma vez |

**Regra de ouro:** comportamento novo entra como **caso de uso em `app-*`**. Se o handler Axum precisa de SQL, de cálculo de total ou de `if` de permissão, o desenho está errado.

---

## 2. O que já dá para fazer localmente (Verificado de Ponta a Ponta)

| Fluxo / Funcionalidade | Situação | Como foi verificado e comprovado |
|---|---|---|
| **Executar Loja via `iniciar_loja.bat`** | ✅ Funciona | Executa `mikella-server.exe` (porta 47823) e abre o shell desktop `mikella.exe` sem erros de runtime |
| **Controles de Janela Nativos** | ✅ Funciona | Botões **Fechar (X)** e **Minimizar (_)** chamam a API oficial do Tauri 2; em **Tela Cheia (F11)** os botões somem em fade-out e reaparecem em fade-in ao passar o mouse no topo |
| **Login com Argon2id + 2FA** | ✅ Funciona | `/v1/login` valida credenciais e dispara OTP. O código 2FA **não vaza** na tela nem em console (`SEC-001`, `SEC-002`) |
| **Logout com Revogação Real** | ✅ Funciona | `POST /v1/auth/logout` invalida o token Bearer no servidor e limpa a sessão local (`SEC-011`) |
| **Confirmação de Logout no ESC (Menu Principal)** | ✅ Funciona | Teclar ESC no menu principal abre modal de confirmação [Enter confirma / Esc cancela], prevenindo logout acidental |
| **Abertura & Fechamento de Caixa** | ✅ Funciona | Status consultado em `/v1/caixa/status`. Fechamento cego nativo, conferência física e bobina com linha de assinatura |
| **Venda de Balcão (Direta / Comanda)** | ✅ Funciona | Bipagem de código de barras ou SKU, fita de itens atualizada dinamicamente com cálculo server-side |
| **Consulta Rápida de Preço (F9 / F6)** | ✅ Funciona | Menu do PDV (F9) e tela de venda (F6) abrem a mesma consulta somente leitura por código de barras, nome ou SKU; mostra preço e estoque sem adicionar item à venda |
| **Saída Limpa da Venda Vazia (ESC)** | ✅ Funciona | Carrinho vazio sai direto sem pergunta; carrinho com itens pede confirmação; venda com pagamentos parciais bloqueia saída até estorno/conclusão |
| **Pagamento Múltiplo e Parcelamento Padrão** | ✅ Funciona | Pagamento parcial reabre seleção F3 com saldo restante; parcelamento exibido no padrão comercial brasileiro (`11x de R$ 0,41 sem juros`) sem sobreposição CSS |
| **Bloqueio de Venda sem Caixa Aberto** | ✅ Funciona | Sistema recusa venda e bloqueia finalização caso o caixa esteja fechado (`UI-002`), exibindo badge dinâmico (`UI-004`) |
| **Grade de Pagamentos F1–F6** | ✅ Funciona | Ícones SVG nítidos e coloridos (Dinheiro, Débito, Crédito, Pix, Voucher, Crediário) tanto no atalho F3 quanto no fechamento F1 |
| **Histórico de Vendas & Estorno (F3)** | ✅ Funciona | Busca por COO ou data, conferência em 3 passos, motivo obrigatório, observação e autorização por gerente/dono com bobina térmica ESC/POS |
| **Fuso Horário IANA da Loja** | ✅ Funciona | Comprovantes térmicos, cupom de teste e dashboard usam `loja_config.fuso_horario` (ex.: `America/Sao_Paulo`, `America/Manaus`). O "hoje" financeiro é calculado no fuso da loja e comparado em UTC, eliminando o erro entre 21h e 0h |
| **Onboarding Inicial (Primeiro Acesso)** | ✅ Funciona | `GET /v1/onboarding/status` decide entre login e cadastro inicial; `app/onboarding.html` cria loja + dono via `POST /v1/onboarding` e dispara o primeiro código 2FA por e-mail |
| **Sangria com Aprovação Mandatória** | ✅ Funciona | Retirada de dinheiro exige justificativa e credenciais de gerente/dono quando executada por operador (`SEC-006`) |
| **Pagar Dívidas com Busca Instantânea (F6)** | ✅ Funciona | Carrega lista alfabética de devedores ao abrir; busca por prefixo/qualquer palavra do nome, CPF ou celular a partir de 2 dígitos |
| **Cadastro de Clientes com Endereço e LGPD** | ✅ Funciona | CEP automático via ViaCEP, número com botão `S/N`, opt-in LGPD desmarcado por padrão e isolado da base legal |
| **Cadastro de Produtos com Leitor e Estoque** | ✅ Funciona | Código de barras primeiro com autofocus, Enter do leitor não submete formulário, produto nasce com estoque inicial registrado em movimento imutável |
| **Dashboard Financeiro do Dono** | ✅ Funciona | Filtros de período (Hoje, 7 dias, Mês), KPIs dominantes (Faturamento, Margem, A Receber, A Pagar 7d, Caixa), gráficos SVG leves e lista "Cobrar Hoje" |
| **Backup Local Blindado** | ✅ Funciona | `POST /v1/sistema/backup-local` com `VACUUM INTO` restrito a `MIKELLA_BACKUP_DIR`, recusa sobrescrita e devolve tamanho + hash BLAKE3 (`SEC-005`) |
| **Diagnóstico Técnico sem PII** | ✅ Funciona | `GET /v1/sistema/diagnostico` devolve integridade SQLite, versão, contagens e estado do schema sem dados de cliente (`OPS-001`) |
| **Teste de Impressora Térmica** | ✅ Funciona | `POST /v1/sistema/teste-impressora` imprime cupom de teste no fuso da loja e devolve erro genérico (`SEC-017`) |
| **Migrações de Banco Não-Destrutivas** | ✅ Funciona | Migrações aditivas (`0004` reescrita sem `DROP`, `0007`–`0010` aplicadas sem perda) com checksum BLAKE3 (`DAT-001`) |

---

## 3. Matriz da Auditoria de UX e Cadastros (AUD-01 a AUD-13) · ✅ Concluída

| Identificador | Onde | Requisito da Auditoria | Resolução Técnica |
|---|---|---|---|
| **AUD-01** | PDV | Esc não sai da venda vazia | Venda vazia sem pagamentos sai direto para o menu; com itens pede confirmação; com pagamento parcial já lançado trava e exige estorno. |
| **AUD-02** | F6 PDV | Busca de devedor não retorna nada | Lista completa carregada imediatamente ao abrir; filtro instantâneo por nome, CPF e celular; empty state claro. |
| **AUD-03** | Clientes | Consentimento LGPD pré-marcado | Checkbox opt-in desmarcado por padrão para WhatsApp/SMS; base legal do Art. 7º, V separada e explícita. |
| **AUD-04** | Produtos | Produto nasce sem estoque | Campo obrigatório `Quantidade que está entrando` salvando movimento de entrada inicial imutável no SQLite. |
| **AUD-05** | Produtos | Cadastro começa pelo nome | Código de barras no topo com autofocus; Enter do leitor busca dados sem submeter o formulário. |
| **AUD-06** | Clientes | Endereço em campo único | Endereço decomposto: CEP com busca automática, logradouro, número com atalho `S/N`, complemento, bairro, cidade, UF. |
| **AUD-07** | Todos | Autocomplete sugerindo dados | `autocomplete="off"`, `autocorrect="off"`, `autocapitalize="off"`, `spellcheck="false"` e IDs neutros em todos os formulários. |
| **AUD-08** | Financeiro | Tabela vazia sem inteligência | Dashboard executivo com filtro Hoje/7d/Mês, 5 KPIs essenciais, gráficos SVG leves e lista acionável "Cobrar Hoje". |
| **AUD-09** | Global | Três estilos visuais conflitantes | Identidade unificada: tokens `tokens.css`, cabeçalho padrão preto/cobalto com atalho `[ESC] Voltar`. |
| **AUD-10** | Global | Falhas de layout e sobreposição | Eliminado triângulo azul de 64px, logo flutuante ajustado, classe `.campo` corrigida para não sobrepor inputs/parcelas. |
| **AUD-11** | Pagamento / PDV | Cálculo de parcelas e responsividade do modal | Modal de pagamento com parcelamento em centavos exatos (`11x (1x de R$ 0,45 + 10x de R$ 0,41) • Total: R$ 4,55`) sem off-by-one ou corte visual. |
| **AUD-12** | Financeiro | Falta de filtros de pagamento | Aba "Visão Geral & Vendas": filtros rápidos (Hoje/Semana/Mês/De-Até), 6 cards analíticos (Faturamento Bruto, Lucro Bruto, Pix, Cartão, Dinheiro, Vendas) e tabela detalhada com lucro no fuso IANA da loja. |
| **AUD-13** | LGPD & Hardware | Anonimização de titular e teste de bobina térmica | Botão de anonimização LGPD (Art. 18, com retenção fiscal de 5 anos) no cadastro de clientes e botão "Testar Impressora" no PDV. |

---

## 4. O Plano de Fases Revisado (F0 a F13)

```
[F0]  ✅ Fundação, Desktop Shell & Arquitetura Base
[F1]  ✅ Autenticação, 2FA, Sessão & Logout Seguro
[F2]  ✅ Frente de Caixa, Venda Balcão & Pagamentos F1-F6
[F3]  ✅ Histórico de Vendas, Estorno & Movimentação de Gaveta
[F4]  ✅ Desacoplamento Arquitetural & Anti-Monólito Rust
[F5]  ✅ Proteção de Dados, LGPD & Blind Index de Clientes
[F6]  ✅ Módulo Financeiro, Crediário & Contas da Loja
[F7]  ✅ Trilha de Auditoria, Fechamento Cego & Bobina Física
──────────────────────────────────────────────────────────────────
[F8]  🟡 Resiliência Comprovada & Homologação em Hardware Real  <-- EM ANDAMENTO
[F9]  🔒 Operação de Balcão Completa (devolução, vale, inventário, clientes)
[F10] 🔒 Comandas, Vendedor Comissionado & Regras de Comissão
[F11] 🔒 Lotes, Validade & Alertas Ativos de Perecíveis
[F12] 🔒 Fiscal, Maquininha & Conciliação de Meios de Pagamento
[F13] 🔒 Nuvem: Licença, Entitlements, Backup Offsite & Portal da Conta
```

### F8 — Resiliência Comprovada & Homologação em Hardware Real · 🟡 Em andamento
**Objetivo:** provar, com medição, que o caixa sobrevive a queda de energia e que a release roda no piso de hardware.

- [x] **Backup Local de 1 Clique (`SEC-005`):** `VACUUM INTO` restrito a diretório seguro, sem sobrescrita, com tamanho + hash BLAKE3.
- [x] **Diagnóstico Técnico sem PII (`OPS-001`):** integridade SQLite, versão, contagens e schema; nenhum dado de cliente.
- [x] **Cupom de Teste de Impressora:** usa o fuso IANA da loja e não expõe detalhe de hardware (`SEC-017`).
- [x] **Restauração Assistida (`P0-1`):** `infra-sqlite/src/backup.rs` verifica BLAKE3, `integrity_check`, FKs, schema, contagens e as cadeias de livro/auditoria; CLI `--verificar-backup` e `--restaurar-backup` preservam o banco anterior em `.pre-restauracao-<ts>.db`. Endpoints `POST /v1/sistema/backup/verificar` e `GET /v1/sistema/backups` restritos a `MIKELLA_BACKUP_DIR`; testes automatizados de restauração e recusa de backup corrompido.
- [x] **Teste de Queda Abrupta de Energia (`P0-2`):** WAL com `synchronous=FULL` comprovado em teste; imagem de queda com transação não commitada reabre mantendo só o commitado e passa no `integrity_check`. Roteiro físico de desligamento segue no benchmark.
- [x] **CSP sem estilo inline (`P0-4`):** 319 `style=` migrados para classes utilitárias, 6 `cssText` substituídos; `style-src 'self'` no `tauri.conf.json`; `verificar-frontend.ps1` reprova `<style>`, `style=`, `cssText` e `innerHTML` com template multi-linha interpolado.
- [x] **Auditoria de código/UI (09/11):** `resumo.js` deixou de montar HTML/SVG por `innerHTML` com dado do servidor (SEC-008), `venda_id` passou a ser a chave estável do rascunho (ADR-0002) e o crediário bloqueia cliente sem `id` cadastrado. PDV corrigido em PAG-07/08/09/10/11 e VND-03.
- [x] **Expurgo LGPD Operacional (`P0-6` técnico):** `infra-sqlite/src/lgpd.rs` + `POST /v1/sistema/lgpd/expurgo` (simulação por padrão, mínimo de 5 anos, anonimização com auditoria atômica). Documentado em `mikella/docs/LGPD-RETENCAO.md`.
- [x] **Pacote de Suporte (`OPS-001`):** `POST /v1/sistema/suporte/pacote` gera `.zip` com `diagnostico.json`, `ambiente.json`, migrações, `logs.txt` higienizado (CPF/CNPJ, e-mail, token, senha e caminho redigidos), `manifesto.json` com BLAKE3 e `README.txt`. ZIP *stored* escrito em Rust, sem nova dependência; nunca sobrescreve pacote anterior; pasta `MIKELLA_SUPPORT_DIR` (default `suporte`). Ação “PACOTE DE SUPORTE” no menu principal para DONO e Gerente. Documentado em `mikella/docs/PACOTE-SUPORTE.md`.
- [ ] **Homologação de Hardware (`P0-5`):** i3/i5 de 5ª geração, 6 GB, HDD 5400 rpm; medir PERF-001/002, tempo de abertura, rolagem da fita e custo de `clip-path` com `scripts/benchmark-hardware.ps1`.
- [ ] **Piloto de 30 Dias:** operação contínua, fechamento diário 100% conciliado, zero perda de centavos.
- [x] **Configuração por loja (PAG pendente):** `loja_config` (migração `0012`) guarda `max_parcelas_cartao` (1–24, padrão 12) e `prazo_crediario_dias` (1–365, padrão 30). `GET /v1/config/pagamento` alimenta o PDV; `PUT /v1/sistema/config-pagamento` grava com auditoria `CONFIG_PAGAMENTO`; a venda revalida os limites no servidor.
- **Gate:** restauração reproduzível + relatório de queda de energia + medições de hardware anexadas.

### F9 — Operação de Balcão Completa · 🔒 Bloqueada por F8
**Objetivo:** fechar o ciclo comercial do pequeno varejo sem planilha paralela.

- [ ] **Devolução e Troca:** caso de uso próprio, motivo obrigatório, recomposição de estoque e lançamento no livro (nunca `UPDATE` em venda finalizada — ADR-0002).
- [ ] **Vale-Crédito:** crédito rastreável por cliente, com validade e baixa no livro; não confundir com crediário.
- [ ] **Inventário Cego:** contagem sem saldo visível, divergência gera movimentação imutável de ajuste com aprovação.
- [ ] **Mesclagem de Clientes Duplicados:** consolidar cadastro preservando histórico, com trilha de auditoria e sem perder vínculo fiscal.
- [ ] **Busca de Clientes em Volume:** medir os índices de prefixo já criados; se necessário, FTS5 com `unicode61` e *fallback* por prefixo.
- **Gate:** cada operação tem caso de uso, teste de integração com SQLite real e trilha de auditoria.

### F10 — Comandas, Vendedor Comissionado & Comissão · 🔒 Bloqueada por F9
**Objetivo:** suportar balcão/mesas com atribuição transparente de comissão.

- [ ] **Comanda Física:** bipagem do número, associação de itens e fechamento no caixa.
- [ ] **Vendedor por Item:** código do vendedor no item comissionável; operador não pode apropriar comissão alheia (antifraude).
- [ ] **Regras de Comissão:** percentual por produto/categoria, cálculo exato em `domain-commission` e fechamento por período.
- [ ] **Relatório de Comissão:** por vendedor, com estorno e cancelamento refletidos.
- **Gate:** comissão calculada só no backend; teste de conflito de vendedor e de venda cancelada.

### F11 — Lotes, Validade & Alertas de Perecíveis · 🔒 Bloqueada por F9
**Objetivo:** reduzir perda de perecíveis antes do vencimento.

- [ ] **Lote com Validade:** entrada obrigatória de validade quando a categoria exigir.
- [ ] **Alertas Ativos:** aviso no login de gerente/dono para itens em 60/30/7 dias, com ação sugerida.
- [ ] **Aferição Física:** confirmação de inspeção na prateleira para arquivar o alerta (`UX-035`).
- [x] **Vencimentos no Fuso da Loja:** `date('now')` removido das regras de vencimento, validade e devedores; a data local é resolvida no servidor a partir de `loja_config.fuso_horario` (P0-3).
- **Gate:** alerta não duplica, não expira sozinho e a data é a da loja.

### F12 — Fiscal, Maquininha & Conciliação de Meios · 🔒 Bloqueada por F9
**Objetivo:** decidir e implementar o escopo fiscal sem acoplar o núcleo.

- [x] **Persistência de Detalhes de Cartão:** `parcelas`, `maquininha`, `modalidade` e `taxa_centavos` já existem em `venda_pagamentos` e no domínio.
- [ ] **UI de Maquininha:** capturar modalidade, parcelas e taxa na tela de pagamento (hoje o backend aceita, a UI não expõe).
- [ ] **Conciliação por Modalidade:** relatório de taxas e valores liquidados por maquininha; refletir taxa no resultado (DRE).
- [ ] **Decisão Fiscal Formal:** documento curto definindo "PDV não fiscal" **ou** NFC-e.
- [ ] **`infra-fiscal` Plugável:** se NFC-e, trait de emissão + certificado A1 + contingência, sem regra fiscal no handler.
- [ ] **TEF:** fora desta fase; registrar como backlog (`P2`).
- **Gate:** decisão registrada em ADR; se NFC-e, emissão testada em homologação da SEFAZ.

### F13 — Nuvem: Licença, Entitlements, Backup Offsite & Portal · 🔒 Bloqueada por F12
**Objetivo:** transformar o desktop em produto licenciado e recuperável.

- [ ] **Licença Ed25519:** token assinado com chave pública embutida e suporte a duas chaves para rotação (`FRD-001`).
- [ ] **Entitlements e Cotas:** limites de lojas, admins, produtos e janela de histórico lidos de configuração, nunca *hardcoded* (`FRD-002`).
- [ ] **Backup Cifrado Offsite:** enviar cópia cifrada + âncora de hash do livro para a nuvem (`06`, `05`).
- [ ] **Portal da Conta:** plano, dispositivos com `Revogar`, download e restauração (`08 §11.4`).
- [ ] **Atualização Assinada:** canal de update com verificação de assinatura.
- [ ] **E-mail Real de 2FA:** provedor definido (hoje modo console em desenvolvimento).
- **Gate:** licença não pode ser forjada sem a chave privada; backup restaurado em máquina limpa.

---

## 5. Fechamento desta rodada (07/11/2026) · ✅ Concluído

- [x] **Onboarding na UI:** `landing.html` consulta `/v1/onboarding/status`; banco sem loja redireciona para `onboarding.html`, que cria o dono e o primeiro 2FA. Separação HTML/CSS/JS em `features/auth/` (zero inline).
- [x] **Fuso IANA ponta a ponta:** `loja_config.fuso_horario` é lido no servidor e injetado em todos os geradores ESC/POS (venda, estorno, fechamento e cupom de teste). O fallback fixo UTC-3 foi removido; fuso inválido cai em `America/Sao_Paulo` com log.
- [x] **Dashboard sem erro de meia-noite:** faturamento, CMV, meios de pagamento e lista de entradas filtrados por intervalo UTC parametrizado, calculado a partir da data local no fuso da loja. Datas de entrada validadas (`AAAA-MM-DD`) antes do SQL.
- [x] **Testes de fuso:** `America/Sao_Paulo`, `America/Manaus` e fuso inválido cobertos em `infra-hardware`.
- [x] **SEC-017 no cupom de teste:** detalhe de hardware só no log; resposta ao usuário é acionável e genérica.
- [x] **Esteira verde:** `cargo test --workspace --tests`, clippy e `verificar-frontend.ps1`.
- [x] **Release publicada:** `dist` reconstruído e smoke test E2E (onboarding → login → resumo) no binário empacotado.
- [x] **Git:** commit `0fcf8d1`, push para `develop`, PR #1 (`develop → main`) mergeado em `2507916` com título e descrição preenchidos.

---

## 6. Próximos Passos Priorizados

Prioridade por **risco × valor para o piloto**: primeiro o que pode perder dinheiro ou dados, depois o que fecha a operação, por último o que escala o produto.

### P0 — Bloqueiam o piloto (fazer nesta ordem)

| # | Item | Por que agora | Critério de aceite | Status |
|---|---|---|---|---|
| **P0-1** | **Restauração de backup testada** | Hoje o sistema gera backup, mas não prova que ele restaura. Backup não testado não é backup. | Teste automatizado: restaurar arquivo, abrir em banco temporário, conferir hash e `integrity_check = ok`; roteiro manual documentado. | ✅ Código + testes; falta rodar em hardware real |
| **P0-2** | **Teste de queda abrupta de energia** | A maior promessa do produto ("o caixa volta de onde parou") ainda não foi medida (`06 §8`). | Venda em andamento + desligamento forçado + reinício: WAL recuperado, venda íntegra ou ausente, cadeia de hashes válida, saldo igual ao esperado. | ✅ Teste de imagem de queda; falta desligamento físico roteirizado |
| **P0-3** | **Vencimentos no fuso da loja** | Ainda há `date('now')` (UTC) em contas a pagar/receber, validade e devedores; entre 21h e 0h o filtro erra o dia. | Nenhum `date('now')` em regra de vencimento; data local resolvida no servidor; teste com `America/Manaus` e `America/Sao_Paulo`. | ✅ `date('now')` removido de vencimentos/devedores/validade; helpers com testes |
| **P0-4** | **CSP sem `style-src 'unsafe-inline'`** | A CSP existe, mas ainda permite estilo inline; `SEC-009` está só parcialmente fechado. | Remover estilos inline remanescentes ou usar hash/nonce; WebView2 e WebKitGTK validados; `verificar-frontend.ps1` verde. | ✅ 319 estilos migrados; CSP `style-src 'self'`; gate verde |
| **P0-5** | **Homologação no piso de hardware** | Sem medição, PERF-001/002 e o custo de `clip-path` continuam hipótese (`07`, `08 §17.1`). | Relatório com RAM/CPU/FPS no i3/i5 de 5ª geração, HDD 5400 rpm e telas 1024x600 / 1366x768 a 125%. | 🟡 Script pronto; falta executar no hardware |
| **P0-6** | **Revisão jurídica LGPD** | Retenção fiscal de 5 anos, anonimização e contrato loja × Mikella precisam de validação profissional (`10`). | Parecer registrado; textos de consentimento e política revisados; prazos refletidos no código. | 🟡 Expurgo e retenção no código/docs; falta parecer jurídico |

### P1 — Fecham a operação de loja

| # | Item | Por que | Critério de aceite |
|---|---|---|---|
| **P1-1** | **Devolução/troca + vale-crédito** | Toda loja devolve; hoje só existe estorno de venda. | Casos de uso próprios, estoque recomposto, livro atualizado, aprovação por gerente e comprovante. |
| **P1-2** | **Inventário cego** | Divergência de estoque sem contagem cega vira desconfiança. | Contagem sem saldo visível, ajuste imutável e relatório de divergência. |
| **P1-3** | **Mesclar clientes duplicados** | CPF repetido polui crediário e LGPD. | Merge com trilha, sem perder histórico nem violar unicidade do documento. |
| **P1-4** | **Busca de clientes em volume** | Índices de prefixo existem; falta medir. | Busca < 100 ms com 10 mil clientes; FTS5 só se a medição exigir. |
| **P1-5** | **Comandas + vendedor comissionado** | Receita direta para lojas com balcão/mesas. | Comissão calculada no backend e resistente a troca de vendedor. |
| **P1-6** | **Lotes e validade com alertas** | Perecível é perda recorrente no pequeno varejo. | Alerta por 60/30/7 dias no fuso da loja, com aferição física. |
| **P1-7** | **UI de maquininha + conciliação** | O backend já persiste parcelas/taxa; a UI não expõe e o DRE não concilia. | Modalidade, parcelas e taxa na venda; relatório por maquininha; taxa no resultado. |

### P2 — Escala o produto (depois do piloto)

| # | Item | Observação |
|---|---|---|
| **P2-1** | **Licença Ed25519 + entitlements + cotas** | Depende da nuvem; bloqueia plano gratuito/pago (`05`). |
| **P2-2** | **Backup cifrado offsite + âncora de hash** | Fecha a proteção contra adulteração local (`ADR-0003`). |
| **P2-3** | **Portal da conta, dispositivos e restauração** | Landing já existe; portal é o próximo passo comercial. |
| **P2-4** | **Atualização assinada** | Requisito de segurança da cadeia de distribuição. |
| **P2-5** | **E-mail real de 2FA** | Hoje `MIKELLA_EMAIL_CONSOLE=1` em desenvolvimento. |
| **P2-6** | **Fiscal / NFC-e** | Decidir antes de prometer; `infra-fiscal` plugável. |
| **P2-7** | **TEF** | Só depois de conciliação de maquininha estável. |
| **P2-8** | **Marca, wordmark, ícones e galeria de componentes** | `08 V1/V3/V9`, `docs/design/ASSETS.md`; registrar INPI antes de investir. |
| **P2-9** | **Auditoria WCAG 2.2 AA e contraste no CI** | `08 §13`, `VIS-013`; script de contraste em `tools/contrast/`. |

---

## 7. Gates de Qualidade (definição de pronto)

Nenhuma fase termina "porque o código existe". Termina quando o gate é **comprovado**.

### 7.1 Esteira obrigatória
```powershell
# Frontend: sem monólitos, sem handler inline, referências válidas, JS válido
powershell -ExecutionPolicy Bypass -File mikella/scripts/verificar-frontend.ps1

# Rust
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --tests
```

### 7.2 Critérios de aceite de qualquer entrega
- [ ] Regra de negócio em `domain-*`/`app-*`, nunca no handler nem no JS.
- [ ] DTO com `deny_unknown_fields`; SQL 100% parametrizado.
- [x] Erro ao cliente genérico; detalhe só no log (`SEC-017`). Aplicado em backup,
  verificação de backup, pacote de suporte, diagnóstico e LGPD: a resposta traz
  apenas nome de arquivo, `ok`/`falha` e rótulo curto; SQL, caminho absoluto e
  stack ficam em `tracing::warn`.
- [ ] Teste automatizado (unitário ou integração com SQLite real) cobrindo o caminho feliz **e** a recusa.
- [ ] Migração aditiva; nunca editar migração já aplicada (checksum BLAKE3).
- [ ] Frontend sem `<style>`/`<script>` inline e sem `on*=`; acesso HTTP via `shared/services/api.js`.
- [ ] `dist` reconstruído quando HTML/CSS/JS mudar (assets são embutidos no binário).
- [ ] `git diff` revisado; sem `.env`, banco, binário, token ou PII.
- [ ] ADR criado/atualizado quando a decisão for estrutural.

### 7.3 Critérios de produto (piloto)
- [ ] Caixa fecha 100% conciliado por 30 dias.
- [ ] Nenhuma venda perdida em queda de energia ou internet.
- [ ] Nenhum erro 500 com detalhe interno exposto.
- [ ] Backup gerado **e restaurado** com sucesso.

---

## 8. Segurança & LGPD — Estado e Pendências

### 8.1 Já implementado
| Controle | Evidência |
|---|---|
| Senha com Argon2id + salt CSPRNG | `/v1/login` |
| Token Bearer CSPRNG, sessão única | `mikella_<uuid-v4>`, revogação no logout |
| 2FA OTP 6 dígitos, TTL 5 min, uso único | `/v1/2fa/*` |
| SQL parametrizado + `deny_unknown_fields` | todos os DTOs |
| Livro de caixa append-only + cadeia BLAKE3 | triggers + `verificar_integridade` |
| Blind index de CPF/CNPJ | `0008_blind_index_cpf.sql` + `loja.key` |
| Backup restrito + hash BLAKE3 | `/v1/sistema/backup-local` |
| Erros sem SQL/caminho/stack | `SEC-017` |
| LGPD: exportação do titular e anonimização com guarda fiscal | `/v1/clientes/:id/exportar`, `DELETE /v1/clientes/:id` |
| CSP ativa no shell | `tauri.conf.json` |

### 8.2 Pendências
- [x] **CSP:** `'unsafe-inline'` removido de `style-src`; estilos inline migrados e gate `verificar-frontend.ps1` verde (P0-4).
- [ ] **Logs:** revisar se algum `tracing` ainda registra CPF, valor de pagamento ou credencial.
- [ ] **LGPD jurídica:** prazos, base legal e contrato controlador/operador (P0-6). O expurgo técnico e `docs/LGPD-RETENCAO.md` já existem.
- [ ] **Chave da loja:** documentar rotação e separação do backup (quem tem banco + chave correlaciona documentos).
- [x] **Retenção:** rotina explícita de expurgo após o prazo fiscal, com simulação, aplicação transacional e trilha `EXPURGO_LGPD`.

---

## 9. Decisões Abertas (não inventar valor fixo)

| # | Decisão | Impacto | Recomendação |
|---|---|---|---|
| **D1** | Escopo fiscal: PDV não fiscal ou NFC-e | `infra-fiscal`, certificado, contingência | Decidir antes de prometer ao cliente; se NFC-e, trait plugável |
| **D2** | Hospedagem da nuvem e do e-mail 2FA | licença, backup offsite, custo | Provedor gratuito/barato até tração; definir antes do F13 |
| **D3** | Preço e limites dos planos | entitlements e landing | Fechar junto com o F13 |
| **D4** | TEF vs. maquininha manual | conciliação e suporte | Maquininha primeiro; TEF só com demanda comprovada |
| **D5** | Busca global `Ctrl+K` | navegação | Só se o piloto mostrar usuários perdidos (`08 V5`) |
| **D6** | Atalhos F2–F10 | produtividade do caixa | Validar com operadores reais (`08 V6`) |
| **D7** | Marca/wordmark/ícones | identidade e INPI | Pesquisar disponibilidade antes de investir (`08 V1/V3/V9`) |
| **D8** | Modo Noite como padrão | conforto e contraste | Manter Dia como padrão (`08 V2`) |

---

## 10. Matriz de Rastreabilidade de Achados Técnicos

| Identificador | Categoria | Descrição do Achado | Alocação no Plano | Status |
|---|---|---|---|---|
| **SEC-001** | Segurança | Código 2FA vazava na resposta de login | Fase F1 | ✅ Concluído |
| **SEC-002** | Segurança | Envio de e-mail 2FA para console por padrão | Fase F1 | ✅ Concluído |
| **SEC-003** | Segurança | CORS permissivo em loopback | Fase F1 | ✅ Concluído |
| **SEC-004** | Segurança | Cancelamento sem consumo de autorização | Fase F3 | ✅ Concluído |
| **SEC-005** | Segurança | Backup com caminho arbitrário e delete prévio | Fase F8 | ✅ Backend + UI; verificação e restauração testadas (P0-1) |
| **SEC-006** | Segurança | Sangria de caixa com autoaprovação | Fase F3 | ✅ Concluído |
| **SEC-007** | Segurança | Hashes serializados em `Usuario`/`PinHash` | Fase F1 | ✅ Concluído |
| **SEC-008** | Segurança | XSS armazenado via interpolações `innerHTML` | Fase F0 / F2 | ✅ Concluído |
| **SEC-009** | Segurança | CSP restritiva no shell Tauri | Fase F5 | ✅ CSP ativa com `style-src 'self'`, sem estilo inline (P0-4) |
| **SEC-011** | Segurança | Falta de endpoint `POST /v1/auth/logout` | Fase F1 | ✅ Concluído |
| **SEC-013** | Segurança | CPF/CNPJ exposto a operadores comuns | Fase F5 | ✅ Concluído |
| **SEC-014** | Segurança | CPF em texto claro sem blind index | Fase F5 | ✅ Concluído |
| **SEC-017** | Segurança | Erros internos com `e.to_string()` vazando SQL | Fase F4 | ✅ Concluído |
| **ARQ-001** | Arquitetura | Queries SQL diretas nos handlers Axum | Fase F4 | ✅ Concluído |
| **DAT-001** | Dados | Migração 0004 com `DROP TABLE` destrutivo | Fase F3 | ✅ Concluído |
| **DAT-004** | Dados | Falta de checks de valor positivo e FKs em parcelas | Fase F6 | ✅ Concluído |
| **DAT-010** | Domínio | Perda de sinal negativo em frações de `Money` | Fase F2 | ✅ Concluído |
| **UI-001** | Interface | Perda de foco ao digitar pagamentos mistos | Fase F2 | ✅ Concluído |
| **UI-002** | Interface | Venda com `sessao_id` fabricado se caixa fechado | Fase F2 | ✅ Concluído |
| **UI-004** | Interface | Badge "SESSÃO PROTEGIDA" estático | Fase F2 | ✅ Concluído |
| **LGPD-002** | Privacidade | Tabela `auditoria` vazia sem gravações | Fase F7 | ✅ Concluído |
| **LGPD-003** | Privacidade | Falta de exportação estruturada do titular | Fase F5 | ✅ Concluído |
| **LGPD-004** | Privacidade | Anonimização irreversível com guarda fiscal de 5 anos | Fase F5 | ✅ Concluído |
| **LGPD-005** | Privacidade | Falta de expurgo periódico por retenção | Fase F8 | ✅ Simulação/aplicação com auditoria atômica; parecer jurídico pendente (P0-6) |
| **AUD-01..13** | UX / PDV / Fin | Resolução de 13 não-conformidades de UX e parcelamento | Fase F2/F6/F7 | ✅ Concluído |
| **OPS-001** | Suporte | Pacote de diagnóstico com PII | Fase F8 | ✅ JSON sem PII e `.zip` de suporte com log higienizado, manifesto BLAKE3 e auditoria |
| **PERF-001/002** | Desempenho | Metas de RAM/CPU não medidas no piso | Fase F8 | 🟡 Script de benchmark pronto; falta executar no hardware (P0-5) |
| **RES-001** | Resiliência | Queda de energia não testada | Fase F8 | ✅ Teste de imagem de queda; falta roteiro físico (P0-2) |
| **TZ-001** | Correção | `date('now')` UTC em vencimentos | Fase F11 | ✅ Data local IANA em vencimentos, devedores e validade (P0-3) |

---

## 11. Backlog de Produto (fora do núcleo do MVP)

Itens conscientemente adiados para não inflar o escopo. Não entram em promessa de venda antes de virarem fase.

- TEF / integração direta com adquirente.
- NFC-e / SAT / impressora fiscal (depende da decisão **D1**).
- Multi-loja e consolidação entre filiais.
- Aplicativo móvel para o dono.
- Integração com marketplace ou emissão de NF-e de compra.
- Mascote/personagem de marca (`08 V3`).
- Busca global `Ctrl+K` (`08 V5`).
- Relatórios avançados com data warehouse local.

---

## 12. Referências

| Documento | Uso no plano |
|---|---|
| `mikella/AGENTS.md` | contrato de engenharia, estrutura e checklist |
| `mikella/docs/fases.md` | roadmap consolidado MVP-0..6 e gates |
| `mikella/docs/decisions/0001..0004` | SQLite, venda atômica, livro com hash, pagamento misto |
| `BASE/03_ARQUITETURA_CLEAN_E_PADROES.md` | clean architecture e anti-monólito |
| `BASE/04_SEGURANCA.md` | SEC/SEG e LGPD |
| `BASE/05_ANTI_FRAUDE_E_LICENCIAMENTO.md` | FRD, entitlements e cotas |
| `BASE/06_RESILIENCIA_E_BACKUP.md` | WAL, backup e testes de queda |
| `BASE/07_OTIMIZACAO_PC_FRACO.md` | PERF-001/002, orçamentos |
| `BASE/08_IDENTIDADE_VISUAL_E_UX.md` | VIS/UX, design system e acessibilidade |
| `BASE/09_LACUNAS_PISO_DE_HARDWARE_E_MVP.md` | corte de MVP e decisões abertas |
| `BASE/10_LGPD_CONFORMIDADE.md` | direitos do titular e retenção |
| `mikella/docs/LGPD-RETENCAO.md` | política técnica de retenção e expurgo |
| `mikella/docs/PACOTE-SUPORTE.md` | conteúdo, endpoint e revisão do pacote de suporte |
| `BASE/Mikella ERP — Auditoria do fluxo de pagamento do PDV.md` | regras de pagamento misto/parcelamento |
| `mikella/docs/auditoria/AUDITORIA-2026-11-04.md` | correções de frontend/Tauri e pendências manuais |
