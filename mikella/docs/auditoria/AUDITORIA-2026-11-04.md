# Auditoria técnica e limpeza arquitetural — 2026-11-04

**Projeto:** Mikella ERP (Tauri 2 + Rust + HTML/CSS/JS)
**Escopo:** frontend (`mikella/frontend/src`), shell desktop Tauri
(`mikella/crates/desktop-shell`) e verificação de build/distribuição.
**Natureza:** auditoria com correções incrementais. Não é reescrita total.
**Branch:** `develop` · **Commit base:** `a43958b`
**Toolchain verificada:** `cargo 1.99.1`, `node v24.18.0`, `tauri 2.12.5`,
`tauri-build 2.7.4`, `wry 0.57.1`.

---

## 1. Método

1. Inventário de Git, árvore de arquivos, configuração Tauri, script `.bat` e
   versões travadas no `Cargo.lock`.
2. Medição objetiva dos monólitos (tamanho de `<style>`/`<script>` inline e
   quantidade de handlers `on*=`) antes de tocar em qualquer arquivo.
3. Reprodução do defeito dos ícones em navegador, lendo `getComputedStyle` de
   cada forma SVG — não por suposição.
4. Correção por feature, com `node --check` e `cargo check` após cada etapa.
5. Registro do que foi automatizado e do que depende de teste manual no Windows.

---

## 2. Linha de base encontrada

| Item | Estado inicial |
|---|---|
| Branch | `develop` (3 commits à frente de `main`) |
| Alterações locais pré-existentes | `main.rs`, `tauri.conf.json`, `payment-icons.css` |
| Script de inicialização | `dist/iniciar_loja.bat` → `mikella-server.exe` + `mikella.exe` |
| `dist/mikella.exe` | build de 03/11/2026 19:25 — assets antigos embutidos |
| Config Tauri | schema v1 (`tauri.studio/schema/tauri.json`), `product-name`, capability inline |
| CLI Tauri | **não instalada** (`cargo tauri` inexistente) |

> `dist/` é ignorado pelo Git e os assets são **embutidos** no binário
> (`frontendDist: "../../frontend/src"`). Por isso qualquer alteração de
> frontend exige reconstrução do executável; o `.bat` sozinho não recarrega
> `src/`.

---

## 3. Achados e evidências

### 3.1 HTML monolítico (confirmado por medição)

| Página | Linhas antes | CSS inline | JS inline | `on*=` | Linhas depois |
|---|---:|---:|---:|---:|---:|
| `app/landing.html` | 609 | 7.543 car. | 7.120 car. | 1 | **82** |
| `app/main-menu.html` | 670 | 7.670 car. | 8.925 car. | 0 | **69** |
| `app/pos-menu.html` | 1.515 | 11.855 car. | 23.216 car. | 9 | **403** |
| `app/pos.html` | 1.184 | 8.796 car. | 22.105 car. | 3 | **212** |
| `app/produtos.html` | 365 | 4.046 car. | 7.333 car. | 0 | **112** |
| `app/financeiro.html` | 419 | 4.086 car. | 8.985 car. | 0 | **125** |
| `features/caixa/ui/cash-spike.html` | 530 | 14.994 car. | 0 | 0 | **116** |

**Total:** ~4.208 linhas de apresentação removidas do HTML e redistribuídas em
módulos por feature.

### 3.2 Ícones de pagamento pretos (“quadrado preto vazio”)

**Evidência coletada em navegador** (`getComputedStyle` por elemento):

- Com o CSS antigo, os `<path>`/`<line>` sem `fill` declarado computavam
  `fill: rgb(0,0,0)` — herdado do valor inicial de `fill` em SVG. Resultado:
  manchas pretas sobre os `rect` coloridos.
- Os `rect`/`circle` com `fill` próprio (ex. `#E8F5E9`) permaneciam corretos.

**Conclusão:** o problema não era “falta de cor”, e sim formas de traço
(`path`/`line`) sendo preenchidas de preto por não declararem `fill`.

**Correção aplicada:** cada forma declara `fill` e `stroke` explicitamente; a
raiz do SVG usa `fill: none` como rede de segurança para que nenhuma forma
“sem preenchimento” vire mancha preta. A grade deixou de ser 6 blocos de SVG
duplicados no HTML e passou a ser gerada por `shared/ui/payment-icons.js`
(fonte única `FORMAS_PAGAMENTO`).

### 3.3 Minimizar / fechar / tela cheia não funcionavam no `.bat`

Três causas independentes, todas verificadas:

1. **Executável desatualizado.** O `.exe` em `dist/` era anterior às mudanças
   de frontend; o `.bat` abre esse binário, não o `src/`.
2. **Capability ausente na janela correta.** `core:window:default` **não**
   inclui `allow-close`, `allow-minimize`, `allow-is-fullscreen` nem
   `allow-set-fullscreen` (confirmado em
   `gen/schemas/acl-manifests.json`, 158 permissões de `core:window`).
3. **API errada/ambígua.** Havia comandos Rust próprios (`fechar_app`,
   `minimizar_app`, `alternar_fullscreen`) sem uso pelo frontend — superfície de
   IPC morta.

**Correção aplicada:** uso exclusivo da API oficial
`window.__TAURI__.window.getCurrentWindow()`; capability dedicada
`capabilities/main.json` com janela `main`; remoção dos comandos mortos.

### 3.4 Fontes e letreiro

- `shared/design/fontes.css` usa `url("./fontes/...")`, que resolve
  corretamente para `shared/design/fontes/` (o `href` do HTML é relativo ao
  HTML; o `url()` é relativo ao CSS — bases diferentes).
- Existia uma **segunda cópia idêntica** das 7 fontes em `shared/fontes/`,
  sem nenhuma referência. Foi removida (7 arquivos, ~103 KB duplicados).
- Os 7 `.woff2` restantes estão em UTF-8/binário válidos e presentes.

### 3.5 Configuração Tauri em schema v1

- `tauri.conf.json` usava `https://tauri.studio/schema/tauri.json` e
  `product-name` (alias aceito, mas legado), com capability **inline**.
- Migrado para schema v2 (`https://schema.tauri.app/config/2`), `productName`,
  identificador de janela `label: "main"` e capability por arquivo.

### 3.6 Duplicação de lógica no frontend

Cada página reimplementava: `API_URL`, leitura de token, `authHeaders`,
`Money.fmt/parse`, `toast`, `fetchJSON` e os controles de janela. Tudo
consolidado em módulos compartilhados.

---

## 4. Correções aplicadas

### 4.1 Frontend — nova organização

```
frontend/src/
├── app/                       # apenas HTML semântico
├── features/
│   ├── auth/login.{js,css}
│   ├── navigation/main-menu.{js,css}
│   ├── pos/pos-menu.{js,css}
│   ├── pos/pos-sale.{js,css}
│   ├── products/products.{js,css}
│   ├── finance/finance.{js,css}
│   └── caixa/ui/cash-spike.{html,css}
└── shared/
    ├── services/api.js        # base URL, token, fetch JSON/texto
    ├── ui/feedback.js         # toast único
    ├── ui/formatadores.js     # centavos (Intl pt-BR)
    ├── ui/payment-icons.{js,css}
    └── ui/window-controls.{js,css}
```

- HTMLs agora só têm estrutura + `<link>` + `<script type="module" src=...>`.
- `onclick=` eliminados: `data-acao` + mapa `acoesDeInterface` em `pos-menu.js`;
  `data-acao="fechar-modais"` em `pos-sale.js`.
- `landing.html`: nav de controles duplicada removida (usava outra classe e
  duplicava listeners); `onsubmit="return false"` removido (o módulo já faz
  `preventDefault`).
- `pos.html`: grade de 6 SVGs duplicados substituída por
  `<div id="grade-pagamentos">`, renderizada por `montarGradePagamentos`.

### 4.2 Controles de janela (fade em tela cheia)

`shared/ui/window-controls.js` + `.css`:

- Barra injetada uma única vez por documento (idempotente).
- Estado visual sempre derivado de `isFullscreen()` real; nunca de variável
  local isolada.
- Em tela cheia: controles saem com **fade out** (`opacity` + `translateY`) e
  `pointer-events: none`.
- Zona sensora no topo direito (`280×18px`) revela com **fade in**; ao sair da
  zona/barra, oculta após 900 ms (evita tremulação).
- `:focus-within` também revela (acessibilidade por teclado).
- F11 alterna a janela real, com `preventDefault` e trava
  (`transicaoEmAndamento`) contra cliques repetidos.
- Ao entrar em tela cheia, o foco do botão é removido (senão o
  `:focus-within` manteria a barra visível).
- Respeita `prefers-reduced-motion`.
- Falha de API gera toast controlado, não silêncio.
- A barra recebe `data-tauri-drag-region`: como a janela não tem decoração
  nativa (`decorations: false`), ela também funciona como área de arraste do
  aplicativo, sem interferir nos cliques dos botões.

### 4.3 Rust / Tauri

```
crates/desktop-shell/
├── capabilities/main.json      # janela "main", 5 permissões mínimas
├── tauri.conf.json             # schema v2, withGlobalTauri: true
└── src/
    ├── main.rs                 # 7 linhas: chama executar()
    ├── lib.rs                  # composição
    ├── app/{mod.rs,setup.rs}   # logging + builder + registro de comandos
    └── commands/{mod.rs,sistema.rs}  # health, app_info (+ testes)
```

- Comandos `fechar_app`, `minimizar_app`, `alternar_fullscreen` **removidos**
  (dead code; a API oficial cobre os três).
- `app_info` devolve apenas `status` e `versao` (teste garante que não vaza
  caminho/dado sensível).
- `[lib] crate-type` recebeu `"rlib"` para o binário reutilizar a lib.
- Capability compilada confirmada em
  `target/debug/build/.../out/capabilities.json` com `mikella-main`.

---

## 5. Verificações executadas (resultado real)

| Verificação | Comando | Resultado |
|---|---|---|
| Referências de asset + inline | `scripts/verificar-frontend.ps1` | **OK** — 0 `<style>`, 0 `<script>` inline, 0 `on*=`, 0 link quebrado |
| Sintaxe JS (11 módulos) | `node --check` (via script) | **OK** |
| Encoding | round-trip UTF-8 em 31 arquivos | **OK** — nenhum `U+FFFD` |
| Rust format (crate alterado) | `cargo fmt -p mikella-desktop-shell -- --check` | **OK** |
| Rust lint (crate alterado) | `cargo clippy -p mikella-desktop-shell --all-targets -- -D warnings` | **OK** |
| Testes do shell | `cargo test -p mikella-desktop-shell` | **2 passed** |
| Workspace | `cargo check --workspace --all-targets` | **OK (exit 0)** |
| Build release | `cargo clean -p mikella-desktop-shell` + `cargo build --release -p mikella-desktop-shell` | **OK (exit 0)** — executável copiado para `dist/mikella.exe` |

### Observação sobre `cargo fmt --all`

O workspace **já tinha** divergência de formatação em 33 arquivos de crates
não tocados por esta auditoria (`app-cash`, `app-finance`, `store-server`,
`infra-*`, etc.). Para não misturar um diff mecânico gigante com correções de
comportamento, o `fmt` foi aplicado/verificado apenas no crate alterado.
Recomendação: rodar `cargo fmt --all` em um commit exclusivo de formatação.

### Observação sobre Docker

Não há `Dockerfile`, `docker-compose` nem `.dockerignore` no repositório. Não
foi executado nenhum comando Docker. O produto é desktop-first e empacota os
assets no binário; Docker não participa do runtime da loja.

---

## 6. Pendências (dependem de teste manual no Windows)

1. **`dist/mikella.exe` já foi atualizado** com o build release desta auditoria
   (`cargo clean -p mikella-desktop-shell` seguido de
   `cargo build --release -p mikella-desktop-shell`, executável copiado para
   `dist/`). Falta apenas a validação funcional no Windows.
   > Os assets são comprimidos no binário, então não é possível conferir o
   > conteúdo por busca de texto; a garantia vem do rebuild limpo do crate, que
   > reexpande `generate_context!` lendo `frontend/src` no momento da compilação.
2. **Validar visualmente os ícones** em `cargo run`/`cargo tauri dev` e no
   `.exe`: seis formas com pastel + contorno preto, sem mancha preta.
3. **Validar os controles** no executável: minimizar, fechar, F11 e fade
   in/out da extremidade superior direita.
4. **Confirmar `withGlobalTauri`** em runtime: `window.__TAURI__` deve existir
   no WebView2. Se o toast “Não foi possível…” aparecer, o build não contém a
   capability (verificar `gen/schemas/capabilities.json`).
5. **CSP restritiva** não foi habilitada: não há como validar o WebView2 neste
   ambiente. Exige teste dedicado antes de produção.
6. **Migração de schema de fonte/estilos globais**: `tokens.css` e `forms.css`
   foram mantidos como estavam para evitar churn.

---

## 7. Roteiro de regressão manual

```text
1. cargo run -p mikella-desktop-shell   (ou cargo tauri dev, se CLI instalada)
2. Login: usuário + senha + 2FA → menu principal
3. Menu → PDV → venda com 2 itens → F1 pagamento
   - conferir 6 ícones coloridos
   - dinheiro com troco, débito, pix, voucher e crediário
4. Finalizar venda → comprovante → voltar
5. Produtos: cadastrar, editar, importar CSV
6. Financeiro: nova conta, liquidar, backup
7. Controles: minimizar, restaurar, fechar
8. F11: entrar em tela cheia (controles somem em fade)
   - mover mouse ao topo direito → fade in
   - sair da zona → fade out
   - F11 novamente → volta ao modo janela
9. Fechar e reabrir: dados persistem
10. Repetir 7–8 no .exe iniciado por dist/iniciar_loja.bat
```

---

## 8. Arquivos-chave alterados

| Arquivo | Papel |
|---|---|
| `shared/services/api.js` | cliente HTTP/token único |
| `shared/ui/formatadores.js` | centavos (Intl) |
| `shared/ui/feedback.js` | toast único |
| `shared/ui/payment-icons.{js,css}` | fonte única dos 6 ícones + grade |
| `shared/ui/window-controls.{js,css}` | fade/fullscreen/F11 |
| `features/**/*.{js,css}` | comportamento e estilo por tela |
| `app/*.html` | apenas estrutura |
| `crates/desktop-shell/src/{main,lib,app/*,commands/*}` | shell em camadas |
| `crates/desktop-shell/capabilities/main.json` | permissões mínimas |
| `crates/desktop-shell/tauri.conf.json` | schema v2 |
| `scripts/verificar-frontend.ps1` | guarda anti-monólito |
| `AGENTS.md` | regras de engenharia |
