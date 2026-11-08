# Mikella ERP — núcleo local

ERP local-first para loja de pequeno porte. O banco e as regras de negócio
rodam na máquina da loja; a nuvem (conta, licença e backup cifrado) entra depois.

Este diretório contém o **núcleo Rust** e os **assets estáticos**
(landing page e tela de caixa em HTML/CSS puro, sem dependências de rede).

## Estrutura

```
crates/
  shared-kernel/      Money, IDs tipados, erros, paginação, Clock
  domain-sales/       Venda, ItemVenda, Pagamento, regras de fechamento
  domain-cash/        SessaoCaixa, LivroCaixa, cadeia de hashes
  domain-inventory/   Produto, Lote, MovimentacaoEstoque, ports (traits)
  domain-finance/     (contratos iniciais)
  domain-people/      Usuario, Papel, Capacidade, ports
  domain-commission/  (contratos iniciais)
  domain-licensing/   (contratos iniciais)
  app-sales/          finalizar_venda
  app-cash/           abrir_caixa, fechar_caixa, registrar_sangria
  app-inventory/      criar_produto, atualizar_produto, arquivar_produto
  app-people/         criar_usuario, atualizar_usuario, desativar_usuario
  infra-sqlite/       schema, migrações, repositórios, hash blake3
  store-server/       HTTP local (loopback): /v1/health, /v1/login (Argon2id), /v1/usuarios
  desktop-shell/      casca Tauri (entry point + assets HTML/CSS integrados)
```

Regra de dependência: `domain` não conhece `infra`; `app` conhece `domain` e
portas; `infra` implementa as portas; o composition root monta tudo.

### Frontend (assets estáticos)

```
frontend/
  server.js                           # servidor HTTP estático (para preview)
  src/
    index.html                        # entry point Tauri → redirect app/landing.html
    app/
      landing.html                    # landing page (Modo Rua)
    features/
      caixa/
        ui/
          cash-spike.html             # tela de PDV (Modo Dia)
    shared/
      design/
        tokens.css                    # tokens de núcleo + temas (Dia/Noite/Rua)
        fontes.css                    # @font-face (woff2 local)
        halftone.svg                  # textura de pontos (< 2KB, Modo Rua)
        fontes/                       # woff2 Archivo + Source Sans 3 (~103KB)
      ui/
        forms.css                     # componentes (5 formas, botões, tabela, etc.)
```

**Identidade visual:** `docs/design/ASSETS.md` (autoria/licença) e `docs/design/galeria.md` (decisões).
Baseado em `BASE/08_IDENTIDADE_VISUAL_E_UX.md` — paleta cobalto/preto/branco,
zero border-radius, Corte (45°) como assinatura, 5 formas geométricas originais.

## Requisitos

- Rust 1.80+ (testado com 1.99.1)
- SQLite embutido via `rusqlite` (sem servidor de banco)
- Windows 10/11 ou Linux (Ubuntu, Mint, Zorin, Lubuntu, Linux Lite)

Windows 7 não é suportado.

## Comandos

```bash
# testes (inclui integração com SQLite real em memória)
cargo test --workspace --offline

# lint
cargo clippy --workspace --all-targets --offline

# servidor local
MIKELLA_DB_PATH=dados/loja.db MIKELLA_PORT=47823 cargo run -p mikella-store-server
```

No PowerShell:

```powershell
$env:MIKELLA_DB_PATH = "dados\loja.db"
$env:MIKELLA_PORT = "47823"
cargo run -p mikella-store-server
```

## store-server — HTTP local

Escuta apenas em `127.0.0.1` (loopback). Env vars:

| Variável      | Padrão  | Descrição |
|---------------|---------|------------|
| `MIKELLA_PORT` | `47823` | Porta TCP |
| `MIKELLA_DB_PATH` | (obrigatório) | Caminho para o arquivo SQLite |
| `MIKELLA_STORE_KEY` | (gerado) | Chave hex de 64 chars para o blind index de CPF/CNPJ. Se ausente, uma chave é gerada e persistida em `loja.key` ao lado do banco. **Nunca rotacione sem migrar os blind indexes.** |
| `MIKELLA_KEY_PATH` | `dados/loja.key` | Caminho alternativo do arquivo de chave da loja |

> O arquivo `loja.key` (ou o valor de `MIKELLA_STORE_KEY`) é necessário para
> buscar e comparar CPF/CNPJ pelo blind index. Guarde-o separado do backup do
> banco: quem tiver os dois consegue correlacionar os documentos.

### Rotas públicas (loopback)

```
GET  /v1/health
POST /v1/login
POST /v1/2fa/solicitar
POST /v1/2fa/verificar
GET  /v1/onboarding/status
POST /v1/onboarding
```

### Rotas protegidas (Bearer token)

```
GET  /v1/produtos              — lista produtos não arquivados (response: {"produtos": [...]})
POST /v1/produtos              — cria um produto (JSON body)
GET  /v1/produtos/:id          — busca produto por ID
PUT  /v1/produtos/:id          — atualização completa (JSON body)
POST /v1/produtos/:id/arquivar — marca produto como arquivado (soft delete)
POST /v1/caixa/abrir           — abre sessão de caixa (auto-provisioning de dispositivo)
POST /v1/caixa/fechar          — fecha sessão de caixa (verifica cadeia de hashes, idempotente)
GET  /v1/caixa/status           — status da sessão de caixa
POST /v1/vendas               — finaliza uma venda (itens, pagamentos)
GET  /v1/livro-caixa?sessao_id=<uuid> — entradas do livro (read model, CQRS)
GET  /v1/vendas                 — lista vendas (read model, CQRS)
GET  /v1/donation/prompt        — verifica prompt de doação
POST /v1/donation/ack           — marca doação (suprime futuros prompts)
```

Token ausente, prefixo errado ou sessão inválida → HTTP 401.

## O que já está implementado

- Schema `0001_initial` + migração `0002_app_preferences` + `0003_email_usuario` com tabelas `STRICT`, FKs, CHECKs e triggers.
- Migração com checksum blake3 (editar migração aplicada faz o banco recusar abertura).
- Repositórios SQLite de produtos, lotes, movimentações, vendas, sessões e livro do caixa.
- `finalizar_venda`: idempotência por UUID, preço vindo do banco, baixa de estoque,
  pagamento misto, lançamentos no livro e projeções por trigger.
- `abrir_caixa`, `fechar_caixa` e `registrar_sangria` com verificação da cadeia de hashes.
- **store-server MVP-1**: `GET /v1/health`, `POST /v1/login` (Argon2id),
  `GET /v1/onboarding/status`, `POST /v1/onboarding`,
  `GET /v1/caixa/status`, `GET /v1/livro-caixa`, `GET /v1/vendas`,
  `POST /v1/caixa/abrir` (abre caixa, auto-provisioning de dispositivo),
  `POST /v1/caixa/fechar` (verifica cadeia de hashes, idempotente).
  `POST /v1/vendas` (finaliza venda, pagamento misto, baixa de estoque).
- **Rotas de cadastro de produtos**: `POST /v1/produtos` (criar),
  `GET /v1/produtos/:id`, `PUT /v1/produtos/:id`, `POST /v1/produtos/:id/arquivar`.
- **Auth middleware**: Bearer token `mikella_<uuid-v4>` (CSPRNG), in-memory `sessoes`.
- **Single admin session**: login e 2FA revogam token anterior via `sessoes.retain()`.
- **2FA**: `POST /v1/2fa/solicitar` (código 6 dígitos, TTL 5 min),
  `POST /v1/2fa/verificar` (one-time use). `EmailBackend` trait injetável.
- **SEG-060**: Todos os DTOs de request usam `#[serde(deny_unknown_fields)]` — rejeita campos desconhecidos no JSON.
- **Donation prompt**: `GET /v1/donation/prompt`, `POST /v1/donation/ack`,
  tabela `app_preferences`, lógica a cada 3 dias + supressão mensal.
- **121 testes automatizados**, clippy e fmt limpos. Inclui 6 testes de integração
  para casos de uso de produto e 7 testes de repositório para `ProdutoRepo`/`buscar_por_sku`.
- **E2E local validado:** server → onboarding → login → 2FA → token → produtos → caixa (abrir/fechar/status) → vendas → livro → donation → ack → suppress.
- **Desktop-shell Tauri:** entry point (`main.rs`), `build.rs`, `tauri.conf.json` com
  allowlist mínima (SEG-060). Assets HTML/CSS servidos via protocolo `asset://`.
  Modo 1-computador (ARQ-001): UI carrega via Tauri e chama use cases via IPC.

## O que falta antes de ser um produto

1. Pareamento autenticado + TLS local — expor servidor na rede da loja (MVP-2).
2. **Rotas HTTP de venda, caixa e relatórios (com autorização por capacidade). MVP-1** ✅
3. **`POST /v1/vendas` (finalizar venda) — use case pronto; HTTP handler pronto. MVP-1** ✅
4. Backup cifrado, licença e cotas do plano gratuito — **bloqueado** por P6/P10/P2/P4/N1/N3/N4.
5. **Sistema de design implementado:** landing page (Modo Rua) e tela de
   caixa (Modo Dia) com CSS tokens, fontes woff2 locais (Archivo + Source Sans 3),
   textura halftone, e as 5 formas geométricas originais. Ver `docs/design/`.
6. Auditoria (`auditoria` já existe no schema, ainda sem repositório/caso de uso).
7. 2FA com provider de email real — **adiado** até decidir onde hospedar gratuitamente.

### Conta de teste (local)

Criar conta sem produto ou nome de loja (para alimentar depois):

```bash
curl -X POST http://127.0.0.1:47823/v1/onboarding \
  -d "nome=Teste&username=teste&senha=teste123"
```

Para testes E2E com produto pré-cadastrado: `cargo run -p seed`.

## Banco de desenvolvimento

O caminho padrão é `dados/loja.db`. Em desenvolvimento, se uma migração for
alterada antes de existir dado real, apague o arquivo e os arquivos `-wal` /
`-shm` para recriar. Em produção, nunca edite migração aplicada: crie a próxima.