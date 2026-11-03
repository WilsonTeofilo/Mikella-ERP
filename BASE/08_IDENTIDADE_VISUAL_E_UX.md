# Mikella ERP — identidade visual, design system e UX

**Criado em:** 28/10/2026
**Status:** direção de design e especificação. **Nada foi desenhado, codificado nem testado em hardware real.** Os hex da paleta foram amostrados da imagem de referência e os contrastes foram calculados (seção 5.4), mas cores em tela, fontes e efeitos ainda precisam ser validados no PC de referência (i5 de 2ª geração, 4 GB, tela de baixa resolução). Tudo marcado **[DECISÃO PENDENTE]** ou **[VERIFICAR]** não deve ser tratado como fato.
**Relação com os outros documentos:** este arquivo estende `07_OTIMIZACAO_PC_FRACO.md` (seções 5, 6 e 7) e `03_ARQUITETURA_CLEAN_E_PADROES.md` (seção 7). Em conflito, **`07` vence**: o caixa (latência e confiabilidade) vem antes de qualquer beleza que custe desempenho. Posição sugerida na ordem de precedência do `00`: `04` e `05` > `06` > `03` > `02` > `07` > **`08`** > `01`.
**IDs de requisito:** `VIS-` (visual) e `UX-` (experiência).

---

## 1. Resumo da direção

**Conceito: "Letreiro e Livro-caixa".**

O Mikella nasce no balcão de pequeno comércio brasileiro. Esse balcão já tem duas linguagens visuais prontas, e elas casam com o pedido (hip hop urbano + tech corporativo):

- **Letreiro:** o cartaz de oferta pintado à mão, a etiqueta de gôndola, o adesivo, o lambe-lambe, a letra recortada. É a "rua": barulhenta, em blocos, em ângulo, em poucas cores. Dialoga com o que há de melhor nas referências (adesivos, letras em bloco, azul/preto/branco).
- **Livro-caixa:** a fita da bobina, a coluna de valores alinhada, a linha de régua, o carimbo de conferido. É o "balcão": preciso, tabular, silencioso. Dialoga com o lado tech/corporativo e com a promessa do produto (o livro imutável do caixa).

**Regra central: duas vozes, nunca misturadas na mesma tarefa.**

| Voz | Onde vive | Comportamento |
|---|---|---|
| **Rua** (letreiro) | Landing, portal, tela de ativação, frase de recuperação, estados vazios, confirmações curtas | Cobalto dominante, letra em bloco, ângulos, selos, textura de pontos. Pode ser expressiva |
| **Balcão** (livro-caixa) | Caixa (PDV), estoque, financeiro, equipe, ajustes | Fundo claro (ou noite), tipografia legível, alinhamento rígido, ortogonal, quase sem ornamento. A personalidade vem de cor, cantos cortados, números grandes e selos de estado |

A identidade é reconhecível porque as duas vozes **compartilham o mesmo vocabulário** (azul cobalto, preto de tinta, branco de papel, canto cortado, selo, número grande), não porque a tela do caixa grita.

**Onde gastamos a ousadia (uma coisa memorável por contexto):**

- Na tela do caixa: o **bloco do total**, em cobalto sólido com número enorme. O resto é calmo.
- Na landing: o **letreiro** do título (letras recortadas), uma única vez.
- No sistema inteiro: o **canto superior direito cortado**, sempre o mesmo, sempre a 45 graus.

---

## 2. Dissecação das referências

### 2.1 O que cada imagem é (e o que não é)

- **Imagem 1 (embalagens de cafeteria, "Roll Together"):** é a única referência de **cor**. O texto visível indica branding de cafeteria, com copos, caixas e cartazes. Pelo que se vê, o autor é desconhecido; trata-se de material com direitos reservados.
- **Imagens 2 e 3 (letras recortadas e folha de adesivos/UI):** a linguagem tipográfica e os elementos (rótulos de status, numerais, setas, ícones) lembram fortemente a interface de **Persona 5**. Não consigo confirmar a procedência pelas imagens. Elas servem como **referência de estrutura e de gramática**, não de cor: são multicoloridas (vermelho, amarelo, verde, laranja, rosa), enquanto você pediu azul, branco e preto.

### 2.2 Imagem 1: o que ela ensina

| Observação | Lição para o Mikella |
|---|---|
| **Três cores só** (cobalto, preto, branco) e a variedade vem de inversão: branco sobre azul, azul sobre branco, preto sobre branco | Paleta fechada. A variação de blocos vem de **inverter** as três cores, nunca de adicionar cores |
| Cobalto saturado, mas **chapado** (sem gradiente, sem brilho) | Cor chapada. Zero gradiente |
| Ilustração em **traço preto grosso com preenchimento chapado** e poucos realces brancos | Contorno grosso (2 px ou mais) como assinatura. Sem sombra suave |
| **Escala radicalmente diferente** no mesmo layout: letras gigantes extrudadas ao lado de microtexto | Hierarquia por **escala extrema** (número grande, rótulo pequeno), sem meio-termo morno |
| **Texto contornado e espelhado** ao fundo como textura | Textura tipográfica só no modo Rua, estática |
| **Selos pentagonais** (formato de etiqueta/casa) com texto em volta | O selo como unidade de informação de estado |
| Grade em **mosaico** de blocos de tamanhos diferentes, com sangria | Layouts assimétricos e modulares, nada de 3 colunas idênticas |
| Marca repetida em muitos suportes | **Repetição** como identidade: o mesmo wordmark, o mesmo corte, a mesma cor |

### 2.3 Imagem 2: letras recortadas

Cada letra é um bloco próprio, alternando preto sobre branco e branco sobre preto, com rotação, escala diferente e **contorno duplo** (filete branco, contorno preto, halo de cor deslocado) sobre fundo de concreto.

- **O que é forte:** energia, "colagem de rua", reconhecimento instantâneo.
- **O custo:** legibilidade baixa e leitura lenta. Uma análise crítica da interface de Persona 5 observa justamente que ela era difícil de interpretar, e que a sequência (Tactica) ficou mais acessível ao adotar uma linguagem construtivista com tipografia mais racional (ver Fontes, F4).
- **Decisão:** a técnica vira o componente **Letreiro** (seção 7.5): no máximo **3 palavras**, só no modo Rua, com texto real no DOM. Nunca em tela operacional, nunca em parágrafo, nunca em rótulo de campo.

### 2.4 Imagem 3: folha de adesivos e elementos de UI

Esta é a referência mais útil para o produto, porque é um **vocabulário de estados**:

| Elemento da folha | Função na UI de origem | Equivalente no Mikella |
|---|---|---|
| Rótulos curtos em bloco ("WEAK", "LEVEL UP", "STOCK FULL", "CRITICAL") | Dizem o **estado** de relance | **Selos** de estado: "ESTOQUE BAIXO", "VENCE EM 7 DIAS", "COTA 61/85", "DIFERENÇA" |
| Numerais em **vários estilos**: pontilhado, itálico pesado, condensado | O número é tratado como personagem | **Número grande** do total, das cotas e do saldo, com tratamento próprio |
| Pílulas pequenas com marcador à esquerda ("CHECK", "GO TO") | Ações e categorias | Marcas de estado discretas nas tabelas |
| Ícones geométricos simples (saco de dinheiro, símbolo da moeda, exclamação em losango) | Reconhecimento imediato | Poucos ícones, de formas fortes, no mesmo traço |
| Conjunto de ícones de clima em contorno | Consistência de traço | Um único conjunto de ícones, mesmo traço e mesma grade |
| Fundo cinza neutro | Os elementos aparecem sem competição | Fundo calmo no modo Balcão |

**Lição de gramática:** em vez de uma interface com 40 estilos diferentes, a folha usa **poucas famílias de forma repetidas** (bloco recortado, pílula, ícone geométrico). O Mikella vai ter **cinco formas** e nenhuma além delas (seção 7).

### 2.5 O que a pesquisa sobre Persona 3 Reload acrescenta

(Resumo em palavras próprias; fontes F1 a F7 no final do documento.)

1. **Um conceito por jogo, uma cor por jogo.** A UI de Persona 3 Reload partiu do azul do Persona 3 original e dos temas de água corrente e bolhas. A direção de arte contrasta isso com Persona 5, cujo desenho é pop-punk e antissistema (F1).
2. **A UI carrega informação e identidade ao mesmo tempo.** A equipe de Persona 5 descreveu que a interface precisa comunicar dados, mas também o caráter do jogo, e que o ângulo e o contraste das telas dão contexto ao que está sendo mostrado. Em Persona 5, quase não usaram sub-cores, para manter o vermelho distintivo (F3).
3. **UI é função sênior, não tarefa de quem chegou por último.** Segundo o diretor de Persona 5, no Japão o trabalho de UI costuma ir para funcionários novos, enquanto a Atlus entrega a designers experientes e os remunera bem. A equipe também diz que a UI define grande parte da atmosfera (F2). **Para o Mikella:** o design system tem **um dono**, e nenhuma tela é feita "no improviso".
4. **Menu de PDV é menu de jogo:** ambos são usados centenas de vezes por dia. Persona prova que repetição não precisa ser monótona, desde que a estrutura seja estável e a personalidade esteja nos detalhes (formas, transições curtas, tipografia).
5. **Cuidado com o fundo "só azul":** uma resenha de jogador (opinião, não dado) comenta que telas azuis lisas parecem vazias quando falta textura. Resposta do Mikella: a textura vem das **linhas de régua do livro**, do **halftone** (só Rua) e da **tipografia**, nunca de gradiente.

### 2.6 O que absorver e o que descartar

| Absorver | Descartar |
|---|---|
| Azul cobalto como cor-assinatura, com preto e branco | Cores extras das imagens 2 e 3 (vermelho como marca, rosa, roxo, verde néon) |
| Contorno grosso, cor chapada | Gradientes, brilho, vidro fosco, bolhas, transparência em camadas |
| Número grande e rótulo pequeno | Letra recortada em texto corrido |
| Selo como unidade de estado | Ícones, máscaras, coroas e personagens do jogo |
| Ângulo e contraste para dar contexto | Efeitos 3D, extrusão, sombras profundas |
| UI com dono e orçamento próprio | Imitar menu de jogo (menu circular, cartas virando) |

### 2.7 Originalidade e propriedade intelectual (regra invariável de design)

- As referências são obras protegidas. O Mikella usa **princípios** (paleta de três cores, tipografia em bloco, selos de estado), **nunca ativos**: nada de traçar, recompor, vetorizar nem recortar letras, ícones, personagens, rótulos ou composições das imagens.
- Não usar fontes que imitem a tipografia de Persona 5 (existem recriações de fãs), nem nomes, termos ou símbolos da franquia na marca, na interface ou na copy.
- Toda forma, letra recortada, ícone e textura do Mikella é **desenhada para o Mikella** e registrada com autoria e licença em `docs/design/ASSETS.md`.
- A tipografia do Letreiro é **montada com a família de display escolhida** (seção 6) com blocos e rotações próprias, não digitada em uma fonte "estilo Persona".

---

## 3. Princípios de design aplicados (requisito do dono do produto)

Cada princípio vira regra verificável.

| Princípio | Regra no Mikella | ID |
|---|---|---|
| **Hierarquia** | Cada tela tem **um** elemento dominante (ex.: o total no caixa) e **uma** ação primária. Hierarquia por tamanho, peso e posição, nessa ordem. Máximo de 3 níveis tipográficos visíveis ao mesmo tempo, além do número grande | VIS-001 |
| **Contraste** | Contraste de **papel** (tamanho e peso: 56 px contra 13 px) antes de contraste de cor. Texto e componentes cumprem os mínimos da seção 5.4 | VIS-002 |
| **Alinhamento** | Grade de **4 px**, margens e colunas fixas. Texto sempre alinhado à esquerda; **números alinhados à direita** com algarismos tabulares. Nada centralizado, exceto estados vazios e confirmações | VIS-003 |
| **Proximidade** | Espaço **entre** grupos é pelo menos o dobro do espaço **dentro** do grupo. Rótulo colado ao campo. Nada de divisória onde o espaço já separa | VIS-004 |
| **Repetição** | As mesmas 5 formas, as mesmas 4 cores base e o mesmo canto cortado em tudo. Se um componente novo precisa de uma sexta forma, ele é rejeitado ou o sistema é revisado | VIS-005 |
| **Equilíbrio** | Peso visual **assimétrico e estável**: coluna larga de trabalho e trilho estreito de atenção. Sem simetria forçada nem grades de cartões idênticos | VIS-006 |
| **Espaço negativo** | Densidade controlada: no caixa, tabela de itens e total ocupam a tela e todo o resto some; nas demais telas, no máximo **7 blocos** de informação por vista | VIS-007 |

---

## 4. Lista de proibições (anti "IA slop") e teste de identidade

### 4.1 Proibido (vale para landing, portal, desktop e materiais)

| Proibido | Por quê |
|---|---|
| Gradientes de fundo ou decorativos, especialmente roxo, azul néon e rosa magenta | Marca registrada do visual "gerado por IA" |
| Neon forte, brilho (glow), vidro fosco (`backdrop-filter`) | Genérico e **também proibido por desempenho** (`07`, PERF-061) |
| Grades de 3 colunas com cartões idênticos, cantos `rounded-xl` e sombras suaves iguais | Layout de template |
| Ícone flutuante acima do título de cada cartão | Template |
| Herói padrão: título centralizado, subtítulo cinza, selo "novidade" e dois botões lado a lado | Template |
| Números inventados ("99,9% de satisfação"), gráficos circulares fictícios, avatares genéricos | Preenchimento sem informação. **Só dado real ou nenhum** |
| Emojis em qualquer parte (interface, copy, commits de design, documentação) | Decisão do dono do produto |
| Fontes de uso padrão de SaaS ou ilegíveis em tamanho pequeno | Ver seção 6.2 |
| Preto "tingido" como fundo padrão por reflexo, rótulos em caixa-alta com espaçamento largo acima de cada título, seta ao final de cada link, listas de metadados separadas por ponto | Sinais de template. Exceções só se justificadas aqui |
| Cor como **único** portador de significado | Acessibilidade (seção 13) |
| Animação contínua, parallax, partículas | `07`, PERF-048 e PERF-061 |

### 4.2 Teste de identidade (revisão de cada tela)

1. **Teste do logo coberto:** esconda o logo. A tela ainda parece Mikella (cobalto, corte, número grande, selo)? Se poderia ser de qualquer SaaS, reprovada.
2. **Teste do borrão:** com a tela desfocada, dá para dizer o que é mais importante? Se não, falta hierarquia.
3. **Teste do balcão:** uma pessoa de pé, a 1 metro de uma tela de 1366x768, lê o total e o estado do caixa? Se não, aumentar.
4. **Teste da ousadia única:** há mais de uma coisa gritando na mesma vista? Cortar até sobrar uma.
5. **Teste do conteúdo real:** todo número, nome e texto é real (ou claramente exemplo)? Placeholder genérico reprova.

---

## 5. Cor

### 5.1 Princípio

Paleta **fechada**: cobalto, preto, branco, mais um cinza-azulado para superfície e **um** amarelo, restrito a aviso e ao halo do Letreiro. Os tons de estado (perigo, sucesso) existem **só por função**, em pequenas áreas, com texto e forma junto.

### 5.2 Tokens

O azul foi **amostrado da imagem 1** (valor dominante próximo de `#0040D8`). Calibrar em monitor real e em tela barata (painéis TN e IPS fracos deslocam saturação).

| Token | Hex | Papel |
|---|---|---|
| `azul-600` | `#0040D8` | Cor da marca, ações primárias, bloco do total, seleção |
| `azul-700` | `#0030A8` | Estado pressionado e hover sobre azul-600; texto azul sobre fundo claro de alto contraste |
| `azul-300` | `#7DA2FF` | Texto e ícone azul no modo Noite |
| `azul-100` | `#DCE6FF` | Fundo de linha selecionada, realce curto de item novo |
| `preto` | `#000000` | Tinta: texto, contornos, trilho de navegação |
| `branco` | `#FFFFFF` | Papel: fundo de trabalho, texto sobre azul e preto |
| `papel` | `#EEF0F6` | Fundo de área de trabalho no modo Dia, painéis laterais |
| `cinza-500` | `#5E6377` | Texto secundário e bordas de campo (passam 3:1 e 4,5:1) |
| `linha` | `#C9CCD8` | **Apenas** réguas decorativas do livro. **Não** serve de borda de campo (1,6:1) |
| `noite-2` | `#141623` | Painéis no modo Noite (base do modo Noite é `preto`) |
| `amarelo` | `#FFD21F` | Aviso, validade a vencer, halo do Letreiro. **Sempre com texto preto** |
| `vermelho` | `#C81E16` | Perigo, diferença de caixa, estoque crítico |
| `verde` | `#07733C` | Confirmação, conferido |
| `vermelho-claro` | `#FF6B5E` | Perigo no modo Noite |
| `verde-claro` | `#3DD68C` | Sucesso no modo Noite (texto e ícone pequenos apenas) |

**Banidos da paleta:** roxo, magenta, rosa, ciano néon e qualquer gradiente. As demais cores das imagens 2 e 3 **não** entram.

### 5.3 Modos e proporção

| Modo | Fundo | Texto | Uso |
|---|---|---|---|
| **Dia** (padrão operacional) | `branco` e `papel` | `preto` | Caixa e telas de trabalho. Melhor leitura em ambiente iluminado e em telas de baixa qualidade |
| **Noite** | `preto` e `noite-2` | `branco` | Opção do usuário para uso prolongado em ambiente escuro |
| **Rua** | `azul-600` dominante, com preto e branco | `branco` e `preto` | Landing, portal, ativação, frase de recuperação, estados vazios |

Proporção aproximada de área: **Dia**: 75% branco/papel, 18% preto, 6% azul, 1% estados. **Rua**: 55% azul, 25% preto, 20% branco.

- VIS-010: o modo **Dia** é o padrão do desktop. Modo Noite é opção em Ajustes. O modo Rua **nunca** cobre a tela do caixa durante a venda.
- VIS-011: a seleção atual (linha, item de menu, aba) é sempre **inversão de cor** (cobalto com texto branco), a forma mais reconhecível e barata de mostrar foco.
- VIS-012: gráficos usam **um** tom de azul para a série principal, preto para comparação e amarelo para destaque pontual. Sem arco-íris. Antes de criar qualquer gráfico, aplicar a regra de acessibilidade da seção 13.

### 5.4 Contrastes calculados (WCAG, razão de luminância)

Medidos por script em 28/10/2026 com os hex acima. Mínimos: texto normal 4,5:1; texto grande (24 px ou 19 px em negrito) 3:1; contornos de componentes e ícones informativos 3:1.

| Combinação | Razão | Uso permitido |
|---|---|---|
| branco sobre `azul-600` | 7,71 | Texto em qualquer tamanho |
| branco sobre `azul-700` | 10,56 | Texto em qualquer tamanho |
| `azul-600` sobre branco | 7,71 | Texto e ícone |
| `azul-600` sobre `papel` | 6,76 | Texto e ícone |
| `azul-700` sobre `papel` | 9,27 | Texto |
| `azul-600` sobre `azul-100` | 6,17 | Texto |
| **preto sobre `azul-600`** | **2,72** | **Proibido para texto** |
| **`azul-600` sobre preto** | **2,72** | **Proibido para texto e ícone**. No modo Noite usar `azul-300` |
| `azul-300` sobre preto | 8,48 | Texto no modo Noite |
| `azul-300` sobre `noite-2` | 7,26 | Texto no modo Noite |
| branco sobre preto | 21,00 | Texto |
| preto sobre `papel` | 18,43 | Texto |
| `cinza-500` sobre branco | 5,95 | Texto secundário, borda de campo |
| `cinza-500` sobre `papel` | 5,23 | Texto secundário |
| preto sobre `amarelo` | 14,49 | Selo de aviso |
| **`amarelo` sobre branco** | **1,45** | **Proibido** como texto ou ícone sobre claro |
| `amarelo` sobre preto | 14,49 | Permitido |
| branco sobre `vermelho` | 5,75 | Selo de perigo |
| `vermelho` sobre `papel` | 5,05 | Texto de erro |
| branco sobre `verde` | 5,95 | Selo de sucesso |
| `verde` sobre `papel` | 5,23 | Texto de sucesso |
| `vermelho-claro` sobre `noite-2` e preto | acima de 7 | Modo Noite |
| `verde-claro` sobre preto | acima de 10 | Modo Noite |
| `linha` sobre branco | 1,60 | **Somente decorativo**; reprova como borda de campo |

- VIS-013: toda nova combinação de cor entra nesta tabela **com a razão calculada** antes de ser usada. Um script em `tools/contrast/` valida os pares dos tokens no CI.
- Nota: o 21:1 de branco sobre preto é o máximo teórico da escala WCAG (luminância 1 contra 0), não uma medição.

---

## 6. Tipografia

### 6.1 Papéis

| Papel | Família | Uso |
|---|---|---|
| **Display** | **Archivo**, instância condensada em peso extra-negrito (ou equivalente, ver 6.4) | Wordmark, títulos de landing, o Letreiro, selos, o número grande do total |
| **Texto e dados** | **Source Sans 3** | Todo o resto: interface, tabelas, formulários, parágrafos, valores monetários em tabelas |

Escolha deliberada de **duas famílias claramente distintas**: uma condensada, pesada e urbana (cartaz de rua), outra neutra, aberta e de leitura fácil (livro-caixa). A personalidade mora no display; o texto precisa ser invisível.

### 6.2 Por que essas famílias (e quais foram descartadas)

- **Source Sans 3** (licença livre, SIL OFL, da Adobe): é citada em guias de interface densa como a de melhor *hinting*, o que importa em **Windows com ClearType** e em telas de baixa resolução, exatamente o hardware-alvo. Suporta algarismos **tabulares**, essenciais para colunas de valores (F5). Tem pesos de 200 a 900, o que cobre o texto e um número grande de reserva.
- **Archivo** (SIL OFL): superfamília com eixos de largura e peso, o que permite uma versão **condensada e extra-pesada** com a mesma proporção industrial dos cartazes e embalagens de referência, sem recorrer às fontes que viraram estereótipo.
- **Descartadas, e por quê:**
  - **Bebas Neue e Oswald:** são a resposta automática para "fonte de cartaz urbano" (guias da web as sugerem como substitutas da tipografia de Persona 5, F6). Viram clichê imediato.
  - **Inter, Space Grotesk, Poppins, Montserrat:** a cara padrão de SaaS. Reprovadas pelo teste de identidade.
  - **Qualquer fonte "estilo Persona" feita por fãs:** risco de propriedade intelectual (seção 2.7).
  - **Fontes decorativas de grafite ou manuscritas:** ilegíveis em tamanho de interface.
- **IBM Plex Sans** fica como reserva de texto caso Source Sans 3 falhe nos testes: é desenhada para interfaces de dados e tem variante condensada.

### 6.3 Regras de uso

- VIS-020: **maiúsculas só no Display** (selos, títulos de cartaz, Letreiro) e em até 3 palavras. Rótulos, cabeçalhos de tabela, abas e botões usam **caixa de frase**.
- VIS-021: **valores monetários e quantidades** usam algarismos tabulares e lining (`font-variant-numeric: tabular-nums lining-nums`), alinhados à direita. Formato brasileiro: `R$ 1.234,56`. O prefixo `R$` fica na mesma linha de base, a 60% do tamanho do número.
- VIS-022: valor **negativo** mostra o sinal (`− R$ 50,00`) e a cor vermelha **juntos**. Nunca só a cor.
- VIS-023: o número grande do total usa Display. **[VERIFICAR]** se a instância escolhida de Archivo tem algarismos tabulares. Se não tiver, usar **Source Sans 3 em peso 900** para o número, e manter o Display só para texto.
- VIS-024: comprimento de linha de parágrafo até **70 caracteres**. Entrelinha de 1,4 no texto e de 1,0 a 1,1 no Display.
- VIS-025: espaçamento entre letras do Display em `0` a `+0,01em`. Nada de espaçamento largo em rótulos.

### 6.4 Escala (px CSS; base de 14 px no desktop)

| Nível | Tamanho | Peso | Uso |
|---|---|---|---|
| Número do caixa | 56 a 72 | Display ou texto 900 | Total da venda, saldo do caixa |
| Display L | 44 | Display | Título de landing |
| Display M | 32 | Display | Título de tela (Rua), selo grande |
| Título | 20 | Texto 700 | Título de seção e de diálogo |
| Subtítulo | 16 | Texto 600 | Subseção, valor em destaque |
| **Texto** | **14** | Texto 400 | Padrão da interface |
| Texto do caixa | 16 | Texto 400 | Lista de itens da venda |
| Auxiliar | 13 | Texto 400 | Legendas e metadados. **Mínimo absoluto: 13 px** |

- VIS-026: tamanho mínimo de 13 px em qualquer lugar do desktop. A landing usa 16 px para texto corrido.
- VIS-027: a escala é **parametrizada por tokens**, e o fator de escala do Windows (125% e 150% são comuns em telas pequenas) não pode quebrar o layout.

### 6.5 Carregamento e orçamento de peso

- VIS-030: **fontes locais em woff2, com subconjunto latino** (inclui acentos do português e símbolos monetários). Sem fontes remotas (`07`, PERF-045, e CSP do `04`, SEG-061).
- VIS-031: orçamento de partida: **até 3 arquivos e até ~120 KB comprimidos no total** (Display condensado em 1 peso; Source Sans 3 em 1 arquivo variável ou em 3 pesos estáticos: 400, 600 e 700, mais 900 se necessário). **Hipótese a medir no spike**; se estourar, cortar pesos antes de cortar acentos.
- VIS-032: `font-display` configurado para **não bloquear** o caixa; o texto aparece imediatamente na fonte de sistema e troca sem mudar o layout (reservar métricas com `size-adjust`/`ascent-override` quando suportado).
- VIS-033: **[VERIFICAR]** antes de adotar: licença (OFL) no repositório oficial de cada fonte, presença de algarismos tabulares e de todos os glifos usados (`ç`, `ã`, `õ`, `R$`, `−`), tamanho real do subconjunto e renderização em **Lubuntu (WebKitGTK/FreeType)** e **Windows 10 (WebView2)**.
- VIS-034: alternativas se Archivo falhar nos testes: **Big Shoulders Display** (condensada industrial, SIL OFL) ou **Barlow Condensed** (SIL OFL). Mesma regra de verificação.

---

## 7. Forma e linguagem gráfica

O sistema tem **cinco formas** e nenhuma além delas.

### 7.1 Forma 1: o Corte

Retângulo com o **canto superior direito cortado a 45 graus**. Sempre o mesmo canto, sempre a 45 graus. É a assinatura (eco do selo pentagonal da referência e dos blocos recortados).

| Variante | Tamanho do corte | Uso |
|---|---|---|
| Corte-S | 6 px | Botões, campos, marcas de estado |
| Corte-M | 12 px | Painéis, diálogos, bloco do total |
| Corte-L | 24 px | Painéis de destaque no modo Rua |

- VIS-040: **raio de borda zero** em todo o sistema. Nada de `rounded-*`. A suavidade vem do espaço, não do canto.
- VIS-041: implementação por `clip-path: polygon(...)` **estático** sobre um elemento externo (preto) com um interno recuado (branco), formando o contorno. **Proibido animar `clip-path`** (repinta). Limitar a **~30 elementos com corte visíveis por tela**. Linhas de tabela **não** têm corte. **[VERIFICAR]** custo de pintura no WebKitGTK sem GPU.
- VIS-042: contorno de **2 px preto** em campos, botões secundários e painéis de destaque. Réguas de tabela em 1 px na cor `linha`. Contorno de 3 px ou mais só no modo Rua.

### 7.2 Forma 2: o Selo (estado em destaque)

Etiqueta em **pentágono** (ponta à direita), com **uma a três palavras** em Display e maiúsculas. Carrega **estado importante** e é **rara**: no máximo **3 selos por tela**.

| Selo | Fundo e texto | Exemplos de texto |
|---|---|---|
| Atenção | `amarelo` com texto preto | `VENCE EM 7 DIAS`, `ESTOQUE BAIXO` |
| Perigo | `vermelho` com texto branco | `DIFERENÇA NO CAIXA`, `VENCIDO` |
| Confirmação | `verde` com texto branco | `CONFERIDO`, `VENDA OK` |
| Marca/plano | `azul-600` com texto branco | `COTA 61/85`, `PLANO PAGO` |
| Neutro/bloqueado | Branco com contorno tracejado preto | `SÓ NO PAGO` |

- VIS-043: o selo **sempre** tem texto. A forma e o texto repetem a cor (acessibilidade).
- VIS-044: **inclinação** de -2 a -3 graus só em selos do modo Rua e em confirmações temporárias. **Zero inclinação** em tabelas e formulários.

### 7.3 Forma 3: a Marca de estado (estado discreto)

Para tabelas e listas: pílula **retangular com Corte-S**, texto em caixa de frase, peso 600, 13 px, com um **pequeno quadrado ou triângulo** como indicador à esquerda. Ex.: `Aberta`, `Em fechamento`, `Pendente`, `Pago`. A marca de estado **nunca** gira nem usa maiúsculas.

### 7.4 Forma 4: a Régua (livro-caixa)

Linha horizontal de 1 px em `linha` que organiza listas e tabelas como livro contábil, com **valores alinhados à direita**. Linha total em **2 px preto** acima do total (como o traço de soma). É o que dá textura ao modo Balcão sem nenhum ornamento.

- VIS-045: a **fita** (lista vertical de movimentações) é o motivo recorrente do produto: caixa, estoque e financeiro mostram movimentações em fita, nunca como grade de cartões.

### 7.5 Forma 5: o Letreiro (somente modo Rua)

Título em que **cada letra é um bloco próprio**: preto sobre branco ou branco sobre preto, com rotações de ±2 a ±4 graus, escalas levemente diferentes e, opcionalmente, **um** halo amarelo deslocado. É a técnica da imagem 2, reescrita com a **família Display do Mikella** e com blocos e rotações **próprios** (seção 2.7).

- VIS-050: **máximo de 3 palavras** e **uma ocorrência por página**.
- VIS-051: construído com **texto real no DOM** (um `span` por letra com `aria-hidden` nos fragmentos e o texto completo em um rótulo acessível). Sem imagem de texto.
- VIS-052: no máximo **~40 elementos** por Letreiro. Sem animação de entrada por letra. Respeita `prefers-reduced-motion`.
- VIS-053: **nunca** em telas operacionais, rótulos, botões, tabelas ou mensagens de erro.
- VIS-054: se a leitura for prejudicada (teste de 1 metro, 3 segundos), o Letreiro é simplificado.

### 7.6 Textura: pontos (halftone), apenas modo Rua

- VIS-055: padrão de **pontos em uma cor**, como **SVG estático** (padrão repetido, abaixo de 2 KB), usado em painéis azuis da landing e da ativação. **Não** aparece em telas operacionais. Pode ser desligado pelo Modo Desempenho.

### 7.7 Ícones

- VIS-060: **um conjunto único**, traço de 2 px, pontas **retas** e junções **em ângulo** (nada arredondado), grade de 20 px, no máximo **~30 ícones** no produto inteiro, em **sprite SVG**.
- VIS-061: ícones são **apoio**, nunca substituem o texto em navegação. Item de menu sempre tem rótulo escrito.
- VIS-062: partir de um conjunto livre (ex.: Lucide, com licença ISC, ou Phosphor, com licença MIT; **[VERIFICAR]** licença e versão) **reestilizado** com pontas retas e junções em ângulo, ou desenhar do zero. Registrar em `docs/design/ASSETS.md`.

### 7.8 Grade, espaçamento e elevação

- VIS-070: unidade de **4 px**. Escala: 4, 8, 12, 16, 24, 32, 48, 64, 96.
- VIS-071: altura de controle de **40 px** (padrão) e **48 px** no caixa. Alvo de clique mínimo de 40 px.
- VIS-072: **sem sombras difusas**. Elevação por contorno e por espaço. Exceção única: **placa deslocada** (`box-shadow` sólido de 4 px, sem desfoque, preto) no botão primário do modo Rua. No Modo Desempenho some.
- VIS-073: largura **mínima de viewport 1024x600 CSS px** (1366x768 com escala de 125% do Windows resulta em cerca de 1093x614). Layout testado em 1024x600, 1280x720, 1366x768 e 1920x1080.

---

## 8. Motion (dentro do orçamento do `07`)

Todo motion respeita PERF-060 a PERF-067: só `transform` e `opacity`; 120 a 220 ms; a venda **nunca** espera a animação terminar; `prefers-reduced-motion` e Modo Desempenho desligam tudo que é decorativo.

| Momento | Animação | Duração |
|---|---|---|
| Abertura de painel lateral | `translateX(12px)` para 0 mais opacidade | 160 ms |
| Troca de tela | Sem animação (ou fade de 100 ms) | 0 a 100 ms |
| Item adicionado à venda | Camada `azul-100` por cima da linha, que some por opacidade | 220 ms |
| Selo de confirmação (ex.: `VENDA OK`) | `scale(0.92)` para 1 mais opacidade; permanece ~1,2 s no canto da área de trabalho; sai por opacidade | 140 ms entrada e 120 ms saída |
| Entrada de telas do modo Rua (login, ativação) | Um bloco cobalto com borda diagonal desliza por `translateX` e revela o conteúdo. **Uma vez** | 200 ms |
| Landing | **Um único momento orquestrado** (seção 11.2). Nenhuma animação de entrada por seção | Ver 11.2 |
| Carregamento | Texto estático "Carregando" após 400 ms. **Sem spinner contínuo** | n/a |

- VIS-080: **proibido:** parallax, partículas, animação em laço, entrada com deslize em cada seção, efeito ao passar o mouse em cada cartão.
- VIS-081: transições nascem desligadas no **Modo Desempenho** (`07`, PERF-064). O visual continua inteiro, porque não depende de movimento.
- VIS-082: toda animação nova entra no teste de quadros do hardware de referência (`07`, PERF-066). Se cair o FPS, sai.

---

## 9. Arquitetura de informação e UX

### 9.1 Objetivo de UX

O usuário nunca fica perdido: sempre sabe **onde está, qual é a próxima ação e se o caixa está seguro**. A experiência do Bling foi descrita pelo dono do produto como excesso de módulos e ruído. O contra-desenho é **subtração**: menos destinos, menos níveis, uma ação principal por tela.

### 9.2 Regras estruturais

- UX-001: **no máximo 7 destinos** no menu principal: `Caixa`, `Produtos`, `Estoque`, `Financeiro`, `Comissões` (aparece como bloqueada com selo `SÓ NO PAGO` no Gratuito, sem esconder), `Equipe`, `Ajustes`.
- UX-002: **no máximo 2 níveis** de navegação (destino e aba). Sem menu suspenso em cascata e sem "mega menu".
- UX-003: **uma ação primária por tela**, em cobalto. Ações secundárias com contorno; terciárias como texto.
- UX-004: o nome na interface é o nome do **objetivo do usuário** ("Fechar caixa", "Receber mercadoria"), não do sistema ("Sessão", "Entidade").
- UX-005: **progressive disclosure:** o que é raro (configuração avançada) fica atrás de "Mais opções", nunca no caminho principal.
- UX-006: **busca global** (`Ctrl+K`, "Ir para...") como atalho opcional, com no máximo 10 comandos nomeados. **[DECISÃO PENDENTE V5]** se entra no MVP.
- UX-007: o trilho de navegação é **preto, fixo, com texto** (nunca só ícones), recolhível. A seleção atual é cobalto invertido.

### 9.3 Barra de estado do caixa (confiança)

Uma faixa fixa no topo, em todas as telas, mostra três fatos em palavras claras:

- **Sessão:** `Caixa 02 aberto desde 08:02`.
- **Segurança:** `Caixa protegido` (gravado no servidor) ou `Sem conexão com o servidor. A venda continua e será enviada.` (RES-020 e RES-021).
- **Internet e backup:** `Sem internet. O caixa funciona normal.` e `Último backup: ontem 22:10`.

É a resposta visível à maior dor do produto (o caixa que volta ao valor inicial). Esses estados usam **Marca de estado** (quieta), nunca selo grande, para não alarmar o operador sem necessidade.

- UX-010: perda de energia e reinício mostram uma tela única: `O caixa voltou exatamente de onde parou. Saldo: R$ 800,00.` com botão `Continuar vendas` como ação primária.

### 9.4 Atalhos de teclado (o caixa é operável sem mouse)

- UX-020: foco **sempre** no campo de bipagem ao abrir o caixa e ao fechar qualquer diálogo.
- UX-021: atalhos visíveis no rodapé (`F2 Buscar`, `F3 Quantidade`, `F4 Desconto`, `F6 Sangria`, `F7 Cancelar item`, `F8 Pagamento`, `F9 Fechar caixa`, `F10 Finalizar`). **[DECISÃO PENDENTE V6]** confirmar mapeamento com operadores reais.
- UX-022: ordem de `Tab` lógica e visível; `Esc` fecha o diálogo; `Enter` confirma a ação primária.
- UX-023: feedback de bipagem em menos de 100 ms (`07`, PERF-005): linha entra, valor atualiza, som leve opcional.

### 9.5 Voz e copy

- UX-030: português do Brasil, direto, caixa de frase, verbos no imperativo ("Fechar caixa", "Salvar alterações"). Nada de "Oops", exclamação, humor forçado nem emoji.
- UX-031: **a mesma ação tem o mesmo nome em todo o fluxo**: o botão "Finalizar venda" gera a confirmação "Venda finalizada".
- UX-032: **erro explica o que houve e como resolver**, sem se desculpar e sem vaguear. Ex.: `Valor menor que o total. Faltam R$ 12,30.`
- UX-033: **vazio é convite à ação**: `Nenhum produto ainda. Cadastre o primeiro ou importe uma nota de compra.`
- UX-034: **cota e limite** sempre com número real e saída clara (RF-073, RF-094, FRD-024): `Você criou 61 de 85 produtos neste mês. Faltam 24.` e, ao estourar: `Limite do mês atingido. O que já existe continua funcionando. Volta a liberar em 01/11 ou no plano Pago.`
- UX-035: alertas de validade (RF-023) dizem **o que fazer**: `Whey 900g vence em 7 dias. Considere uma promoção.` e persistem até a leitura confirmada.

---

## 10. Telas-chave (esboços de estrutura)

Os esboços definem **hierarquia e alinhamento**, não aparência final. Medidas em CSS px para 1366x768 em escala de 100%.

### 10.1 Caixa (PDV): tela principal do produto

```
+------------------------------------------------------------------------------------+
| Caixa 02 aberto desde 08:02 | Caixa protegido | Sem internet: funciona normal        |
+--------------------------------------------------------+---------------------------+
| Bipe ou digite o produto  [_________________________]  |  TOTAL                    |
|                                                        |                           |
|  #  Produto                  Qtd    Unitário  Subtotal |   R$ 184,70               |
|  ------------------------------------------------------|   (bloco cobalto sólido,  |
|  1  Whey 900g Baunilha        1      129,90    129,90  |    número 64 px branco,   |
|  2  Creatina 300g             1       54,80     54,80  |    Corte-M)               |
|  3  ...                                                |                           |
|                                                        |  7 itens                  |
|                                                        |  Vendedor 23 · Comanda 118|
|                                                        |  ------------------------ |
|                                                        |  Pagamento          F8    |
|                                                        |  [  Finalizar   F10  ]    |
+--------------------------------------------------------+---------------------------+
| F2 Buscar  F3 Qtd  F4 Desconto  F6 Sangria  F7 Cancelar item  F9 Fechar caixa      |
+------------------------------------------------------------------------------------+
```

- **Dominante:** o bloco do total (cobalto). **Ação primária:** `Finalizar`.
- Lista em **fita** com réguas de 1 px, números alinhados à direita, sem zebra e sem cartões. Linha selecionada em cobalto invertido.
- A UI mostra o total como **prévia**; o valor oficial vem do servidor (`03`, seção 7).
- O vendedor e a comanda aparecem como **informação fixa** (`05`, FRD-060 a FRD-062); mudar vendedor depois exige aprovação.
- **Diálogo de aprovação (PIN):** painel Corte-M no centro, campo de PIN, motivo, `Aprovar` (primária) e `Cancelar`.
- **Fechamento de caixa:** tela de **contagem** em fita (cédulas e moedas), com diferença calculada **depois** quando o fechamento cego estiver ativo (FRD-052). Diferença usa selo `DIFERENÇA NO CAIXA` apenas ao final.

### 10.2 Início do dono (painel)

Sem grade de cartões e sem gráficos decorativos.

```
+--------------------------------------------------+----------------------------+
| Hoje, 28/10                                      |  Atenção                   |
|                                                  |                            |
|  Faturamento    R$ 4.318,20                      |  [VENCE EM 7 DIAS]         |
|  Lucro estimado R$ 1.102,40                      |   Whey 900g, lote 12       |
|                                                  |   Marcar como lido         |
|  Caixas                                          |                            |
|  Caixa 01  Ana    Aberto   R$ 1.240,00           |  [COTA 61/85]              |
|  Caixa 02  Bruno  Aberto   R$   800,00           |   Faltam 24 produtos       |
|  ...                                             |                            |
|                                                  |  Backup: ontem 22:10       |
+--------------------------------------------------+----------------------------+
```

- Coluna larga com **fita do dia**; trilho estreito com **selos de atenção** (no máximo 3).
- Um único gráfico opcional (faturamento dos últimos 30 dias, barras em azul sólido). Antes de implementar gráficos, seguir a diretriz de dataviz do projeto.
- Nenhum número inventado em demonstração: usar dado real ou rotular `Exemplo`.

### 10.3 Estoque e validade

- Tabela de produtos em fita (virtualizada, `07`, PERF-042), busca no topo, coluna de saldo à direita.
- Aba `Validade`: lista ordenada por vencimento, com Marca de estado (`No prazo`, `Vence em 60 dias`, `Vence em 7 dias`, `Vencido`). Ao abrir o item, o botão primário é `Confirmar leitura` (RF-023) e a sugestão `Criar promoção` aparece como ação secundária.
- Contador de cota de produtos **sempre visível** na tela de cadastro (seção 11 de componentes).

### 10.4 Financeiro

- Abas: `Fluxo de caixa`, `A pagar`, `A receber`, `Resultado`. Máximo de 4.
- No Gratuito, um filtro de período limitado a 6 meses com o texto `Histórico completo no plano Pago` em **Marca de estado**, não em banner (RF-094).

### 10.5 Equipe e permissões

- Lista de pessoas com papel; ao abrir, **grupos de capacidades** em caixas de seleção, agrupadas por assunto (Vendas, Estoque, Financeiro, Equipe). Sem matriz gigante.
- Ações que o usuário não pode fazer por hierarquia (RF-001) aparecem **desabilitadas com motivo**, e não escondidas.

### 10.6 Ativação e frase de recuperação (modo Rua)

Tela **cobalto**, calma e inevitável. A frase de recuperação em fita numerada (**aqui a numeração é uma sequência real**), fonte de texto em 20 px, com botões `Copiar` e `Imprimir`, e uma caixa de confirmação `Guardei a frase em local seguro`. O aviso de que perder a frase perde o backup (P3) usa **selo de perigo** e texto claro. A tela **não** avança sem a confirmação.

---

## 11. Landing e portal

### 11.1 Estrutura visual (conteúdo definido em `01`, seção "Estrutura de conteúdo")

```
+--------------------------------------------------------------------------+
| MIKELLA                                   Planos   Entrar   [Criar conta] |
+--------------------------------------------------------------------------+
|  (painel cobalto, textura de pontos, alinhado à esquerda)                 |
|                                                                          |
|   A LUZ CAIU.            <- Letreiro (3 palavras, uma vez)                |
|   O caixa continua em R$ 800,00.                                          |
|                                                                          |
|   Sistema de gestão para pequenas lojas. Vende sem internet, roda em     |
|   computador antigo.                                                      |
|                                                                          |
|   [Criar conta grátis]   Ver os planos                                    |
+--------------------------------------------------------------------------+
```

- **Alinhado à esquerda**, assimétrico. Nada de selo "novidade", nada de dois botões equivalentes (o secundário é link de texto).
- O título vem da **dor real** do produto (o caixa que volta ao início do dia). **Toda promessa só vai ao ar depois dos testes de `06`, seção 8** (`01`, regra de copy). Até lá, usar um texto de placeholder claramente marcado.

### 11.2 O momento único da landing

Uma sequência CSS de **~2,4 s**, tocada **uma vez** no carregamento: o painel mostra `R$ 800,00`, a "luz" apaga (a camada preta sobe por opacidade) e volta, com o mesmo valor. Demonstra a promessa em vez de descrevê-la. Desligada por `prefers-reduced-motion` e por Modo Desempenho (mostra o estado final). **[DECISÃO PENDENTE V4]** aprovar o conceito.

### 11.3 Planos

Duas colunas (Gratuito e Pago) em **tabela de comparação** com os limites reais de `02`, seção 4 (lojas, administradores, cotas, janela de histórico, comissões). **Nada de três cartões iguais** e nada de recurso inventado. Valores de preço seguem **[DECISÃO PENDENTE]** do `01`.

### 11.4 Portal (conta)

Modo Balcão com cabeçalho Rua mínimo. Telas: plano, dispositivos (lista com `Revogar`), download, restauração. Mesma barra de ações, mesma voz.

- VIS-090: landing e portal respeitam o orçamento: sem fontes remotas, JS pequeno, imagens apenas quando agregam, e o fundo de pontos como SVG. A landing também é vitrine de desempenho (`01`).

---

## 12. Inventário de componentes (v1)

Cada componente nasce em `frontend/src/shared/ui/`, é **apresentacional puro** (`03`, seção 7) e usa só tokens.

| Componente | Especificação resumida |
|---|---|
| **Botão primário** | Fundo `azul-600`, texto branco, Corte-S, 48 px no caixa e 40 px nas demais. Pressionado: `azul-700` e deslocamento de 2 px por `transform`. Foco: ver 12.1 |
| **Botão secundário** | Fundo branco, contorno 2 px preto, texto preto |
| **Botão terciário** | Texto `azul-700` com sublinhado no foco |
| **Botão perigoso** | Fundo `vermelho`, texto branco. Só para ação destrutiva, sempre com confirmação |
| **Campo de texto** | Contorno 2 px `cinza-500` (ou preto no foco), Corte-S, rótulo **acima** e sempre visível. Erro: contorno `vermelho` **mais** texto de erro abaixo e ícone |
| **Campo de valor** | Número tabular alinhado à direita, `R$` fixo, máscara brasileira |
| **Tabela em fita** | Régua 1 px, cabeçalho em caixa de frase peso 600, números à direita, linha selecionada invertida, virtualizada |
| **Selo** | Seção 7.2 |
| **Marca de estado** | Seção 7.3 |
| **Painel** | Corte-M, contorno 2 px, cabeçalho com título 20 px |
| **Diálogo** | Painel Corte-M centralizado, fundo da página escurecido por camada sólida de 60% de preto (**sem blur**), foco preso |
| **Aviso em linha** | Faixa com selo à esquerda e texto à direita, sem ícone flutuante |
| **Confirmação temporária** | Selo de confirmação (7.2) com a animação descrita na seção 8 |
| **Contador de cota** | Texto `61 de 85` em 16 px peso 700 + barra **chapada** de 8 px com marcas a cada 10 unidades. Passa de 80%: barra amarela e selo de atenção. 100%: barra vermelha e selo de perigo |
| **Trilho de navegação** | Preto, 176 px, texto branco, seleção cobalto invertida |
| **Barra de estado** | 9.3 |
| **Tecla (atalho)** | Caixa 1 px com Corte-S em torno do nome da tecla, 13 px |
| **Entrada de PIN** | 4 a 6 caixas de dígitos, teclado numérico opcional na tela |

### 12.1 Foco visível

O contorno é cortado pelo `clip-path`. Portanto, o foco é desenhado **dentro** do corte: **inversão de cor** (cobalto com texto branco) **mais** barra de 4 px na base do componente. Deve manter razão de pelo menos **3:1** contra os vizinhos. **[VERIFICAR]** navegando com `Tab` em todos os componentes antes de fechar o design system.

---

## 13. Acessibilidade

Meta: **WCAG 2.2, nível AA**, nas telas operacionais.

- VIS-100: contraste conforme a seção 5.4. **Borda de campo usa `cinza-500` ou preto**, nunca `linha` (1,60:1).
- VIS-101: **cor nunca é o único portador de significado:** todo estado tem texto e forma (selo, sinal, ícone).
- VIS-102: navegação **completa por teclado**; foco sempre visível; ordem lógica; nenhum atalho que prenda o foco.
- VIS-103: tamanhos mínimos (13 px), alvos de clique de 40 px, suporte a escala de 125% e 150% do Windows.
- VIS-104: **gráficos** com paleta distinguível para daltonismo (diferença de luminosidade e rótulos diretos, não só matiz) e tabela de dados equivalente.
- VIS-105: suporte a **modo de alto contraste do sistema** (`forced-colors`): bordas e foco não podem desaparecer.
- VIS-106: texto do Letreiro e dos selos com alternativa de leitura de tela (7.5).
- VIS-107: moeda, datas e números no formato brasileiro (`R$ 1.234,56`, `28/10/2026`), com idioma `pt-BR` declarado.

---

## 14. Implementação (como isso entra no código)

### 14.1 Tokens

Os tokens vivem em `frontend/src/shared/design/tokens.css` como **variáveis CSS**, e os componentes só os usam (`03`, seção 7: "tokens trocáveis sem reescrever componentes").

```css
:root {
  --azul-600: #0040d8;
  --azul-700: #0030a8;
  --azul-300: #7da2ff;
  --azul-100: #dce6ff;
  --preto: #000000;
  --branco: #ffffff;
  --papel: #eef0f6;
  --cinza-500: #5e6377;
  --linha: #c9ccd8;
  --amarelo: #ffd21f;
  --vermelho: #c81e16;
  --verde: #07733c;
  --corte-s: 6px;
  --corte-m: 12px;
  --corte-l: 24px;
  --espaco-1: 4px;
  --fonte-display: "Mikella Display", "Archivo", "Arial Narrow", sans-serif;
  --fonte-texto: "Source Sans 3", "Segoe UI", "Noto Sans", sans-serif;
}
[data-tema="noite"] {
  --fundo: var(--preto);
  --painel: #141623;
  --texto: var(--branco);
  --destaque: var(--azul-300);
}
```

- VIS-110: nenhum valor de cor, tamanho de fonte ou espaçamento **hardcoded** fora de `tokens.css` (`00`, regra inviolável 4). Regra de lint (`stylelint`) proíbe `gradient`, `backdrop-filter`, `filter: blur`, `border-radius` diferente de `0` e cores literais fora dos tokens.
- VIS-111: **[VERIFICAR] risco de compatibilidade do Tailwind.** A versão 4 do Tailwind depende de recursos modernos de CSS e, segundo a documentação do projeto, de navegadores recentes; o WebKitGTK de distribuições Ubuntu 22.04 pode ser mais antigo. Antes de fixar a versão, testar no hardware de referência. Alternativas: Tailwind 3.4 ou CSS próprio com os tokens. Escrever CSS **conservador**: sem `color-mix()`, sem recursos sem suporte comprovado no WebKitGTK alvo.
- VIS-112: **uma página interna "galeria de componentes"** (rota só de desenvolvimento, fora do build de produção) mostra todos os componentes nos três modos e no Modo Desempenho. Dispensa Storybook e seu peso.
- VIS-113: **orçamentos no CI** (`07`, PERF-102): CSS total, JS inicial, peso de fontes, número de elementos com `clip-path` na galeria.
- VIS-114: o **Modo Desempenho** (`07`, PERF-064) é uma classe no `html` que desliga: transições, placa deslocada, halftone, Letreiro animado e qualquer sombra. O visual (cor, corte, tipografia) permanece.

### 14.2 Estrutura de pastas (extensão de `03`)

```
frontend/src/shared/
  design/
    tokens.css            # cores, espaços, fontes, cortes
    tema-dia.css
    tema-noite.css
    tema-rua.css
    fontes/               # woff2 com subconjunto latino
  ui/
    botao/  campo/  tabela-fita/  selo/  marca-estado/  painel/  dialogo/
    contador-cota/  barra-estado/  trilho-nav/  letreiro/
docs/design/
  ASSETS.md               # autoria e licença de cada ativo
  galeria.md              # decisões visuais aceitas
tools/contrast/           # script de validação de pares de cor
```

---

## 15. Marca (identidade)

O nome **Mikella** ainda precisa de busca de disponibilidade (INPI e domínios, `00`). Este documento define a direção; o desenho final do logotipo é trabalho de design.

- VIS-120: **wordmark** em Display condensado extra-pesado, com o **canto superior direito cortado** em uma letra (o corte do sistema). Testar legibilidade a **16 px** (favicon) e a **32 px** (ícone da bandeja do sistema).
- VIS-121: **ícone do aplicativo:** forma de **selo pentagonal** cobalto com a inicial em branco, dentro do quadrado exigido pelo sistema operacional. Variantes: cobalto e branco, branco sobre cobalto, preto sobre branco e **uma cor** (para impressão e cupom).
- VIS-122: **cupom e comprovante** (impressora térmica, `02`, RF-019) são monocromáticos: wordmark em preto, números em tabulares, régua de soma, sem imagens pesadas.
- VIS-123: **mascote:** não faz parte do MVP. **[DECISÃO PENDENTE V3]** Se for criado, deve ser **personagem original**, nunca derivado das referências.
- VIS-124: usar um **instalador e tela de abertura** em modo Rua, com o mesmo corte e o mesmo cobalto, em tela estática (sem animação de carregamento contínua).

---

## 16. Impacto nos documentos existentes (alterações sugeridas)

| Documento | Alteração |
|---|---|
| `00` | Remover "A identidade visual ainda não foi decidida". Incluir `08` no índice e na ordem de precedência. Adicionar ao glossário: **Letreiro**, **Selo**, **Corte**, **Modo Rua/Balcão**. No **MVP-0**, incluir o **spike visual**: tokens, fontes, tela de caixa com `clip-path` e bloco do total, medidos em hardware fraco |
| `01` | Em "Estrutura de conteúdo da landing", referenciar a seção 11 deste documento. Em "Regras de copy", adicionar a lista de proibições da seção 4 |
| `02` | Adicionar requisitos `UX-` ao conjunto de critérios de aceite (barra de estado, tela de retomada, contador de cota visível) |
| `03` | Seção 7: trocar "identidade visual ainda indefinida" por a referência a `08`. Adicionar pasta `shared/design/` e as regras de lint de estilo |
| `07` | **PERF-045:** famílias definidas (Archivo e Source Sans 3), orçamento de ~120 KB. **PERF-061:** explicitar que `clip-path` estático, contorno grosso e textura SVG estática são **permitidos**; gradientes, desfoque e sombras difusas continuam **proibidos**. **PERF-044:** registrar o risco da versão do Tailwind (VIS-111) |

---

## 17. Plano de validação

### 17.1 Spike visual (parte do MVP-0, em `07`, seção 3)

Construir a **tela de caixa real** com tokens, fontes, trilho, barra de estado, fita virtualizada de 5 mil linhas, bloco do total com `clip-path`, um selo e um diálogo, e medir em Lubuntu (i5 de 5ª geração, 6 GB DDR3-1333) e Windows 10 equivalente:

1. Quadros por segundo durante rolagem, troca de tela e entrada de painel.
2. RAM e CPU com a interface em uso e ociosa (`PERF-001`, `PERF-008`).
3. Custo de pintura de `clip-path` e de `box-shadow` sólido.
4. Renderização das fontes (nitidez em tamanho de 13 e 14 px, acentos, `R$`, `−`, algarismos tabulares).
5. Comparativo Tailwind 4 e alternativa (VIS-111).

**Reprovar** qualquer efeito que derrube o orçamento; ele sai do sistema (e a ordem é sempre: tipografia, cor, espaço e corte antes de efeitos).

### 17.2 Teste com pessoas

- 3 a 5 operadores de caixa e 2 donos de loja (perfil de `01`), em tarefa: abrir caixa, vender 5 itens com leitor, finalizar com pagamento misto, fazer sangria, fechar caixa, confirmar um alerta de validade.
- Métricas: tempo por tarefa, erros, necessidade de ajuda, teste de leitura do total a 1 metro. **Meta inicial a calibrar:** nenhuma tarefa precisa de explicação verbal.
- **Os números de meta de usabilidade são hipóteses.** Registrar o resultado, não o desejo.

### 17.3 Checklist de revisão de tela (para cada alteração de interface)

1. Passa nos 5 testes da seção 4.2?
2. Há um elemento dominante e uma ação primária?
3. Só usa tokens, as 5 formas e as 4 cores base?
4. Contrastes e tamanhos conferem com 5.4 e 6.4?
5. Operável só pelo teclado, com foco visível?
6. Números tabulares alinhados à direita, formato brasileiro?
7. Motion só de `transform`/`opacity`, desligável?
8. Funciona em 1024x600 e com escala de 125%?
9. Copy no padrão da seção 9.5, sem emoji, sem número inventado?
10. Nada copiado das referências (seção 2.7)?

---

## 18. Decisões em aberto

| # | Pergunta | Recomendação |
|---|---|---|
| V1 | Confirmar Archivo e Source Sans 3 após verificação de licença, tabulares, glifos e peso (VIS-033) | Adotar após o spike. Reservas: Big Shoulders Display e Barlow Condensed (display), IBM Plex Sans (texto) |
| V2 | Modo padrão: Dia ou Noite? | **Dia** como padrão. Noite opcional. Medir fadiga em uso prolongado com operadores |
| V3 | Mascote ou personagem da marca | Fora do MVP. Se existir, original |
| V4 | Aprovar o momento único da landing ("A luz caiu") | Aprovar só quando o teste de queda de energia (`06`, seção 8) existir |
| V5 | Busca global (`Ctrl+K`) no MVP | Deixar para depois do MVP, a menos que o teste de navegação mostre pessoas perdidas |
| V6 | Mapeamento de atalhos do caixa | Validar com operadores reais antes de fixar |
| V7 | Letreiro: só na landing ou também em estados vazios e na ativação | Landing e ativação no MVP; estados vazios depois |
| V8 | Halftone: manter ou cortar | Manter só na landing e na ativação; cortar se pesar |
| V9 | Conjunto de ícones: adaptar um livre ou desenhar do zero | Adaptar um conjunto livre no MVP; desenhar do zero se a marca exigir |
| V10 | Registrar a marca e o logotipo (INPI) | Pesquisar antes de investir (`00`) |

---

## 19. O que não foi comprovado

- Nenhuma fonte, cor ou efeito foi testado em hardware fraco.
- O azul `#0040D8` é uma **amostra aproximada** da imagem; calibrar em monitor.
- A licença, os algarismos tabulares e o peso real de Archivo e Source Sans 3 **não foram verificados nos repositórios oficiais**; as afirmações vêm de guias de terceiros e do conhecimento geral. Verificar (VIS-033).
- O custo de `clip-path` e de contornos duplos no WebKitGTK sem GPU **é hipótese**.
- A compatibilidade do Tailwind 4 com o WebKitGTK alvo **não foi verificada**.
- Nenhuma tela foi testada com usuários.
- As informações sobre o design de Persona 3 e 5 vêm de matérias de imprensa e análises de terceiros, algumas baseadas em entrevistas traduzidas; não foi possível consultar as entrevistas originais.

---

## Fontes consultadas

- F1. Automaton Media, "Persona 3 Reload's viral 'sexy UI': what ideas went into its design?": https://automaton-media.com/en/news/20240207-26873/
- F2. Push Square, sobre a equipe de UI da Atlus e a entrevista do diretor de Persona 5: https://pushsquare.com/news/2024/02/persona-3-reloads-sexy-ui-result-of-retaining-key-staff-says-atlus
- F2b. GamesRadar, mesma entrevista: https://www.gamesradar.com/persona-3-reloads-ui-goes-hard-and-one-indie-dev-reckons-theyve-figured-out-why
- F2c. One More Game, entrevista sobre Persona 3 Reload no Switch 2 (a UI como parte da atmosfera): https://onemoregame.ph/2025/10/persona-3-reload-switch-2-interview/
- F3. Siliconera, sobre os princípios da UI de Persona 5: https://www.siliconera.com/?p=668266
- F4. Inverse, análise da UI de Persona 5 Tactica e da legibilidade: https://inverse.com/gaming/persona-5-tactica-user-interface
- F5. Font Alternatives, fontes para painéis densos e algarismos legíveis: https://fontalternatives.com/blog/best-fonts-dense-dashboards/ e https://fontalternatives.com/blog/best-fonts-legible-numerals-ui/
- F6. Made Good Designs, sobre a tipografia de Persona 5 (e recomendação de criar arte original): https://madegooddesigns.com/persona-5-font/
- F7. Robin Rendle, "Video games and WiggleTech", sobre a tipografia de Persona 5: https://robinrendle.com/adventures/video-games-and-wiggletech/
