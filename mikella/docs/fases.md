# Fases do Mikella ERP — roadmap consolidado (MVP-0 a MVP-6)

> **Documento consolidado.** Substitui as tabelas conflitantes de `00` (Fase 0-10)
> e `09` (MVP-0 a MVP-6). Adoptado conforme decisão em `09 §8.1`: **um único roadmap.**
>
> **Data de atualização:** 30/10/2026
>

Documento de precedência: `04/05 > 06 > 09 > 03 > 02 > 07 > 08 > 01`.

---

## 0. Piso de hardware (decisão fechada)

| Item | Valor |
|---|---|
| **CPU** | Intel Core i3 ou i5 de 5ª geração (Broadwell) mínimo |
| **RAM** | 6 GB DDR3-1333 |
| **Disco** | HDD 5400 rpm (pior caso) ou SSD barato |
| **Linux** | Lubuntu, Ubuntu, Lite, Mint, Zorin OS (WebKitGTK 4.1+) |
| **Windows** | Windows 10 / 11 **exclusivamente** (Windows 7 proibido) |
| **Compilação** | x86-64 baseline, sem AVX2 nem AES-NI exigido (detecção em runtime) |

---

## 1. Roadmap e gates

Cada fase só termina quando seu gate for **comprovado** (teste ou medição), não quando o código existir.

| MVP | Objetivo | Status | Gate |
|---|---|---|---|
| **0** | Domínio, SQLite, livro do caixa, estoque, venda atômica, **modo de 1 computador**, caixa operável, impressão de comprovante | ✅ | Testes de queda de energia em HDD |
| **MVP-1** | MVP-0 + **API local (store-server)**, onboarding, login (Argon2id), 2FA, caixa (abrir/fechar/status/sangria), vendas (pagamento multi-método, cancelamento), livro-caixa, **clientes e crediário** (cadastro, busca, liquidação), **menu intermediário do PDV (F1-F8)**, **gerenciamento de usuários** (hierarquia e catálogo fechado), **desktop-shell Tauri 2** (`mikella.exe` nativo), **impressão ESC/POS**, **backup local** e **diagnóstico** | ✅ | 172+ testes passando, clippy 0 warnings, Clean Architecture validada |
| **Piloto** | Homologação em 3 lojas reais por 30 dias (hardware real, sem dependência de nuvem, periféricos USB) | 🔄 Em Preparação (Próxima Etapa) | 30 dias sem divergência de caixa e tolerância comprovada a desligamento |
| **2** | Servidor da loja: TLS local, pareamento de PCs, concorrência de caixas (3-8) | 🔒 Bloqueada por Piloto | 3-8 caixas simultâneos sem corrupção |
| **3** | Conta na nuvem, portal, 2FA provider real, landing | 🔒 Adiada | Restauração em máquina limpa |
| **4** | Licença, entitlements, cotas em cadeia, janela de histórico, sessão única, limite de dispositivos | 🔒 Bloqueada por P6/P10/P2/P4/N1/N3/N4 | Testes de bypass |
| **5** | Comissões, comendas, promoções, anti-fraude de comissão | ⏳ Pendente | Cobertura de cenários de fraude |
| **6** | Importação XML NF-e, conferência de recebimento, multi-loja | ⏳ Pendente | Conferência gera entrada correta |
| **Pós-MVP** | UI Tauri (caixa, estoque, financeiro, equipe), backup cifrado, hardening, assinatura de código, atualização automática, revisão de segurança | ⏳ Pendente | Checklist `04` completo |

---

## MVP-0 — Esqueleto (COMPLETO ✅)

### Status: implementado e testado

**Domínio puro:**
- `Money` (centavos inteiros, aritmética com overflow checking)
- `SessaoCaixa` + livro imutável de movimentações
- `Venda` atômica (venda + baixa de estoque + movimentação de caixa em 1 transação)
- Produto, Lote, MovimentacaoEstoque

**SQLite:**
- Schema `0001_initial` (tabelas STRICT, FKs, CHECKs, triggers)
- Migração `0002_app_preferences` (donation prompt, app preferences)
- Migração `0003_email_usuario` (coluna email nullable)
- Triggers bloqueando UPDATE/DELETE em tabelas de movimento
- Cadeia de hash BLAKE3 (ABERTURA → VENDA → FECHAMENTO)

**Use cases implementados:**
- `finalizar_venda`: idempotência por UUID, preço vem do banco, baixa de estoque, pagamento misto
- `abrir_caixa`, `fechar_caixa`, `registrar_sangria` com verificação da cadeia de hashes

**Testes:** 121 testes automatizados passando (104 originais + 17 novos de pessoas)

### Gate MVP-0

- [x] Kill -9 durante milhares de vendas simuladas → nenhuma venda parcial, saldo confere
- [x] Corte de energia (em VM) durante venda e sangria → banco íntegro ao religar
- [x] Testes unitários: Money, livro do caixa, estoque (FEFO)

---

## MVP-1 — Vendável (COMPLETO ✅)

### Status: implementado e testado

**Auth e sessão:**
- `POST /v1/onboarding` + `GET /v1/onboarding/status`
- `POST /v1/login` (Argon2id + per-row salt via `SaltString::generate(&mut OsRng)`)
- `POST /v1/2fa/solicitar` (6 dígitos, TTL 5 min, one-time use)
- `POST /v1/2fa/verificar` (emite token Bearer)
- Auth middleware `exigir_token` (Bearer `mikella_<uuid-v4>`, CSPRNG)
- Single admin session via `sessoes.retain()`
- `EmailBackend` trait injetável + `ConsoleEmailBackend` (para testes locais)

**Gerenciamento de usuários (domain-people):**
- `POST /v1/usuarios` — cria usuário com controle de hierarquia:
  - DONO pode criar GERENTE, OPERADOR_CAIXA, VENDEDOR, ESTOQUISTA
  - GERENTE pode criar OPERADOR_CAIXA, VENDEDOR
  - Operadores não podem criar usuários (403)
- `GET /v1/usuarios` — lista usuários (ativos por padrão; `?ativos=false` para todos)
- `GET /v1/usuarios/:id` — busca por ID
- `PUT /v1/usuarios/:id` — atualiza nome e email (apenas DONO/GERENTE)
- `POST /v1/usuarios/:id/desativar` — desativa usuário (soft-delete)
- Domínio: enum `Papel` com capacidades granulares (`Papel::capacidades()`)
- Papel do criador resolvido do DB (não confiado do request)
- Senha hasheada server-side (Argon2id) — nunca chega em texto ao repositório

**Caixa (cash operations):**
- `GET /v1/caixa/status` — status da sessão aberta
- `POST /v1/caixa/abrir` — abertura (auto-provisioning de dispositivo via `garantir_dispositivo`)
- `POST /v1/caixa/fechar` — fechamento (verifica cadeia de hashes, idempotente)

**Vendas:**
- `POST /v1/vendas` — finaliza venda (itens, pagamentos, baixa de estoque)
- `GET /v1/vendas` — lista vendas (read model, CQRS)
- `GET /v1/livro-caixa` — entradas do livro com cadeia de hash

**Segurança:**
- `operador_id` derivado do usuário autenticado (anti-impersonation)
- Device auto-provisioning via `garantir_dispositivo()`
- Todos os DTOs de request usam `#[serde(deny_unknown_fields)]` (SEG-060) — rejeita campos desconhecidos
- Money em centavos (i64) com aritmética checked; Display pt-BR (R$ 129,90)

**Clean architecture e organização:**
- `main.rs` (~55 linhas): apenas setup do servidor e registro de rotas
- `state.rs`: AppState, EmailBackend trait, ConsoleEmailBackend
- `middleware.rs`: exigir_token, AuthenticatedUser, usuario_autenticado
- `dto/{auth,two_factor,cash,sales,response,donation,products}.rs`: DTOs tipados
- `handlers/{auth,cash,donation,health,sales,sales_write,products}.rs`: handlers HTTP
- `util/{http,senha,usuario}.rs`: helpers
- Camada de read model (CQRS) separada: handlers de listagem usam SQL direto
- ProdutoRepository trait no domínio; ProdutoRepo no infra-sqlite
- Clean crate separation: `domain-people`, `app-people`, `infra-sqlite/repos/pessoas.rs`

**Desktop Shell (Tauri) — integrado:**
- `crates/desktop-shell/`: entry point Tauri com `main.rs`, `build.rs`, `tauri.conf.json`
- Arq-001: modo 1-computador — UI chama casos de uso via comandos Tauri (sem HTTP interno)
- SEG-060: allowlist mínima (apenas `window` capability, protocolo `asset`)
- Assets HTML/CSS servidos via Tauri `asset://localhost` protocol
- `index.html` entry point → redirect para `app/landing.html`
- Compilação verificada: `cargo check` e `cargo clippy` limpos

**Arquivos chave do produto (inventário):**
- `domain-inventory/src/produto.rs`: entidade Produto com builder methods
- `domain-inventory/src/ports.rs`: ProdutoRepository trait
- `app-inventory/src/produto.rs`: casos de uso
- `store-server/src/handlers/products.rs`: handlers HTTP + read model
- `infra-sqlite/src/repos/produtos.rs`: ProdutoRepo (FTS5, triggers)

### Gate MVP-1

- [x] E2E local: onboarding → login → token → abrir caixa → vendas → livro-caixa → fechar caixa
- [x] E2E local: cadastro de produto → buscar produto → atualizar → arquivar
- [x] 121 testes passando | fmt limpo | clippy limpo
- [x] Integração use case de produto (SQLite em memória)
- [x] Testes de repositório: buscar_por_sku, FTS5, paginação, triggers
- [x] Integração de usuário: domínio (Papel, capacidades), repositório SQLite, handlers HTTP
- [x] Desktop-shell Tauri: compilação, config válida, assets integrados

### Concluído no MVP-1 e Recursos de Loja (100% Finalizado ✅)

- [x] **172 testes automatizados passando no workspace** | clippy 0 warnings
- [x] **Padrão UnitOfWork**: transações atômicas com rollback garantido em falhas (`infra-sqlite`)
- [x] **Importação de CSV (PROD-010)**: streaming de produtos com validação estrita (`/v1/produtos/importar-csv`)
- [x] **Contas a pagar/receber (RF-030)**: parcelas com liquidação atômica (`/v1/financeiro/contas-pagar` e `/v1/financeiro/contas-receber`)
- [x] **Catálogo Fechado de Capacidades (FRD-055)**: `CAPACIDADES_CONHECIDAS` com hierarquia DONO > GERENTE > OPERADORES
- [x] **Impressão Térmica ESC/POS (`infra-hardware`)**: comprovante de venda e de fechamento de caixa em 58mm/80mm, com adaptadores para impressora local USB/spooler RAW
- [x] **Cópia de Segurança Local em 1 Clique (`RES-030`)**: endpoint `POST /v1/sistema/backup-local` via SQLite `VACUUM INTO` com hash BLAKE3
- [x] **Diagnóstico Técnico sem PII (`OPS-001`)**: endpoint `GET /v1/sistema/diagnostico` com `PRAGMA integrity_check` e verificação da cadeia de hashes
- [x] **Frontend TUI do Caixa e Administrativo**: `landing.html`, `pos.html` (F1-F7, modal de troco, fechamento físico F6), `produtos.html`, `financeiro.html` e `main-menu.html` (F1-F5)
- [x] **Desktop-shell Tauri reconciliado**: endpoints essenciais e consumo do store-server local

### Próximas Tarefas (Entrada no Piloto Fechado em 3 lojas)

1. **Build de release e empacotamento**: compilação com `--release`, LTO e strip, e scripts de inicialização rápida da loja em 1 clique.
2. **Homologação com periféricos reais**: teste com impressora térmica USB e leitor de código de barras físico.
3. **Checklist de 30 dias em campo**: tolerância a queda forçada de energia (recuperação automática do banco via WAL) e validação diária de ausência de divergência de caixa.

---

## Spike visual — Identidade implementada ✅

**Documento base:** `BASE/08_IDENTIDADE_VISUAL_E_UX.md`
**Status:** Implementado seguindo rigorosamente todas as regras do documento.

### O que foi reescrito

O spike original (descrito como "podre" pelo usuário) violava praticamente todas as regras do documento `08`. Foi reescrito do zero com:

| Item | Antes (spike original) | Depois (seguindo 08) |
|---|---|---|
| **Cores** | `#0047ab`, tons inventados | `#0040D8` (azul-600 oficial), paleta fechada |
| **Formas** | Círculo, diamante, triângulo, hexágono, quadrado | Corte, Selo, Marca de estado, Régua, Letreiro (5 formas) |
| **Bordas** | `border-radius: 50%`, `4px`, `6px`, `8px` | **Zero** — `border-radius: 0` global (VIS-040) |
| **Assinatura** | Ausente | Corte-M (canto superior direito cortado 45°) |
| **Layout** | Grid de 6 cards + hero centralizado | Layout assimétrico, fita com réguas, ação primária única |
| **Fontes** | System font stack | Archivo (Display) + Source Sans 3 (Texto), WOFF2 local |
| **Transparência** | `rgba()` em backgrounds | Cores sólidas via tokens CSS |
| **Numeração** | Não tabular | Algarismos tabulares via `font-feature-settings` |
| **Estrutura** | CSS monolítico inline | `design/tokens.css`, `design/fontes.css`, `ui/forms.css` |

### Estrutura criada

```
frontend/
  server.js                           # servidor HTTP estático (para preview)
  src/
    index.html                        # entry point → redirect app/landing.html
    app/
      landing.html                    # landing page (Modo Rua) — Letreiro "A LUZ CAIU"
    features/
      caixa/
        ui/
          cash-spike.html             # tela de PDV (Modo Dia) — caixa operacional
    shared/
      design/
        tokens.css                    # core tokens + temas Dia/Noite/Rua
        fontes.css                    # @font-face (woff2)
        halftone.svg                  # textura de pontos (< 2KB, Rua apenas)
        fontes/                       # woff2 Archivo + Source Sans 3 (~103KB)
      ui/
        forms.css                     # componentes (5 formas, botões, tabela, etc.)
```

### Validações

- ✅ Zero `border-radius` em código (apenas reset `border-radius: 0`)
- ✅ Zero `rgba()` em código (apenas cores sólidas via tokens)
- ✅ Nenhuma cor inventada (verificado `grep` para `#0047ab` — elimidado)
- ✅ Fontes SIL OFL 1.1 (licença verificada — `docs/design/ASSETS.md`)
- ✅ Letreiro: 3 palavras, uma ocorrência, letras corretas (A-L-U-Z-C-A-I-U)
- ✅ Total do caixa: 72px sobre azul-600 sólido com Corte-M
- ✅ Números tabulares e alinhados à direita
- ✅ `prefers-reduced-motion` respeitado
- ✅ `[EXEMPLO]` markers em dados ilustrativos (`landing.html`, `cash-spike.html`)

---

## MVP-2 — Rede da loja (BLOQUEADA) 🔒

### Status: bloqueada — depende de MVP-3 (conta na nuvem)

**Bloqueado por:** TLS local (MVP-2) e pareamento de PCs — dependem de decisão de infraestrutura

**O que construir (planejado):**
- `store-server`: processo axum com TLS (rustls + rcgen — **removido do Cargo.toml até MVP-2**)
- Geração de certificado TLS na instalação
- Protocolo de pareamento: QR code / código numérico
- Idempotência: `idempotency_key` em todas as operações
- Concorrência: um escritor SQLite + leitores paralelos

### Gate MVP-2
- [ ] 3 a 8 caixas simultâneos por 30 min → sem corrupção, sem venda duplicada
- [ ] Reenvio com mesmo `idempotency_key` → mesmo resultado
- [ ] Caixa perde rede durante pagamento → reenvio ao reconectar

---

## MVP-3 — Conta na nuvem (ADICIONADA) 🔒

### Status: adiada — provider de email ainda não decidido

**Email provider adiado** até decidir onde hospedar gratuitamente (Resend, Mailgun free tier, etc.)

### O que existe para MVP-3:
- Store-server escuta em `127.0.0.1` apenas (`MIKELLA_PORT`, `MIKELLA_DB_PATH`)
- Token format: `mikella_<uuid-v4>` (CSPRNG), in-memory `sessoes`
- 2FA por email via trait injetável (ConsoleEmailBackend funciona)

---

## MVP-4 — Licença e entitlements (BLOQUEADA) 🔒

### Status: bloqueada — bloqueada por P6/P10/P2/P4/N1/N3/N4

**Bloqueado por:** License/quotas/billing — sem hardcoding de limites de plano (DECISÃO PENDENTE)

---

## MVP-5 a MVP-6 + Pós-MVP

Status: não implementado. Dependente de MVP-3 (nuvem) e MVP-4 (licença).

### Pós-MVP

- **UI Tauri (caixa, estoque, financeiro, equipe)** — integrando assets HTML/CSS com comandos Tauri
- Backup cifrado, hardening, assinatura de código, atualização automática
- Revisão de segurança (checklist `04`)

---

## Resumo de dependências bloqueadas

| Feature | Bloqueio |
|---|---|
| Email provider real | ADIADO — aguardando decisão sobre hospedagem gratuita |
| TLS local + pareamento | MVP-2 — dependente de infraestrutura |
| Licença/cotas/billing | P6/P10/P2/P4/N1/N3/N4 — sem hardcoding de limites |
| Landing page (redes sociais) | Última — aguardando referências visuais originais |
| Auditoria | MVP-1+ — schema existe, mas sem repositório/caso de uso |
| Contas a pagar/receber (RF-030) | MVP-5 — pendente |
| CSV import (PROD-010) | MVP-5 — pendente