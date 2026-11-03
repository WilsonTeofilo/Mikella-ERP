# Mikella ERP — arquitetura, clean code e design patterns

**Status:** diretrizes de projeto. Nenhuma estrutura abaixo foi criada. As decisões estruturais relevantes devem virar ADR.

## 1. O que "anti-monolito" significa aqui (e o que não significa)

O produto roda em computadores com 4 GB de RAM. **Microserviços locais seriam prejudiciais**: cada processo extra consome memória, aumenta complexidade de instalação e cria novas superfícies de ataque.

Adotamos, então, o desenho correto para este contexto:

1. **Quatro unidades implantáveis separadas**, cada uma com contrato explícito: Landing/Portal, Mikella Cloud, Servidor da Loja e Cliente (Caixa).
2. **Dentro do Servidor da Loja, um monólito modular**: contextos delimitados (vendas, caixa, estoque, financeiro, pessoas, licença, backup, comissões) isolados em crates próprias, que se comunicam por **portas (traits) e eventos de domínio**, nunca por acesso direto a tabelas de outro contexto.
3. **Nenhuma "classe deus"**, nenhum arquivo utilitário genérico que todos importam, nenhum estado global mutável.

Regra de ouro: dá para trocar a implementação de um contexto, ou testá-lo sozinho, sem editar os outros.

## 2. Clean Architecture

Dependências apontam **sempre para dentro**.

```
Interface / Frameworks      Tauri commands, axum handlers, React, CLI
        │
Adapters (infrastructure)   SQLite, cloud HTTP client, crypto, ESC/POS, filesystem, OS keystore
        │
Application (use cases)     Casos de uso, portas (traits), DTOs, transações
        │
Domain                      Entidades, value objects, invariantes, eventos, regras puras
```

| Camada | Pode importar | Não pode importar |
|---|---|---|
| `domain` | somente a biblioteca padrão e crates utilitárias puras (ex.: `uuid`, `thiserror`) | Tauri, axum, rusqlite, serde-json de rede, relógio do sistema direto |
| `application` | `domain` | SQLite, HTTP, Tauri |
| `infrastructure` | `application`, `domain` | `interface` |
| `interface` | `application`, `infrastructure` (na composição) | regras de negócio |

- **Relógio, gerador de IDs e aleatoriedade são portas** (`Clock`, `IdGenerator`), para teste determinístico e para o relógio confiável da licença.
- **A composição (injeção de dependências) acontece em um único lugar** (`composition root`), por construtor. Sem container mágico, sem singleton global.

## 3. Estrutura de pastas proposta

Cargo workspace (Rust) + frontend separado:

```
mikella/
  crates/
    shared-kernel/        # Money (centavos), Cpf/Cnpj, ids tipados, erros base
    domain-sales/         # Venda, Item, Desconto, regras de total
    domain-cash/          # SessaoCaixa, Movimentacao, saldo derivado
    domain-inventory/     # Produto, Lote, MovimentacaoEstoque, validade
    domain-finance/       # ContaPagar, ContaReceber, Resultado
    domain-people/        # Usuario, Papel, Capacidade, hierarquia
    domain-commission/    # Regra, Comanda, Atribuicao
    domain-licensing/     # Entitlement, Cota, EstadoAssinatura (regras puras)
    app-*/                # casos de uso por contexto + portas
    infra-sqlite/         # repositórios, migrações, unit of work
    infra-crypto/         # AEAD, KDF, assinatura, chaveiro do SO
    infra-cloud/          # cliente HTTP da nuvem, outbox
    infra-hardware/       # ESC/POS, gaveta, leitor (adapters)
    store-server/         # axum: API local, auth, TLS, pareamento
    desktop-shell/        # Tauri: comandos finos, janelas, updater
  frontend/
    src/
      app/                # bootstrap, rotas, providers
      features/           # caixa/, estoque/, financeiro/, equipe/, comissoes/ ...
        caixa/
          api/            # chamadas e tipos (contratos)
          model/          # estado e hooks da feature
          ui/             # componentes da feature
      shared/             # ui-kit mínimo, utilitários puros, tokens de design
  docs/
    decisions/            # ADRs
  tools/                  # scripts de build, medição, geração de tipos
```

Regras de pasta:
- Organizar **por feature/contexto**, não por tipo técnico (`components/`, `utils/` gigantes são proibidos).
- Uma feature do frontend **não importa de outra feature**; compartilhamento vai para `shared/` e só se for realmente genérico.
- Tipos do contrato (DTOs) são **gerados** a partir do Rust (ex.: `ts-rs` ou `specta`), para impedir divergência manual entre UI e núcleo. *(Avaliar peso e manutenção antes de adotar.)*

## 4. Design patterns: onde usar (e onde não usar)

Padrões entram **quando resolvem uma variação ou acoplamento real**. Não aplicar padrão por enfeite.

| Padrão | Uso no Mikella |
|---|---|
| **Repository** (porta + adapter) | Acesso a dados por agregado; `domain/app` definem a trait, `infra-sqlite` implementa |
| **Unit of Work** | Uma transação por caso de uso (ex.: venda grava venda, itens, estoque, caixa e comissão ou nada) |
| **Command / Use Case** | Cada ação é um caso de uso explícito (`FinalizarVenda`, `RegistrarSangria`) com entrada e saída tipadas |
| **State Machine** | Venda (`Rascunho → Paga → Cancelada`), Sessão de caixa (`Aberta → EmFechamento → Fechada`), Recebimento NF-e, Assinatura. Transições permitidas são explícitas e testadas |
| **Strategy** | Variações reais: forma de pagamento, regra de comissão, política de arredondamento, política de estoque negativo, política de alerta de validade |
| **Specification** | Regras compostas de elegibilidade (ex.: item é comissionável por produto OU categoria OU vendedor) |
| **Adapter** | Impressora térmica, gaveta, leitor, chaveiro do SO, relógio, cliente da nuvem, importador de NF-e |
| **Decorator / Middleware** | Cross-cutting em casos de uso: auditoria, verificação de capacidade, verificação de cota. Evita repetir em cada caso |
| **Domain Events + Outbox** | Eventos (`VendaFinalizada`, `ProdutoCriado`) alimentam comissão, fila de backup e auditoria sem acoplar contextos; outbox garante entrega "pelo menos uma vez" com idempotência |
| **Factory / Builder** | Apenas para construção complexa de agregados com invariantes (ex.: `Venda::nova`). Sem factory para tudo |
| **Value Object** | `Money`, `Cpf`, `Cnpj`, `Quantidade`, `Sku`, `DataValidade`: validação na construção, imutáveis |

**Evitar:** Singleton global, Service Locator, herança profunda, interfaces com um único implementador sem motivo de teste/fronteira, "Manager/Helper/Utils", ORM pesado que esconda as queries.

## 5. Regras de domínio centrais (para guiar o código)

- **Money:** `i64` em centavos. Soma e comparação com checagem de overflow. Divisão/percentual com regra de arredondamento única e testada.
- **Estoque e caixa:** `movimentações` imutáveis; saldo por consulta ou projeção atualizada na mesma transação. Triggers do banco recusam `UPDATE/DELETE` em tabelas de movimento.
- **Venda:** o agregado valida invariantes (itens ≥ 1, quantidades positivas, soma dos pagamentos = total). O **preço vem do catálogo no servidor**, nunca do cliente.
- **Idempotência:** chave de operação única por venda/estorno/sangria; repetição devolve o resultado anterior.
- **Concorrência:** baixa de estoque feita no banco (`UPDATE ... SET qtd = qtd - ?` condicional), sem ler-calcular-gravar. Edição de cadastro com **versão (lock otimista)**.
- **Erros:** erro de domínio tipado (`thiserror`), convertido na borda em respostas estáveis sem detalhes internos. Sem `unwrap()`/`expect()` em código de produção fora de inicialização; `panic` é bug.

## 6. Clean Code (critérios de revisão)

- **Nomes revelam intenção** e usam vocabulário do domínio (`FecharSessaoDeCaixa`, não `processData`).
- **Uma responsabilidade por unidade.** Funções coesas; sem limite artificial de linhas, mas se precisa de comentário para explicar um bloco, extraia e nomeie.
- **Fluxo explícito:** cláusulas de guarda, retornos tipados, sem `Option` aninhado confuso, sem booleanos posicionais (use enums/structs).
- **Sem hardcode:** limites de plano, URLs, prazos, textos de política, chaves. Tudo vem de configuração tipada, de entitlements assinados ou do banco.
- **Sem duplicação de regra:** cada regra de negócio tem **um** dono. Validação de CPF existe uma vez (`shared-kernel`).
- **Comentários explicam "por quê"**, invariantes e limitações. Não narram o código nem substituem nomes.
- **Código morto e TODO sem dono não entram.**
- **Testes de comportamento:** domínio com testes unitários rápidos; casos de uso com repositórios em memória; adapters com SQLite real em arquivo temporário; contratos de API com testes de contrato.
- **Formatação e lint automáticos** (`rustfmt`, `clippy` sem avisos, ESLint/TypeScript estrito) no CI. Alertas só são suprimidos com justificativa curta.
- **Dependências:** poucas, fixadas, auditadas (`cargo audit`/`cargo deny`, `npm audit`), com impacto em tamanho e RAM avaliado.

## 7. Frontend (React + TypeScript + Tailwind)

- TypeScript `strict`. Sem `any` sem justificativa.
- **Estado do servidor** (dados da loja) separado de **estado de UI**. Preferir uma biblioteca pequena de cache de requisições ou `fetch` + hooks próprios; evitar bibliotecas de estado globais pesadas se o caso não exigir.
- **Componentes de apresentação puros** na camada `ui/`; lógica em hooks da feature (`model/`).
- **Tokens de design em variáveis CSS/Tailwind `@theme`**, para que a identidade visual (ainda indefinida) seja trocável sem reescrever componentes.
- Acessibilidade básica: foco visível, navegação por teclado completa no caixa (um PDV é usado no teclado), rótulos, contraste.
- O caixa é **operável sem mouse**: atalhos documentados.
- **A UI nunca recalcula total oficial.** Pode exibir uma prévia otimista, mas o valor exibido como final vem da resposta do servidor.

## 8. Contratos e versionamento

- API local versionada (`/v1/...`). O cliente informa sua versão; o servidor recusa versões incompatíveis com mensagem clara.
- **Atualização:** servidor atualiza primeiro; clientes incompatíveis são orientados a atualizar antes de operar.
- **Migrações de banco** automáticas e **precedidas de cópia do banco**, com verificação de integridade antes e depois. Estratégia expandir/contrair, nunca destrutiva sem cópia verificada.
- Contratos com a nuvem são versionados e tolerantes a campos novos.

## 9. Observabilidade local

- Logs estruturados com rotação e limite de tamanho, **sem PII, senha, token, CPF ou chave**.
- Trilha de **auditoria** separada do log técnico (quem fez o quê, quando, resultado), append-only (ver `04` e `05`).
- Painel de saúde simples para o dono: último backup, espaço em disco, estado da licença, versão.

## 10. Checklist de revisão por mudança

1. A regra está no domínio, e não na UI, no handler ou no SQL espalhado?
2. A camada respeita a direção de dependências?
3. Entrada validada e DTO fechado? Capacidade e cota verificadas?
4. Operação financeira/estoque é movimentação, idempotente e transacional?
5. Há valor hardcoded que deveria ser configuração/entitlement?
6. Teste cobre o comportamento (inclusive limite e falha)?
7. Impacto em RAM/tempo medido, quando relevante (`07`)?
8. Log e erro sem dado sensível?
9. Migração é segura e reversível por cópia?
10. É o código mais simples que atende ao requisito?
