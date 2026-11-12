# Mikella ERP — otimização para PC fraco

> **Nota de atualização (09/11/2026):** o frontend entregue é HTML/CSS/JS puro,
> sem React ou Tailwind. O piso vigente é i3/i5 de 5ª geração, 6 GB e HDD
> 5400 rpm; as metas de desempenho ainda precisam ser medidas nesse hardware
> (`scripts/benchmark-hardware.ps1`, P0-5).

**Status:** metas de projeto. **Os números abaixo são metas e hipóteses, não medições.** Nada foi medido ainda. O MVP-0 (spike) existe para validar ou refutar este documento.

## 1. Hardware e sistemas-alvo

- **Referência de pior caso:** Intel Core i3 ou i5 de 5ª geração (Broadwell), 6 GB de RAM (DDR3-1333), HDD 5400 rpm (ou SSD barato) no pior caso, sem GPU dedicada.
- **Sistemas:** Lubuntu, Linux Mint, Linux Lite e derivadas (Ubuntu 22.04+ por exigência de WebKitGTK 4.1 do Tauri 2 — **confirmar na documentação oficial do Tauri antes de prometer compatibilidade**), Windows 10+.
- **Fora do alvo:** Windows 7 e distros antigas sem WebKitGTK 4.1.
- **Laboratório obrigatório:** uma máquina real (ou imagem equivalente) com esse perfil para toda medição. Medir só em notebook moderno não vale.

## 2. Orçamentos (metas iniciais a calibrar no spike)

| ID | Métrica | Meta inicial |
|---|---|---|
| PERF-001 | RAM do **Caixa (cliente)** em uso normal | ≤ ~200 MB (alvo ambicioso: ≤ 150 MB) |
| PERF-002 | RAM do **Servidor da Loja** (processo Rust + SQLite) com 8 caixas ativos | ≤ ~150 MB, mais interface se também opera caixa |
| PERF-003 | Abertura a frio até caixa utilizável | ≤ 4 s (alvo: ≤ 3 s) |
| PERF-004 | Busca de produto (dezenas de milhares de itens) | ≤ 50 ms percebidos; resultado a cada tecla sem travar |
| PERF-005 | Bipar item até item na lista | ≤ 100 ms |
| PERF-006 | Finalizar venda (gravação confirmada) | ≤ 300 ms em disco comum |
| PERF-007 | Tamanho do instalador | Tauri: ordem de poucos MB a dezenas de MB; sem runtime pesado empacotado |
| PERF-008 | Uso de CPU ocioso | ~0% (sem polling frenético, sem animação contínua) |
| PERF-009 | Quadros por segundo em transições | Sem queda perceptível; se cair, desativar motion |

**Regra:** qualquer meta não atingida no hardware de referência vira decisão explícita (otimizar, reduzir escopo, ou acionar o plano B Qt/QML).

## 3. Spike do MVP-0 (critérios de aceite)

Construir uma tela de caixa **mínima porém real**: lista de itens virtualizada, busca com 50 mil produtos, venda sendo gravada, uma transição de motion e uma tabela de relatório. Rodar em Lubuntu/i5 5ª gen/6 GB e no Windows 10 equivalente.

Aprovar somente se:
1. PERF-001 a PERF-006 forem atingidos (ou ficarem dentro de tolerância definida).
2. Rolagem de 5 mil linhas e digitação de busca não travarem.
3. A interface permanecer utilizável com o navegador do sistema sem aceleração por GPU.
4. O consumo se mantiver estável por 8 horas de uso simulado (sem vazamento de memória).

Se reprovar: avaliar **Qt 5.15/QML** (ou outra camada nativa), preservando domínio, casos de uso e API local em Rust.

## 4. Núcleo Rust e SQLite

- PERF-020: **perfil de release**: `opt-level` "z" ou "s" vs 3 (medir), `lto`, `codegen-units = 1`, `strip`. Avaliar `panic = "abort"` apenas no cliente; no servidor, preferir `unwind` para isolar falhas por requisição.
- PERF-021: **runtime assíncrono enxuto:** poucos threads de trabalho (configuráveis conforme número de núcleos), sem pool de threads superdimensionado. Operações SQLite em thread dedicada ou pool pequeno (escritor único + poucos leitores).
- PERF-022: **PRAGMAs a avaliar e medir**: `journal_mode=WAL`, `synchronous=FULL` (dinheiro/estoque), `cache_size` moderado, `temp_store` conforme RAM, `mmap_size` conservador (memória-mapeada pode inflar RSS), `busy_timeout`. Documentar cada valor com a medição que o justificou.
- PERF-023: **índices** desenhados a partir das consultas reais (código de barras, nome, SKU, data de venda, sessão de caixa). Conferir `EXPLAIN QUERY PLAN` das consultas críticas.
- PERF-024: **busca de produto** com **FTS5** (ou índice prefixado) em vez de `LIKE '%x%'`. Resultado limitado (ex.: 50) e paginação.
- PERF-025: **sem carregar tabelas inteiras na memória.** Listas com **paginação por keyset** (cursor), nunca `OFFSET` profundo; relatórios agregados no SQL; exportações em **streaming**.
- PERF-026: **projeções** (saldo de estoque, saldo de caixa, totais do dia) atualizadas na mesma transação para consultas O(1), reconciliáveis com o livro.
- PERF-027: manutenção em horário ocioso: checkpoint de WAL, `PRAGMA optimize`, `VACUUM` raro e com cópia prévia.
- PERF-028: **arquivamento mensal** (`05`/`06`): dados antigos saem do banco ativo e reduzem tamanho de índices e de backup.
- PERF-029: serialização enxuta na API local (JSON compacto; considerar formato binário só se medição justificar), compressão **somente** onde compensa (backup), não em cada resposta curta.
- PERF-030: **backup e hashing em baixa prioridade** (nice/ionice, janelas de ociosidade), nunca competindo com a venda.

## 5. Frontend (HTML/CSS/JS) leve

- PERF-040: **bundle pequeno:** sem bibliotecas de componentes pesadas, sem moment/lodash completos, sem ícones por pacote inteiro. Orçamento de JS inicial definido (meta: ordem de algumas centenas de KB comprimidos) e verificado no CI.
- PERF-041: **code splitting por rota/feature**; carregar módulos (financeiro, comissões, relatórios) sob demanda.
- PERF-042: **virtualização** de listas e tabelas longas (renderizar só o visível). Nenhuma tabela com milhares de linhas no DOM.
- PERF-043: **evitar re-render desnecessário**: estado local o mais próximo possível de quem usa; `memo` e seletores onde a medição mostrar necessidade (não por reflexo). Busca com *debounce* curto e cancelamento de requisição anterior.
- PERF-044: **CSS próprio e modular**, sem framework utilitário; sem CSS morto; tokens em variáveis CSS. CSS conservador para o WebKitGTK alvo.
- PERF-045: **fontes locais, subconjunto (subset) em woff2**, poucas variações (1 a 2 famílias, poucos pesos). Sem fontes remotas. `font-display` adequado.
- PERF-046: **ícones em SVG inline/sprite**, poucos e simples. **Imagens quase inexistentes:** apenas o logo da loja e do produto (SVG ou imagem pequena otimizada); logo da loja redimensionado e recomprimido no cadastro. Nenhuma imagem decorativa pesada.
- PERF-047: **sem polling constante** na UI; atualização por eventos (SSE/WebSocket simples) ou consulta sob demanda. Parar atualizações quando a janela estiver oculta.
- PERF-048: **sem animação contínua em segundo plano** (loops, gradientes animados, partículas).
- PERF-049: **memória:** limpar listeners e timers; cache de requisições com tamanho máximo e expiração; revisar vazamentos com perfil de heap em sessões longas (caixa roda o dia todo).

## 6. Motion e design com orçamento de desempenho

A intenção é **um pouco de motion onde agrega** (feedback de ação, entrada de painéis, confirmação de venda) e transições fluidas, **sem derrubar PC fraco**.

- PERF-060: animar **apenas `transform` e `opacity`** (compositáveis). Evitar animar `width`, `height`, `top`, `left`, `box-shadow`, `filter`.
- PERF-061: **proibido em telas operacionais:** `backdrop-filter`/blur, sombras grandes e múltiplas, gradientes animados, filtros SVG, vídeos, partículas.
- PERF-062: durações curtas (ordem de 120–220 ms para microinterações), curvas simples; **nada que atrase a operação do caixa** (a venda nunca espera a animação terminar).
- PERF-063: respeitar `prefers-reduced-motion`.
- PERF-064: **Modo Desempenho**: detecção automática (RAM/CPU baixos, FPS medido insuficiente) com chave manual nas configurações. Nesse modo: sem transições decorativas, sombras mínimas, atualizações em lote. O visual continua limpo (bom uso de tipografia, espaçamento e cor).
- PERF-065: usar CSS puro (`transition`, `@keyframes`, View Transitions se suportado pelo WebView alvo — **verificar compatibilidade no WebKitGTK/WebView2 alvo**) antes de bibliotecas de animação; se uma biblioteca for necessária, escolher a menor e carregar sob demanda.
- PERF-066: orçamento de **quadros**: testar as transições no hardware de referência; se uma transição reduz o FPS de forma perceptível, ela é removida ou simplificada.
- PERF-067: o caixa prioriza **latência de entrada**: foco imediato no campo de bipagem, resposta à tecla sem espera, feedback sonoro/visual leve.

## 7. Tauri / WebView (Linux em especial)

- PERF-080: o WebKitGTK sem aceleração de GPU pode ser lento em efeitos que, em máquinas modernas, parecem baratos. **Projetar o visual para ser bonito sem depender de efeitos caros** (PERF-060/061).
- PERF-081: **uma janela principal**; evitar múltiplas janelas/webviews simultâneas (cada uma custa RAM).
- PERF-082: variáveis de ambiente e flags de renderização do WebKitGTK como contorno de problemas de composição devem ser **testadas e documentadas caso a caso**, não assumidas.
- PERF-083: evitar passar blobs grandes pela ponte IPC; paginar e enviar só o necessário.
- PERF-084: processos auxiliares mínimos; o servidor e a UI do mesmo PC não devem duplicar caches grandes.

## 8. Rede local e API

- PERF-090: respostas **pequenas e paginadas**; cabeçalhos de cache/ETag onde fizer sentido (catálogo).
- PERF-091: **catálogo do caixa** em cache local com invalidação por versão (evita consultar o servidor a cada bip), sempre revalidando preço no fechamento.
- PERF-092: conexão persistente (HTTP/1.1 keep-alive ou HTTP/2) entre caixa e servidor; tempos de expiração realistas para rede local lenta.
- PERF-093: limitar concorrência por dispositivo para o servidor não degradar sob 8 caixas.

## 9. Medição e prevenção de regressão

- PERF-100: **scripts de benchmark versionados** (`tools/`): tempo de abertura, consulta de busca, venda ponta a ponta, uso de RAM em 1 h/8 h.
- PERF-101: ferramentas sugeridas (validar disponibilidade): `hyperfine` para tempos; `/usr/bin/time -v`, `smem`/`ps` para RSS; `heaptrack` ou equivalente no Linux; perfil do WebView (DevTools em build de desenvolvimento) para JS e layout; `EXPLAIN QUERY PLAN` e `sqlite3_analyzer` para banco.
- PERF-102: **CI com orçamento:** tamanho de bundle e testes de desempenho com falha acima do limite.
- PERF-103: relatório de desempenho por release (hardware, SO, resultados), comparando com a release anterior.
- PERF-104: **dados de teste realistas**: 50 mil produtos, 500 mil vendas, 12 meses de histórico, para medir escala, não só "banco vazio".
- PERF-105: **teste de longa duração:** 8 h de uso simulado com 8 caixas, monitorando RAM, handles, tamanho do WAL e latência (detectar vazamentos).

## 10. Regras de ouro

1. Medir antes de otimizar; registrar a medição que justificou cada ajuste.
2. Preferir **menos código, menos dependência, menos camadas de runtime**.
3. O caixa (latência de entrada e confiabilidade) vem antes de qualquer beleza que custe desempenho.
4. Beleza barata: tipografia, espaço, alinhamento, cor e motion curto com `transform`/`opacity`.
5. Nunca carregar tudo na memória; paginar, indexar, agregar no banco.
6. Se o hardware de referência não aguentar, **mude o plano**, não a meta em silêncio.
