# Mikella ERP — Plano Mestre de Desenvolvimento

**Última atualização:** 02/11/2026  
**Status atual:** MVP-0, MVP-1, Pré-Piloto e Desktop Shell (Tauri 2) CONCLUÍDOS ✅ (172 testes passando, clippy 0 warnings, binários `mikella.exe` e `mikella-server.exe` gerados e validados)  
**Premissa fundamental:** Desktop local-first (Tauri 2 + Rust + HTML/CSS puro + SQLite). O sistema roda e processa 100% na máquina da loja; a nuvem não processa vendas e só servirá (após o MVP-3) para sincronização de conta, licença e cópia de segurança (backup cifrado zero-knowledge). A landing page fica para o final do projeto para utilizar capturas de tela e métricas reais do sistema operando.

Documento de precedência: `04/05 > 06 > 09 > 03 > 02 > 07 > 08 > 01`.

---

## 1. Visão Geral das Camadas e Gates

```
[MVP-0] ✅ Núcleo Puro: Domínio, SQLite, livro de caixa imutável, venda atômica, estoque por movimentação.
[MVP-1] ✅ PDV Vendável (1 PC): API local (store-server), auth Argon2id + 2FA BLAKE3, caixa F1-F7, produtos com FTS5 e CSV, financeiro básico RF-030 (contas a pagar/receber e liquidação), gerenciamento de usuários com hierarquia e catálogo fechado, frontend TUI balcão (cobalto/preto/branco).
[PRÉ-PILOTO] ✅ Recursos de Loja Concluídos:
  ├── 1. Impressão de cupom e fechamento ESC/POS 58mm/80mm (infra-hardware).
  ├── 2. Cópia de segurança local de 1 clique (POST /v1/sistema/backup-local com VACUUM INTO e BLAKE3).
  ├── 3. Pacote de diagnóstico técnico sem PII (GET /v1/sistema/diagnostico com integridade de banco).
  └── 4. Telas operacionais no terminal (produtos.html, financeiro.html e main-menu.html integrado F1-F5).

─── [FASE ATUAL: ENTRADA NO PILOTO FECHADO DE 30 DIAS EM 3 LOJAS REAIS] ───
  ├── 1. Build de distribuição de release (binários otimizados com LTO e strip).
  ├── 2. Script de inicialização da loja em 1 clique (criação automática de dados/loja.db).
  ├── 3. Homologação com hardware físico (impressora térmica USB e leitor de código de barras).
  └── 4. Checklist de acompanhamento diário (tolerância a queda de energia, conferência de caixa sem divergência).

[PILOTO] 🔒 Validação em 3 lojas reais por 30 dias (hardware real i3/i5, 6 GB RAM, HDD, sem dependência de nuvem).
[MVP-2]  🔒 Rede da loja: modo multi-PC (servidor axum local, TLS autoassinado via rcgen, pareamento por código, 3 a 8 caixas simultâneos).
[MVP-3]  🔒 Nuvem: conta lojista, 2FA por e-mail, licença automática, backup cifrado zero-knowledge (Workers + D1 + R2), landing page em Astro.
[MVP-4]  🔒 Licenciamento & Cotas: entitlements assinados por Ed25519, janela de histórico de 6 meses (Gratuito), controle anti-fraude.
[MVP-5]  🔒 Crescimento: regras de comissão, comandas de vendedores, devoluções e promoções.
[MVP-6]  🔒 Expansão: importação de XML de NF-e (modelo 55), conferência de entrada e multi-loja.
```

---

## 2. Piso de Hardware & Requisitos de Execução

| Item | Especificação | Justificativa / Impacto |
|---|---|---|
| **Processador** | Intel Core i3 / i5 de 5ª geração (ex: i3-5005U 2.0 GHz) ou equivalente | Garantir que o sistema opere com fluidez mesmo em CPUs de baixo consumo sem Turbo Boost |
| **Memória RAM** | 6 GB DDR3-1333 | Margem realista para Windows 10/11 + antivírus + WhatsApp Web + Mikella (≤ 450 MB) |
| **Armazenamento** | HDD 5400 rpm (pior caso) ou SSD SATA | SQLite com `synchronous=FULL` e transações curtas para tolerar discos lentos |
| **Sistemas Operacionais** | Windows 10 / 11 e Linux (Ubuntu 22.04+, Mint, Lubuntu, Zorin, Lite) | **Windows 7 é terminantemente PROIBIDO** (fim de suporte, WebView2 obsoleto, insegurança) |
| **Identidade Visual** | Cobalto `#0040D8`, Preto `#0a0a0a`, Branco `#ffffff` | Conceito "Letreiro e Livro-caixa", zero border-radius, Corte-M (45°), anti-IA slop |

---

## 3. Estado Atual Detalhado: O que está Pronto e Testado

### 3.1 MVP-0 — Esqueleto e Domínio Puro (CONCLUÍDO ✅)
- **`Money`:** Tipagem estrita em centavos (`i64`), sem float, aritmética com proteção contra overflow, formatação brasileira (`R$ 129,90`).
- **`domain-cash`:** `SessaoCaixa`, `LivroCaixa` imutável com cadeia de hashes BLAKE3 (GENESIS → ABERTURA → VENDA → FECHAMENTO) e saldo calculado como soma derivada.
- **`domain-sales`:** `Venda`, `ItemVenda`, cálculo de subtotais e descontos estritamente no domínio Rust.
- **`domain-inventory`:** `Produto`, `Lote` (FEFO), `MovimentacaoEstoque` (entradas positivas, saídas negativas, tabela append-only).
- **`infra-sqlite`:** Migrações versionadas com checksum BLAKE3, integridade referencial (`PRAGMA foreign_keys = ON`), WAL mode, triggers SQL bloqueando `UPDATE` e `DELETE` em tabelas imutáveis (`livro_caixa`, `movimentacoes_estoque`, `venda_itens`). Padrão `UnitOfWork` implementado para transações atômicas com rollback garantido.

### 3.2 MVP-1 — Sistema Vendável / Loja Local (CONCLUÍDO ✅)
- **`store-server` (HTTP local em 127.0.0.1):**
  - **Autenticação:** Senhas hasheadas com Argon2id e salt gerado via `OsRng` (CSPRNG).
  - **Segundo Fator (2FA):** Código numérico de 6 dígitos com hash BLAKE3 keyado em memória, validade de 5 minutos, limite de 5 tentativas e uso único.
  - **Sessões e Autorização:** Tokens Bearer seguros, sessão única por administrador, autorização baseada em catálogo fechado (`CAPACIDADES_CONHECIDAS`) e hierarquia rígida (`DONO(3) > GERENTE(2) > OPERADOR/ESTOQUISTA/VENDEDOR(1)`).
  - **Vendas e PDV:** Endpoint `POST /v1/vendas` atômico com `UnitOfWork` (grava venda, baixa estoque e adiciona entrada no livro caixa na mesma transação), suporte a pagamentos múltiplos e idempotência por `venda_id`.
  - **Catálogo de Produtos:** CRUD completo, busca textual ultra-rápida via SQLite FTS5 e importação de planilhas CSV com sanitização e streaming.
  - **Financeiro Básico (RF-030):** Contas a pagar e a receber com parcelas, validações estritas de datas/valores e endpoint de liquidação atômica.
  - **Proteção da API:** Todos os DTOs com `#[serde(deny_unknown_fields)]` (SEG-060), rate limiting por usuário/IP e tratamento de erros sem vazamento de stacktrace ou PII (código de incidência UUID conforme SEG-081).
- **Frontend TUI do Caixa (`frontend/src/app/`):**
  - Páginas HTML5/CSS3 puras: `landing.html` (login + 2FA), `main-menu.html` (navegação por perfil), `pos.html` (caixa balcão).
  - Operação 100% via teclado: atalhos F1–F7, busca rápida de produtos, CPF opcional, modal de pagamento completo (Dinheiro, Débito, Crédito, Pix, Outro) com cálculo automático de troco/restante e tecla `ESC` contextual.
  - Dependência de rede zero: fontes Archivo e Source Sans 3 embutidas em WOFF2 local (< 120 KB).

### 3.3 Recursos de Prontidão Operacional de Loja (CONCLUÍDO ✅)
- **Impressão Térmica ESC/POS (`crates/infra-hardware`):**
  - Modelos `ComprovanteVenda` e `ComprovanteFechamentoCaixa`.
  * Renderizadores de texto puro e binário ESC/POS para bobinas de **58mm** (32 colunas) e **80mm** (48 colunas).
  * Porta de impressão (`PrinterPort`) com adaptadores `MockPrinter`, `ArquivoPrinter` e `DispositivoLocalPrinter` (spooler RAW no Windows e `/dev/usb/lp*` no Linux).
  * Endpoints HTTP integrados: `GET /v1/vendas/:id/comprovante`, `POST /v1/vendas/:id/imprimir`, `GET /v1/caixa/fechamento/:id/comprovante`, `POST /v1/caixa/fechamento/:id/imprimir`.
  * Experiência de balcão: ao finalizar venda, exibe modal de troco com botão **Enter** para emitir comprovante térmico e **Esc** para nova venda. Ao fechar caixa (F6), solicita contagem física, fecha sessão e emite cupom de fechamento.
- **Cópia de Segurança Local em 1 Clique (`RES-030`):**
  - Endpoint `POST /v1/sistema/backup-local`: aciona `VACUUM INTO ?` no SQLite (cópia consistente, atômica e desfragmentada sem parar leituras/escritas).
  - Geração e conferência de checksum **BLAKE3** para garantia de integridade física.
- **Diagnóstico Técnico de Saúde (`OPS-001`):**
  - Endpoint `GET /v1/sistema/diagnostico`: checa integridade SQLite (`PRAGMA integrity_check`), integridade nó-a-nó da cadeia de hashes do livro caixa, volumetria de tabelas e dados de ambiente, estritamente **zero PII**.
- **Telas Administrativas no Terminal Desktop:**
  - `produtos.html`: Catálogo com busca em tempo real, cadastro (F1) e importador CSV (F2).
  - `financeiro.html`: Contas abertas, resumo total, liquidação de parcelas e acionamento de backup local.
  - `main-menu.html`: Integrado com atalhos de teclado F1–F5.

### 3.4 Desktop Shell Tauri 2 (CONCLUÍDO ✅)
- **`crates/desktop-shell`:**
  - Compilação e linkagem nativa concluídas gerando `mikella.exe` no Windows via toolchain MSVC + WebView2.
  - Assets estáticos e páginas TUI servidos localmente com zero dependências externas ou de CDN.
  - Comandos IPC nativos limpos e enxutos (`health` e `app_info`), mantendo toda a lógica de negócio na API do `store-server`.
  - Zero warnings no Clippy e tempo de boot ultra-baixo para máquinas i3/i5 com 6 GB de RAM.

### 3.5 Pacote de Distribuição e Inicialização em 1 Clique (CONCLUÍDO ✅)
- **Pacote Autônomo em `mikella/dist`:**
  - `mikella-server.exe` (3.7 MB): Compilado com perfil `release`, LTO e strip ativados.
  - `mikella.exe` (3.5 MB): Compilado com perfil `release` e WebView2 embutido.
  - Peso total do pacote: **menos de 7.5 MB** (boot instantâneo em HDDs lentos).
  - `iniciar_loja.bat` e `iniciar_loja.sh`: Script de inicialização em 1 clique que sobe o servidor local em segundo plano, abre o balcão e encerra os processos ao sair.
  - Configuração do `tauri.conf.json` fixada com `build.frontendDist` apontando para os arquivos HTML5/CSS3.
  - Porta de comunicação unificada em `47823` em todas as telas e endpoints.

---

## 4. O que Falta Fazer Agora (Testes Físicos de Campo para Iniciar o Piloto)

O sistema de software está 100% desenvolvido, testado e empacotado. As únicas etapas restantes dependem do teste com o hardware físico conectado na máquina da loja:

### Etapa 1: Homologação Física de Balcão (Com o Usuário / Na Loja)
- **Impressora Térmica:** Conectar uma impressora térmica USB (bobina 58mm ou 80mm) e disparar a impressão de um cupom de venda e de um fechamento de caixa para validar o espaçamento, corte de papel e avanço de linhas do gerador ESC/POS.
- **Leitor de Código de Barras:** Conectar o leitor USB (modo teclado/HID) e bipar um código de barras de produto no campo de busca do PDV para garantir que o leitor aciona o Enter/busca instantânea sem travar a interface.
- **Gaveta de Dinheiro (Opcional):** Se a loja tiver gaveta automática, testar o pulso elétrico no pino RJ-12 da impressora ao finalizar venda em dinheiro.

### Etapa 2: Teste de Desligamento Forçado (Queda de Energia)
- Com o caixa aberto e uma venda em andamento, desconectar o computador da tomada (simulação de blecaute).
- Ao ligar novamente o computador e dar 2 cliques em `iniciar_loja.bat`:
  - O SQLite deve se recuperar automaticamente via WAL sem corrupção.
  - A sessão de caixa deve continuar aberta com o saldo anterior preservado.
  - O endpoint de diagnóstico (`GET /v1/sistema/diagnostico`) deve retornar integridade `ok`.

### Etapa 3: Início da Contagem dos 30 Dias de Piloto Fechado
- Operação diária nas 3 lojas piloto.
- Acompanhamento semanal de fechamentos de caixa (zero divergência centavo a centavo).
- Ao final dos 30 dias: aprovação do Gate do Piloto e avanço para o MVP-2 (modo multi-PC em rede local) e MVP-3 (nuvem e landing page).

---

## 5. O que DEVE SER RETIRADO ou POSTERGADO (Eliminação de Desperdício)

Para garantir foco e velocidade máxima:

1. ❌ **POSTERGAR: Nuvem (Cloudflare Workers, D1, R2, Mercado Pago):** Só será construída no MVP-3, após os 30 dias de validação com lojistas reais.
2. ❌ **POSTERGAR: Landing Page:** Ficará para o final, para usar capturas de tela e dados reais das lojas piloto.
3. ❌ **RETIRAR: Sincronização em Contingência Multi-Servidor (P12):** Desnecessária no modo de 1 computador.
4. ❌ **RETIRAR: Suporte a Windows 7 (HW-092):** Proibido categoricamente por segurança e falta de WebView2 moderno.
5. ❌ **RETIRAR: Bibliotecas externas e frameworks pesados no frontend:** Mantido em HTML5/CSS3 puro com tokens locais (consumo de RAM < 150 MB).

---

## 6. Regras Invioláveis de Arquitetura e Engenharia

1. **Centavos Inteiros:** Dinheiro é `Money(i64)`. Nunca `float` ou `f64`.
2. **Dados Imutáveis:** Movimentações de estoque e entradas do livro caixa são estritamente `INSERT ONLY`. Triggers abortam `UPDATE` ou `DELETE`.
3. **Autoridade no Domínio:** A interface (frontend) nunca decide preço, saldo, total ou permissão. O domínio Rust valida tudo.
4. **Sem Hardcoded Limits:** Nenhuma cota ou URL embutida diretamente no código.
5. **Logs Limpos:** Zero senhas, PINs, CPFs ou tokens nos logs de aplicação (`tracing`).
6. **Clean Architecture Estrita:** O domínio (`domain-*`) não depende de banco, HTTP, Tauri ou bibliotecas de infraestrutura.
7. **Resiliência a Quedas:** SQLite configurado com WAL mode e `synchronous=FULL` em operações de caixa.
