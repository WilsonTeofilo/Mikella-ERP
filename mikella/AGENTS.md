# AGENTS.md — Regras de engenharia do Mikella ERP

Este arquivo é o contrato de trabalho para qualquer pessoa ou agente que altere o
repositório. Ele descreve **onde o código mora**, **como ele deve ser escrito** e
**o que reprova uma entrega**. Em caso de conflito entre este arquivo e um pedido
pontual, siga o pedido, mas atualize este arquivo na mesma alteração.

---

## 1. Prioridades (nesta ordem)

1. Preservar dados, fluxos de venda e contratos existentes.
2. Corrigir o defeito comprovado (com evidência: log, seletor, caminho, permissão).
3. Reduzir acoplamento e duplicação reais.
4. Separar apresentação, comportamento, domínio e infraestrutura.
5. Reduzir superfície de ataque e privilégios.
6. Adicionar verificação automática que impeça a regressão.
7. Otimizar somente o que foi medido.

Não é objetivo: reescrever tudo de uma vez, criar abstrações genéricas, dividir
cada função em um arquivo ou trocar de tecnologia sem necessidade.

---

## 2. Estrutura obrigatória do frontend

```
frontend/src/
├── app/                         # somente HTML de entrada das telas
├── features/
│   ├── auth/login.{js,css}, onboarding.{js,css}
│   ├── navigation/main-menu.{js,css}
│   ├── pos/pos-menu.{js,css}, pos-sale.{js,css}
│   ├── products/products.{js,css}
│   ├── finance/ e financeiro/    # dashboard, contas e DRE
│   ├── estoque/, comissoes/, equipe/, ajustes/
│   └── caixa/ui/cash-spike.{html,css}
├── shared/
│   ├── design/{tokens.css,fontes.css,utilitarios.css,fontes/*.woff2}
│   ├── services/api.js        # HTTP + token + base URL
│   └── ui/
│       ├── window-controls.{js,css}
│       ├── payment-icons.{js,css}
│       ├── feedback.js        # toast
│       ├── formatadores.js    # centavos
│       └── forms.css
└── index.html
```

### Regras não negociáveis

- **HTML não contém lógica.** Nada de `<style>` com mais de ~400 caracteres,
  nada de `<script>` inline, nada de `onclick=`, `onchange=`, `onsubmit=`.
  Use `<script type="module" src="../features/<feature>/<pagina>.js">`.
- **Um arquivo por responsabilidade.** Se você precisa procurar uma função no
  meio de 1.500 linhas, o arquivo está errado. Exemplo canônico:
  `features/<feature>/verificadordesenha.js` importado pelo ponto de entrada.
- **Cada página importa um ponto de entrada pequeno** que inicializa apenas as
  features daquela tela. Não duplique o mesmo inicializador em seis HTMLs.
- **Todo módulo compartilhado tem propósito específico.** Proibido criar
  `utils.js`, `helpers.js`, `common.js`, `misc.js` como depósito genérico.
- **Sem duplicação de asset.** Fontes ficam apenas em
  `shared/design/fontes/`; o `@font-face` usa caminho relativo ao CSS
  (`./fontes/...`). Não crie uma segunda pasta `fontes`.
- **CSS global só tem token, reset e tipografia.** Estilo de pagamento fica em
  `payment-icons.css`; estilo de janela em `window-controls.css`.
- **Acesso à API passa por `shared/services/api.js`.** Não monte `fetch` manual,
  não leia token em cada página, não repita leitura de JSON com `catch(() => ({}))`.

### Fronteiras (Clean Architecture no frontend)

| Camada | Responsabilidade | Não pode |
|---|---|---|
| `app/*.html` | estrutura semântica e referências | conter JS/CSS de comportamento |
| `features/<x>/*.js` | comportamento da tela | falar com banco/sistema de arquivos |
| `shared/services` | portas de infraestrutura (HTTP) | conhecer regra de negócio |
| `shared/ui` | componentes reutilizáveis | depender de uma página específica |

---

## 3. Tauri / Rust

```
crates/desktop-shell/
├── tauri.conf.json          # schema v2, withGlobalTauri: true
├── capabilities/main.json   # permissões mínimas da janela "main"
└── src/
    ├── main.rs              # entrada mínima
    ├── lib.rs               # composição (executar)
    ├── app/{mod.rs,setup.rs}
    └── commands/{mod.rs,sistema.rs}
```

- `main.rs` só chama `mikella_desktop_shell::executar()`. Nada de regra de negócio.
- Comandos Tauri são **adaptadores finos**: validam entrada, delegam, convertem
  erro. Regras pertencem a `domain-*` / `app-*`.
- **Não crie comando Rust para operação que a API oficial já oferece.**
  Minimizar/fechar/tela cheia usam `window.__TAURI__.window.getCurrentWindow()`
  com permissões em `capabilities/main.json`.
- Toda permissão nova precisa de justificativa. `core:window:default` **não**
  inclui `close`, `minimize` nem `set-fullscreen`: declare explicitamente.
- Nunca use `*` como janela de capability.
- `unwrap()`/`expect()` proibidos em caminho operacional; aceitáveis apenas onde
  a invariante é garantida e documentada (ex.: `generate_context!`).
- Erros retornados ao WebView não expõem caminho absoluto, SQL, token ou stack.

---

## 4. Segurança

- SQL sempre parametrizado; nunca interpolar entrada do usuário.
- Frontend não é fronteira confiável: valide no backend.
- Não versionar `.env`, banco, token, certificado ou dado de cliente.
- Não logar CPF, pagamento, credencial ou conteúdo sensível.
- CSP restritiva está habilitada (`style-src 'self'`): não use `<style>`,
  `style=`, `cssText` nem `innerHTML` com dado dinâmico. O gate
  `scripts/verificar-frontend.ps1` reprova esses padrões. Teste no WebView2
  antes de produção.
- Dependência nova exige justificativa e lockfile atualizado.

---

## 5. Checklist antes de considerar pronto

```powershell
# Frontend: sem monolitos, sem handler inline, referências válidas, JS válido
powershell -ExecutionPolicy Bypass -File scripts/verificar-frontend.ps1

# Rust
cargo fmt -p mikella-desktop-shell -- --check
cargo clippy -p mikella-desktop-shell --all-targets -- -D warnings
cargo test -p mikella-desktop-shell
cargo check --workspace --all-targets
```

Critérios de aceite:

- [ ] comportamento esperado descrito;
- [ ] teste automatizado ou roteiro de regressão reproduzível;
- [ ] alteração isolada no módulo responsável;
- [ ] erros tratados e observáveis (toast/log), nunca silenciosos;
- [ ] sem 404, sem warning relevante, sem permissão excessiva;
- [ ] `git diff` revisado;
- [ ] mudança de frontend testada **também** no executável empacotado.

> **Atenção:** `dist/` é gerado e ignorado pelo Git. Os assets são embutidos no
> binário (`frontendDist: "../../frontend/src"`). Alterou HTML/CSS/JS?
> Rode `cargo tauri build` e atualize `dist/`; caso contrário o `.bat` continua
> abrindo a versão antiga.

---

## 6. Proibições

- `git reset --hard`, `git clean -xfd`, `git checkout -- .` como atalho de limpeza.
- `docker stop $(docker ps -q)` ou encerrar containers sem identificar o projeto.
- Apagar alteração local de outro agente sem entender a origem.
- Commitar `.env`, `dados/`, `dist/`, `target/`, banco ou binário.
- Simular minimizar/fechar/tela cheia apenas no DOM.
- Deixar `console.log` de depuração em código entregue.
- Reescrever o app inteiro num único diff.
