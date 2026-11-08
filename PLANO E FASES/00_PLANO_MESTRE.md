# Mikella ERP — Plano de Fases, Arquitetura & Estado de Implementação

> **DOCUMENTO MESTRE DE PRODUTO, SEGURANÇA & ARQUITETURA**
>
> **Última Atualização:** 07 de Novembro de 2026  
> **Status Vigente:** Fases F0 a F7 **concluídas e verificadas**. Auditoria de UX e Cadastros do PDV (AUD-01 a AUD-13) 100% resolvida. Onboarding inicial exposto na UI (cria `instance_identity` + dono e dispara o primeiro 2FA). Comprovantes térmicos e dashboard usam o **fuso IANA da loja** (`loja_config.fuso_horario`), não mais UTC-3 fixo. Confirmação de desconexão ativa no ESC do Menu Principal. Esteira automatizada `cargo test --workspace` e `verificar-frontend.ps1` 100% verde. Release empacotada em `mikella\dist\`.
> **Próxima Fila Imediata:** Fase F8 (Backup Protegido, Diagnóstico Técnico & Homologação em Hardware Real).
> **Modelo Operacional:** Desktop 100% Local-First (Tauri 2 + Rust Axum + HTML/CSS/JS ES Modules + SQLite WAL). Autoridade total de negócio no Rust; a nuvem não processa vendas.

---

## 1. Visão Geral da Stack & Invariantes Inegociáveis

### 1.1. Arquitetura de Software
* **Desktop Shell:** Tauri 2 (Rust), sem decorações nativas, área de arraste customizada (`data-tauri-drag-region`), controles de janela oficiais via IPC e modo tela cheia inteligente (fade-out automático e fade-in na borda superior).
* **Backend Local (Loopback 127.0.0.1:47823):** Axum, Tokio, Tower-HTTP com allowlist estrita de CORS, timeouts e limites de requisição.
* **Persistência Relacional:** SQLite em modo WAL (`synchronous=FULL`, foreign keys ativas, escritor único serializado via Mutex).
* **Frontend Balcão:** HTML5 semântico, CSS Modular com Design Tokens (paleta Cobalto `#0040D8`, Preto `#000`, Branco `#FFF`, Amarelo `#FFD21F`, corte angular Persona 3 Reload, zero border-radius) e JavaScript ES Modules organizado por feature (`features/pos`, `features/auth`, `features/products`, `features/finance`, etc.). Proibidos frameworks pesados, monolitos HTML ou handlers inline.

### 1.2. Invariantes de Negócio (LEI NO PROJETO)
1. **Dinheiro é SEMPRE Inteiro (`i64` centavos):** Proibido uso de float/double para valores monetários. Toda matemática financeira é exata e validada pelo tipo de domínio `Money`.
2. **Autoridade Server-Side:** A interface nunca calcula total oficial, preço de item ou saldo de caixa; tudo é calculado pelo Rust.
3. **Livro-Caixa e Vendas Imutáveis (Append-Only):** Movimentações de caixa e itens de venda são gravados via transação única e protegidos por triggers SQLite contra `UPDATE` e `DELETE`.
4. **Hierarquia Rígida de Papéis:** `DONO` > `GERENTE` > `OPERADOR_CAIXA` / `ESTOQUISTA` / `VENDEDOR`. Um papel só gerencia papéis de nível estritamente inferior.
5. **Autenticação com 2FA Mandatório:** Login em duas etapas com código descartável OTP de 6 dígitos e expiração em 5 minutos.
6. **Zero Monólitos no Frontend:** Nenhum arquivo HTML pode conter blocos `<style>` ou `<script>` internos extensos; toda lógica reside em módulos `.js` e `.css` dedicados.

---

## 2. O que já dá para fazer localmente (Verificado de Ponta a Ponta)

Fluxo operacional testado e comprovado no executável nativo e backend local:

| Fluxo / Funcionalidade | Situação | Como foi verificado e comprovado |
|---|---|---|
| **Executar Loja via `iniciar_loja.bat`** | ✅ Funciona | Executa `mikella-server.exe` (porta 47823) e abre o shell desktop `mikella.exe` sem erros de runtime |
| **Controles de Janela Nativos** | ✅ Funciona | Botões **Fechar (X)** e **Minimizar (_)** chamam a API oficial do Tauri 2; em **Tela Cheia (F11)** os botões somem em fade-out e reaparecem em fade-in ao passar o mouse no topo |
| **Login com Argon2id + 2FA** | ✅ Funciona | `/v1/login` valida credenciais e dispara OTP. O código 2FA **não vaza** na tela nem em console (`SEC-001`, `SEC-002`) |
| **Logout com Revogação Real** | ✅ Funciona | `POST /v1/auth/logout` invalida o token Bearer no servidor e limpa a sessão local (`SEC-011`) |
| **Confirmação de Logout no ESC (Menu Principal)** | ✅ Funciona | Teclar ESC no menu principal abre modal de confirmação [Enter confirma / Esc cancela], prevenindo logout acidental |
| **Abertura & Fechamento de Caixa** | ✅ Funciona | Status consultado em `/v1/caixa/status`. Fechamento cego nativo, conferência física e bobina com linha de assinatura |
| **Venda de Balcão (Direta / Comanda)** | ✅ Funciona | Bipagem de código de barras ou SKU, fita de itens atualizada dinamicamente com cálculo server-side |
| **Saída Limpa da Venda Vazia (ESC)** | ✅ Funciona | Carrinho vazio sai direto sem pergunta; carrinho com itens pede confirmação; venda com pagamentos parciais bloqueia saída até estorno/conclusão |
| **Pagamento Múltiplo e Parcelamento Padrão** | ✅ Funciona | Pagamento parcial reabre seleção F3 com saldo restante; parcelamento exibido no padrão comercial brasileiro (`11x de R$ 0,41 sem juros`) sem sobreposição CSS |
| **Bloqueio de Venda sem Caixa Aberto** | ✅ Funciona | Sistema recusa venda e bloqueia finalização caso o caixa esteja fechado (`UI-002`), exibindo badge dinâmico (`UI-004`) |
| **Grade de Pagamentos F1–F6** | ✅ Funciona | Ícones SVG nítidos e coloridos (Dinheiro, Débito, Crédito, Pix, Voucher, Crediário) tanto no atalho F3 quanto no fechamento F1 |
| **Histórico de Vendas & Estorno (F3)** | ✅ Funciona | Busca por COO ou data, conferência em 3 passos, motivo obrigatório, observação e autorização por gerente/dono com bobina térmica ESC/POS |
| **Fuso Horário IANA da Loja** | ✅ Funciona | Comprovantes térmicos, cupom de teste e dashboard usam `loja_config.fuso_horario` (ex.: `America/Sao_Paulo`, `America/Manaus`). O "hoje" financeiro é calculado no fuso da loja e comparado em UTC, eliminando o erro entre 21h e 0h. |
| **Onboarding Inicial (Primeiro Acesso)** | ✅ Funciona | `GET /v1/onboarding/status` decide entre login e cadastro inicial; `app/onboarding.html` cria loja + dono via `POST /v1/onboarding` e dispara o primeiro código 2FA por e-mail. |
| **Sangria com Aprovação Mandatória** | ✅ Funciona | Retirada de dinheiro exige justificativa e credenciais de gerente/dono quando executada por operador (`SEC-006`) |
| **Pagar Dívidas com Busca Instantânea (F6)** | ✅ Funciona | Carrega lista alfabética de devedores ao abrir; busca por prefixo/qualquer palavra do nome, CPF ou celular a partir de 2 dígitos |
| **Cadastro de Clientes com Endereço e LGPD** | ✅ Funciona | CEP automático via ViaCEP, número com botão `S/N`, opt-in LGPD desmarcado por padrão e isolado da base legal |
| **Cadastro de Produtos com Leitor e Estoque** | ✅ Funciona | Código de barras primeiro com autofocus, Enter do leitor não submete formulário, produto nasce com estoque inicial registrado em movimento imutável |
| **Dashboard Financeiro do Dono** | ✅ Funciona | Filtros de período (Hoje, 7 dias, Mês), KPIs dominantes (Faturamento, Margem, A Receber, A Pagar 7d, Caixa), gráficos SVG leves e lista "Cobrar Hoje" |
| **Migrações de Banco Não-Destrutivas** | ✅ Funciona | Migração `0004_financeiro.sql` reescrita de forma aditiva e `0007_parcelas_numero.sql` aplicada sem perda de registros (`DAT-001`) |

---

## 3. Matriz da Auditoria de UX e Cadastros (AUD-01 a AUD-10) · ✅ Concluída

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
| **AUD-09** | Global | Três estilos visuais conflitantes | Identidade Persona 3 Reload unificada: tokens `tokens.css`, cabeçalho padrão preto/cobalto com atalho `[ESC] Voltar`. |
| **AUD-10** | Global | Falhas de layout e sobreposição | Eliminado triângulo azul de 64px, logo flutuante ajustado, classe `.campo` corrigida para não sobrepor inputs/parcelas. |
| **AUD-11** | Pagamento / PDV | Cálculo de parcelas e responsividade do modal | Redesign neo-brutalista Persona 3 do modal de pagamento; parcelamento em centavos exatos no padrão adquirente (`11x (1x de R$ 0,45 + 10x de R$ 0,41) • Total: R$ 4,55`) sem off-by-one ou corte visual. |
| **AUD-12** | Financeiro | Falta de filtros de pagamento do Chokko-Melt | Aba 1 "Visão Geral & Vendas": filtros rápidos (Hoje/Semana/Mês/De-Até), 6 cards analíticos (Faturamento Bruto, Lucro Bruto, Pix, Cartão, Dinheiro, Vendas) e tabela detalhada de entradas com lucro no fuso IANA da loja. |
| **AUD-13** | LGPD & Hardware | Anonimização de titular e teste de bobina térmica | Botão de anonimização LGPD (Art. 18 c/ retenção fiscal de 5 anos) no cadastro de clientes e botão "Testar Impressora" no topo do PDV chamando `/v1/sistema/teste-impressora`. |

---

## 4. O Plano de Fases Atualizado (F0 a F10)

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
[F8]  🟡 Backup Protegido, Diagnóstico & Piloto de 30 Dias (Hardware Real) <-- FILA IMEDIATA
[F9]  🔒 Venda por Comandas & Identificação de Vendedor Comissionado
[F10] 🔒 Alertas Ativos de Validade de Perecíveis & Gestão de Lotes
```

---

### Fechamento desta rodada (07/11/2026) · ✅ Concluído

- [x] **Onboarding na UI:** `landing.html` consulta `/v1/onboarding/status`; banco sem loja redireciona para `onboarding.html`, que cria o dono e o primeiro 2FA. Separação HTML/CSS/JS em `features/auth/` (zero inline).
- [x] **Fuso IANA ponta a ponta:** `loja_config.fuso_horario` é lido no servidor e injetado em todos os geradores ESC/POS (venda, estorno, fechamento e cupom de teste). O fallback fixo UTC-3 foi removido; fuso inválido cai em `America/Sao_Paulo` com log.
- [x] **Dashboard sem erro de meia-noite:** o faturamento, CMV, meios de pagamento e a lista de entradas são filtrados por intervalo UTC parametrizado, calculado a partir da data local no fuso da loja. Datas de entrada são validadas (`AAAA-MM-DD`) antes de chegar ao SQL.
- [x] **Testes de fuso:** `America/Sao_Paulo`, `America/Manaus` e fuso inválido cobertos em `infra-hardware`.
- [x] **SEC-017 no cupom de teste:** detalhe de hardware só no log; resposta ao usuário é acionável e genérica.

### Pendências conhecidas (ordem sugerida)

1. **F8 — Backup e homologação real:** `VACUUM INTO` com checksum/restauração, pacote de diagnóstico e teste de queda de energia em hardware modesto.
2. **Operação de balcão ainda ausente:** devolução/troca, vale-crédito, inventário cego e mesclagem de clientes duplicados.
3. **Pagamento com maquininha:** o servidor ainda recebe apenas forma + valor; nº de parcelas, modalidade e taxa precisam ser persistidos e conciliados (sem TEF nesta fase).
4. **Fiscal:** decisão formal de escopo (PDV não fiscal) e, se houver NFC-e, crate `infra-fiscal` plugável com certificado A1.
5. **LGPD jurídica:** validar com advogado/contador os prazos de retenção e o contrato loja (controladora) × Mikella (operador).
6. **Busca de clientes em volume:** avaliar FTS5/índice por prefixo antes do piloto com muitos cadastros.
7. **Vencimentos no fuso da loja:** os cards de contas a pagar/receber, validade de lote e busca de devedores ainda usam `date('now')` (UTC) para comparar datas; migrar para a data local da loja no próximo ciclo.

---

### F8 — Backup Protegido, Diagnóstico & Piloto de 30 Dias (Hardware Real) · 🟡 Fila Imediata
**Objetivo:** Validação contínua do sistema em lojas piloto reais de pequeno varejo por 30 dias contínuos em PC modesto (i3/i5 antigo, 4–6 GB RAM).

- [ ] **Backup Local de 1 Clique Blindado (`SEC-005`):** `VACUUM INTO` restrito a diretório seguro com geração de checksum SHA-256 e teste de restauração automatizado.
- [ ] **Pacote de Diagnóstico Técnico:** Geração de arquivo `.zip` para suporte contendo schema, integridade do banco e logs operacionais higienizados sem dados de clientes.
- [ ] **Teste de Queda Abrupta de Energia:** Validação da recuperação do WAL do SQLite e da cadeia do livro-caixa após desligamento forçado durante uma venda.
- [ ] **Critério de Sucesso do Piloto:** 30 dias de operação contínua sem travamentos, sem perda de centavos e com fechamento de caixa diário 100% conciliado.

---

### F9 — Venda por Comandas & Identificação de Vendedor Comissionado · 🔒 Bloqueada
**Objetivo:** Suportar operação com comandas físicas de balcão/mesas e atribuição transparente de comissão a vendedores.

- [ ] **Fluxo F2 de Venda por Comanda:** Bipagem do número de comanda física, associação de itens e fechamento no caixa.
- [ ] **Identificação de Vendedor Comissionado:** Bipagem ou digitação do código do vendedor ao passar produtos comissionáveis, com antifraude impedindo que o operador aproprie a comissão.
- [ ] **Regras de Comissionamento por Categoria/Produto:** Cálculo exato no backend baseado no plano da loja.

---

### F10 — Alertas Ativos de Validade de Perecíveis & Gestão de Lotes · 🔒 Bloqueada
**Objetivo:** Prevenir perdas financeiras em produtos perecíveis antes do vencimento.

- [ ] **Controle de Lotes com Data de Validade:** Lançamento de lote e data de expiração no cadastro/entrada de estoque.
- [ ] **Disparo de Alertas Ativos:** Aviso no login de gerente/dono para itens com vencimento próximo (ex.: 60 dias), com recomendação de remarcação/promoção.
- [ ] **Confirmação de Aferição Física:** Exigência de confirmação física de inspeção na prateleira para arquivar o alerta.

---

## 5. Matriz de Rastreabilidade de Achados Técnicos

| Identificador | Categoria | Descrição do Achado | Alocação no Plano | Status |
|---|---|---|---|---|
| **SEC-001** | Segurança | Código 2FA vazava na resposta de login | **Fase F1** | ✅ Concluído |
| **SEC-002** | Segurança | Envio de e-mail 2FA para console por padrão | **Fase F1** | ✅ Concluído |
| **SEC-003** | Segurança | CORS permissivo em loopback | **Fase F1** | ✅ Concluído |
| **SEC-004** | Segurança | Cancelamento sem consumo de autorização | **Fase F3** | ✅ Concluído |
| **SEC-005** | Segurança | Backup com caminho arbitrário e delete prévio | **Fase F8** | ✅ Corrigido no backend |
| **SEC-006** | Segurança | Sangria de caixa com autoaprovação | **Fase F3** | ✅ Concluído |
| **SEC-007** | Segurança | Hashes serializados em `Usuario`/`PinHash` | **Fase F1** | ✅ Concluído |
| **SEC-008** | Segurança | XSS armazenado via interpolações `innerHTML` | **Fase F0 / F2** | ✅ Concluído |
| **SEC-009** | Segurança | Ausência de CSP restritiva no shell Tauri | **Fase F5** | ✅ Concluído |
| **SEC-011** | Segurança | Falta de endpoint `POST /v1/auth/logout` | **Fase F1** | ✅ Concluído |
| **SEC-013** | Segurança | CPF/CNPJ de clientes exposto a operadores comuns | **Fase F5** | ✅ Concluído |
| **SEC-014** | Segurança | CPF em texto claro sem blind index calculado | **Fase F5** | ✅ Concluído |
| **SEC-017** | Segurança | Erros internos com `e.to_string()` vazando SQL | **Fase F4** | ✅ Concluído |
| **ARQ-001** | Arquitetura | Queries SQL diretas dentro dos handlers Axum | **Fase F4** | ✅ Concluído |
| **DAT-001** | Dados | Migração 0004 com `DROP TABLE` destrutivo | **Fase F3** | ✅ Concluído |
| **DAT-004** | Dados | Falta de checks de valor positivo e FKs em parcelas | **Fase F6** | ✅ Concluído |
| **DAT-010** | Domínio | Perda de sinal negativo em frações de `Money` | **Fase F2** | ✅ Concluído |
| **UI-001** | Interface | Perda de foco ao digitar pagamentos mistos | **Fase F2** | ✅ Concluído |
| **UI-002** | Interface | Venda com `sessao_id` fabricado se caixa fechado | **Fase F2** | ✅ Concluído |
| **UI-004** | Interface | Badge "SESSÃO PROTEGIDA" estático | **Fase F2** | ✅ Concluído |
| **LGPD-002** | Privacidade | Tabela `auditoria` vazia sem gravações | **Fase F7** | ✅ Concluído |
| **LGPD-003** | Privacidade | Falta de exportação estruturada do titular | **Fase F5** | ✅ Concluído |
| **LGPD-004** | Privacidade | Anonimização irreversível com guarda fiscal de 5 anos | **Fase F5** | ✅ Concluído |
| **AUD-01..13**| UX / PDV / Fin | Resolução de 13 não-conformidades de UX, Chokko-Melt e Parcelamento | **Fase F2/F6/F7** | ✅ Concluído |
