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
  domain-finance/     Contas, parcelas, crediário
  domain-people/      Cliente, Usuario, Papel, Capacidade, ports
  domain-commission/  Regras de comissão
  domain-licensing/   Licença, entitlements (contratos)
  app-sales/          finalizar_venda, cancelamento
  app-cash/           abrir_caixa, fechar_caixa, sangria
  app-inventory/      produtos, lotes, estoque
  app-people/         usuários, clientes, crediário
  app-finance/        contas a pagar/receber, parcelas
  app-licensing/      validação de licença
  infra-sqlite/       schema, migrações, repositórios, hash blake3, backup, LGPD
  infra-cloud/        contratos de nuvem/licenciamento
  infra-crypto/       primitivas criptográficas
  infra-hardware/     impressora ESC/POS
  store-server/       API HTTP local (loopback), handlers, DTOs e middleware
  desktop-shell/      casca Tauri (entry point + assets HTML/CSS integrados)
```

Regra de dependência: `domain` não conhece `infra`; `app` conhece `domain` e
portas; `infra` implementa as portas; o composition root monta tudo.

### Frontend (assets estáticos)

```
frontend/
  server.js                           # servidor HTTP estático (para preview)
  src/
    index.html                        # entry point → app/landing.html
    app/                              # somente HTML de entrada
      landing.html, onboarding.html, pos.html, pos-menu.html,
      produtos.html, financeiro.html, main-menu.html
    features/                         # JS/CSS por tela
      auth/, navigation/, pos/, products/, finance/, financeiro/,
      caixa/, estoque/, comissoes/, equipe/, ajustes/
    shared/
      design/                         # tokens, fontes, utilitários, halftone
      services/api.js                 # HTTP + token + base URL
      ui/                             # forms, feedback, payment-icons, window-controls
```

A estrutura exata e as regras de fronteira estão em `AGENTS.md`. Não crie
`utils.js` genérico; cada módulo tem responsabilidade única.

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
GET  /v1/produtos              — lista produtos não arquivados
POST /v1/produtos              — cria um produto
GET  /v1/produtos/:id          — busca produto por ID
PUT  /v1/produtos/:id          — atualização completa
POST /v1/produtos/:id/arquivar — arquiva (soft delete)
POST /v1/import/csv            — importação de produtos por CSV (PROD-010)
GET/POST /v1/clientes          — lista e salva clientes
GET  /v1/clientes/:id/exportar — portabilidade LGPD (JSON do titular)
DELETE /v1/clientes/:id        — eliminação/anonimização LGPD
GET  /v1/crediario/devedores   — devedores do crediário
POST /v1/crediario/liquidar    — liquida dívida do crediário
GET/POST /v1/usuarios          — usuários, papéis e capacidades
POST /v1/caixa/abrir|fechar|sangria — operações de caixa
GET  /v1/caixa/status          — status da sessão de caixa
GET  /v1/livro-caixa           — entradas do livro (read model, CQRS)
GET/POST /v1/vendas            — lista e finaliza vendas
GET  /v1/vendas/simular-parcelas — planos de parcelamento (limite da loja)
POST /v1/vendas/:id/cancelar|estornar — cancelamento e estorno
GET/POST /v1/vendas/:id/comprovante|imprimir
POST /v1/vendas/autorizar-cancelamento
GET  /v1/finance/resumo|fluxo-caixa|resultado|contas-abertas
POST /v1/finance/contas-pagar|contas-receber
POST /v1/finance/contas-*/:id/parcelas/:idx/liquidar
POST /v1/sistema/backup-local  — backup `VACUUM INTO` + BLAKE3
POST /v1/sistema/backup/verificar — validação de backup
GET  /v1/sistema/backups       — lista backups autorizados
POST /v1/sistema/lgpd/expurgo  — simulação/aplicação de anonimização
GET  /v1/config/pagamento      — limites de parcelas e prazo do crediário
PUT  /v1/sistema/config-pagamento — altera os limites (DONO, auditado)
GET  /v1/sistema/diagnostico   — integridade sem PII
POST /v1/sistema/suporte/pacote — ZIP de suporte (OPS-001)
POST /v1/sistema/teste-impressora
GET  /v1/sistema/cep/:cep
GET/POST /v1/donation/prompt|ack
```

Token ausente, prefixo errado ou sessão inválida → HTTP 401.

## O que já está implementado

- Schema com migrações versionadas (`0001`–`0012`), checksums BLAKE3, tabelas
  `STRICT`, FKs, CHECKs e triggers que bloqueiam UPDATE/DELETE em movimento.
- Repositórios SQLite de produtos, lotes, movimentações, vendas, sessões,
  livro do caixa, clientes, contas, parcelas e auditoria.
- `finalizar_venda`: idempotência por UUID, preço vindo do banco, baixa de
  estoque, pagamento misto, lançamentos no livro e projeções por trigger.
- `abrir_caixa`, `fechar_caixa` e `registrar_sangria` com verificação da cadeia.
- **API local completa**: auth (Argon2id + 2FA), usuários e capacidades, caixa,
  vendas e cancelamento, clientes/crediário, contas a pagar/receber, CSV,
  backup/restauração/verificação, diagnóstico, pacote de suporte, expurgo LGPD
  e limites de pagamento por loja (parcelas de cartão e prazo do crediário).
- **Auth middleware** Bearer `mikella_<uuid-v4>` com sessão em memória e
  revogação da sessão anterior.
- **DTOs** com `#[serde(deny_unknown_fields)]` e erros ao cliente sem SQL,
  caminho ou stack (SEC-017).
- **Desktop-shell Tauri 2** com CSP `style-src 'self'` (sem estilo inline),
  permissões mínimas e assets embutidos.
- **Frontend HTML/CSS/JS** (sem React/TypeScript/Tailwind), com design system
  cobalto/preto/branco, Corte-M, fontes WOFF2 locais e gráficos SVG originais.
- **Testes**: suíte Rust do workspace (contagem exata depende do ambiente) e
  `scripts/verificar-frontend.ps1` como gate de frontend.

## O que falta antes de ser um produto

1. Homologação em hardware real (i3/i5 de 5ª geração, 6 GB, HDD 5400 rpm) e
   piloto de 30 dias em loja — F8/P0-5.
2. Revisão jurídica do expurgo LGPD e assinatura de contrato de operador.
3. Rebuild do shell Tauri para embutir o frontend atualizado (`dist/`).
4. Pareamento autenticado + TLS local para expor o servidor na rede da loja (MVP-2).
5. Backup cifrado na nuvem, licença e cotas — bloqueados por P6/P10/P2/P4/N1/N3/N4.
6. 2FA com provider de email real — adiado até decidir a hospedagem.
7. Comissões, comendas, promoções, XML NF-e e multi-loja (MVP-5/MVP-6).

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