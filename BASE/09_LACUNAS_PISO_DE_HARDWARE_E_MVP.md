# Mikella ERP — lacunas, novo piso de hardware e corte de MVP

> **Nota de atualização (09/11/2026):** documento histórico de revisão. Suas
> decisões de piso (i3/i5 de 5ª geração, 6 GB, HDD 5400 rpm) foram adotadas e
> o produto já está implementado. O estado vigente está em
> `PLANO E FASES/00_PLANO_MESTRE.md`.

**Criado em:** 28/10/2026
**Status:** documento histórico de revisão (28/10/2026). Na época, **nada estava implementado nem medido**. Hoje o produto já existe; o estado vigente está em `PLANO E FASES/00_PLANO_MESTRE.md`. Todo número de desempenho, custo e prazo aqui é **meta ou hipótese**. Itens **[DECISÃO PENDENTE]** pedem resposta do dono do produto; itens **[VERIFICAR]** vêm de conhecimento geral ou de fonte secundária e precisam de confirmação antes de virarem promessa.
**IDs novos:** `HW-` (hardware e ambiente), `ARQ-`, `PROD-`, `REC-` (recuperação de conta), `OPS-` (operação e suporte), `NEG-` (negócio), `MVP-`. Decisões novas continuam a numeração do `00` (P12 em diante).

## 0. Como ler este documento

1. O **novo requisito mínimo de hardware** (seção 1) é tratado como **aprovado** pelo dono do produto. Ele **substitui** o hardware de referência do `07` (i5 de 2ª geração e 4 GB), que passa a ser **alvo de resistência, sem garantia** (decisão H1, seção 12).
2. As lacunas que levantei antes (seção 4) foram **ampliadas** com o que um novo olhar sobre os 9 documentos revelou (seções 2, 3 e 5).
3. A seção 8 propõe um **corte de MVP** e uma **ordem de fases revisada**. É uma recomendação, e muda o `00`.
4. Precedência sugerida: `04`/`05` > `06` > `03` > `02` > `07` > `08` > **`09`**. Onde este documento muda um número do `07`, vale o `09` **depois da aprovação** da seção 12.

### 0.1 Correção de uma afirmação anterior

Eu havia dito, e o `07` registra ("Fora do alvo: Windows 7"), que o Tauri 2 não roda em Windows 7. A **documentação oficial de pré-requisitos do Tauri 2 lista "Windows 7 e posterior"** (F1). Então **Windows 7 não está tecnicamente excluído pelo Tauri**. Há outros obstáculos reais (WebView2, toolchain, fim de suporte do SO), tratados em HW-092. A decisão de suportar ou não Windows 7 é uma escolha de produto (H2), não uma impossibilidade comprovada.

---

## 1. Novo requisito mínimo de hardware

### 1.1 O piso

| Item | Piso de projeto |
|---|---|
| Processador | Intel Core **i3 ou i5 de 5ª geração** (Broadwell). Referência: **i3-5005U** (2 núcleos e 4 threads, 2,0 GHz, **sem Turbo Boost**, com AES-NI e AVX2, vídeo HD Graphics 5500; F4) |
| Memória | **6 GB DDR3 a 1333 MHz** |
| Armazenamento | **HDD de 5400 rpm como pior caso** (hipótese conservadora) e SSD SATA como configuração recomendada |
| Tela | 1366x768 (e 1024x768) |
| Arquitetura | **64 bits** |
| Sistemas | Windows 10 e Lubuntu/Linux Mint/Linux Lite (Ubuntu 22.04 ou superior, por causa do WebKitGTK 4.1; F1). Windows 7: ver H2 |

**Ordem de grandeza da memória:** DDR3-1333 em dois canais tem cerca de 21 GB/s teóricos (cálculo: 1333 MT/s x 8 bytes x 2). Para um sistema de caixa, a **banda não é o gargalo; a capacidade e o disco são** (hipótese a medir).

### 1.2 O que muda em relação ao `07`

| Aspecto | Referência antiga (`07`) | Novo piso | Consequência |
|---|---|---|---|
| RAM | 4 GB | **6 GB** | Mais folga. Os orçamentos de RAM podem subir um pouco, mas o sistema operacional e o antivírus continuam comendo a maior parte (seção 1.4) |
| CPU | i5 de 2ª geração (desktop, normalmente com Turbo) | i3/i5 de 5ª geração **de notebook** (baixa potência, sem Turbo no i3-5005U) | **O núcleo pode ser mais lento por thread, não mais rápido.** A CPU do i3-5005U tem pontuação baixa em comparativos (cerca de 2.000 pontos no PassMark segundo F4). Provavelmente é comparável ou inferior, por núcleo, a um i5 desktop de 2ª geração **[VERIFICAR com medição]**. O piso é **mais fácil na RAM e igual ou mais difícil na CPU** |
| Formato | Desktop | **Provavelmente notebook** (a família "U" é soldada na placa; F4) | Bateria vira um nobreak improvisado (HW-070), e há risco de bateria viciada ou inchada |
| Sistema | Windows 10+ ou Linux | Windows 10 (que o 5ª geração costuma manter) e Linux | Windows 10 **já saiu do suporte regular** (HW-090) |

**Regra de projeto:** o trabalho de desempenho do `07` continua válido; a prioridade passa a ser **thread principal da interface e latência de disco**, não só memória.

### 1.3 Definição de "funciona normal sem travar" (critério verificável)

"Não travar" vira um conjunto de metas medidas **no hardware de referência**, com o piso em HDD e com estresse realista de fundo (antivírus escaneando, navegador do operador aberto com várias abas).

| ID | Critério | Meta inicial (hipótese) |
|---|---|---|
| HW-001 | Congelamento da interface durante operação de caixa | **Nenhuma tarefa longa acima de 100 ms** na thread principal (p99), em venda, busca e rolagem |
| HW-002 | Bipar item até a linha na lista | ≤ 100 ms (p95), igual ao PERF-005 |
| HW-003 | Finalizar venda com gravação confirmada | ≤ 300 ms (p95) em **SSD**; ≤ 600 ms (p95) em **HDD 5400 rpm** (o `fsync` do disco mecânico é o limite) |
| HW-004 | Abertura a frio até caixa utilizável | ≤ 4 s em SSD; ≤ 6 s em HDD |
| HW-005 | Memória total do Mikella no PC servidor (servidor, interface e todos os processos do WebView) | ≤ ~450 MB. No PC cliente: ≤ ~300 MB |
| HW-006 | Folga de memória do sistema com Mikella e um navegador com ~10 abas abertos | **≥ 1 GB livres**, sem troca de página (swap) contínua |
| HW-007 | CPU média com 8 caixas simulados em horário de pico | ≤ ~35% da capacidade (4 threads) |
| HW-008 | CPU ociosa | ≈ 0% a 1% |
| HW-009 | Crescimento de memória em 8 horas de uso simulado | ≤ 5% (sem vazamento) |

**Importante:** o `07` mede o WebView como "o app". No Windows, o WebView2 roda em **vários processos** (navegador, renderizador, GPU, utilitários). O número que importa é a **soma de todos os processos filhos**, em memória privada. Os scripts de benchmark (`07`, PERF-100) medem a soma.

### 1.4 Orçamento de memória em 6 GB (hipótese, a medir)

| Consumidor | Ordem de grandeza (hipótese) |
|---|---|
| Sistema operacional Windows 10 ocioso | ~2,0 a 2,5 GB (Lubuntu: ~0,8 a 1,2 GB) |
| Antivírus e serviços | ~0,3 a 0,5 GB |
| Navegador do operador (WhatsApp Web, banco, painel da maquininha) | ~0,7 a 1,2 GB |
| **Mikella (servidor + interface + WebView)** | **≤ ~0,45 GB** |
| **Folga** | **≥ 1 GB** |

Os números acima são ordens de grandeza **a medir no laboratório**, não medições.

---

## 2. Gargalos reais de PC antigo (novos requisitos de ambiente)

Esta seção reúne problemas que a lista de falhas do `06` não cobre e que são **comuns** em PC velho de comércio.

### 2.1 Disco e `fsync`

- HW-010: o instalador executa um **teste de disco**: grava e sincroniza algumas centenas de pequenos blocos, mede o tempo de `fsync` e **classifica** o disco (rápido, lento, suspeito). Resultado alimenta o painel de saúde (`03`, seção 9).
- HW-011: disco mecânico lento com `synchronous=FULL` limita a taxa de gravações. Mitigação: **transações pequenas e curtas**, **escritor único**, nada de leituras pesadas dentro de transação de escrita, e relatórios fora do caminho da venda. Medir a taxa máxima de vendas por segundo em HDD com 8 caixas (**hipótese:** suficiente para varejo pequeno).
- HW-012: **disco que mente sobre `fsync`** (tempo de gravação irrealisticamente baixo para HDD) gera **aviso ao lojista** (`06`, seção 2).
- HW-013: o instalador recomenda **SSD SATA** com texto objetivo: "É a troca que mais melhora a velocidade do caixa em computador antigo". Recomendação, não exigência. Nenhuma alegação de preço.

### 2.2 Antivírus e sistema operacional

- HW-020: o antivírus pode varrer o arquivo do banco e o WAL e gerar travadas. O manual de instalação **explica a opção de excluir a pasta de dados da varredura**, com o aviso do trade-off de segurança (o banco é cifrado, `04`, SEG-040). **O instalador nunca altera exclusões do antivírus sem consentimento explícito.**
- HW-021: **atualização forçada e suspensão.** O Windows pode reiniciar sozinho em horário de venda. O instalador orienta (e, com consentimento, configura) **horas ativas**, desativa **suspensão e hibernação** do servidor e instala o servidor para **iniciar com o sistema**. Em Linux, janela de reinício para atualizações automáticas. O reinício **não pode perder o caixa** (RES-011), e o teste de reinício forçado entra na lista de resiliência.
- HW-022: **memória baixa.** Em Linux, documentar `zram` ou arquivo de troca; em Windows, o arquivo de paginação padrão. **[VERIFICAR]** o efeito no hardware de referência.

### 2.3 Relógio do computador (bateria da placa-mãe)

PC antigo com bateria CMOS fraca **volta com data errada** (ano antigo) depois de falta de energia. Isso afeta o relógio confiável (`05`, FRD-005), o período das cotas (FRD-021), a validade de certificados TLS e a data das vendas.

- HW-030: ao iniciar, se o relógio estiver **antes da data de compilação do app** ou **antes do último horário confiável conhecido**, o sistema mostra a tela **"A data deste computador parece errada"**, explica o motivo provável (bateria da placa) e orienta a corrigir.
- HW-031: **não tratar como fraude automaticamente.** Registrar o evento, usar o tempo derivado (último confiável mais relógio monotônico) e permitir que **dono ou gerente confirmem a data correta com PIN de aprovador**. Venda com relógio incerto fica **marcada**. Reportar à nuvem na próxima conexão.
- HW-032: TLS para a nuvem pode falhar por relógio errado; a mensagem de erro aponta a data como causa provável.

### 2.4 Rede local de comércio

- HW-040: roteadores baratos trocam o IP do servidor. O servidor é encontrado por **nome** (descoberta por mDNS ou equivalente **[VERIFICAR]**), com reserva de IP recomendada, e o certificado TLS cobre o **nome**, não um IP fixo (`04`, SEG-031).
- HW-041: o instalador cria a **regra de firewall** do servidor com consentimento (o Windows e distribuições Linux bloqueiam a porta por padrão).
- HW-042: caixas em **Wi-Fi** são suportados, mas o painel de saúde indica qualidade da conexão. Testes de resiliência incluem **perda de pacotes e quedas curtas** (`06`, seção 8, itens 3 e 4).

### 2.4.1 Instalação

- HW-050: **WebView2.** Segundo a documentação do Tauri 2, o Windows 10 (versão 1803 e posteriores) já inclui o WebView2 (F1). O instalador padrão baixa o componente na hora, e para **lojas sem internet** existe o modo de instalador **offline**, que embute o runtime e **soma cerca de 127 MB** ao arquivo, e o modo de versão fixa, com cerca de 180 MB (F2). Decisão: publicar **dois instaladores**, "leve" (PERF-007 mantido) e "completo/offline". Registrar a exceção no `07`.
- HW-051: **permissões de instalação.** Serviço e regra de firewall exigem **administrador**. Definir o que acontece com instalação sem privilégio (somente modo usuário, sem serviço). **[DECISÃO PENDENTE]**.

### 2.5 Notebook como servidor: a bateria é um nobreak (e um risco)

- HW-070: se o PC servidor é notebook, a **bateria cobre quedas curtas de energia**. O painel de saúde mostra **"Rodando na bateria"** e a autonomia estimada, e a interface do caixa mostra um estado discreto.
- HW-071: em **bateria crítica** (limite parametrizável), o servidor **para de aceitar novas vendas, faz checkpoint e sincroniza o disco** antes de desligar. **A sessão do caixa não é encerrada** (RES-012): apenas as gravações são pausadas com aviso claro. **[VERIFICAR]** como obter o estado da bateria em Windows e Linux de forma confiável.
- HW-072: bateria viciada (dura segundos) é comum em notebook velho. O painel de saúde alerta se a energia cai e o computador desliga, e a recomendação de **nobreak** do `06` continua valendo.

### 2.6 Compilação e criptografia

- HW-080: compilar para **x86-64 de linha de base**: **sem** `target-cpu=native` e **sem exigir AVX2** (o piso novo tem AVX2, mas a referência antiga, i5 de 2ª geração, **não** tem **[VERIFICAR]**). Permite rodar nas duas.
- HW-081: AES-NI existe no i3-5005U (F4), mas há processadores de entrada sem AES-NI. O sistema **detecta AES-NI em tempo de execução** e, sem ele, usa **XChaCha20-Poly1305** onde a biblioteca permitir. Confirmar o custo do SQLCipher no spike (`04`, SEG-040).
- HW-082: **somente 64 bits.** Com 6 GB de RAM, 32 bits não faz sentido.

### 2.7 Sistemas operacionais (fatos e consequências)

- HW-090: o **suporte regular do Windows 10 terminou em 14/10/2025**. A Microsoft estendeu as **atualizações de segurança estendidas (ESU)** até **12/10/2027** para quem se inscrever, com condições (Windows 10 22H2, atualizações em dia, conta Microsoft e as opções de inscrição descritas na matéria; F3). **[VERIFICAR]** a regra atual na página da Microsoft. Computadores de 5ª geração **provavelmente não são elegíveis ao Windows 11** (a exigência oficial de processador é mais recente **[VERIFICAR]**), então parte da base instalada fica em Windows 10 sem garantia de atualização depois de 2027. Consequências: (a) risco de segurança do sistema operacional é **do ambiente**, mas o produto **mitiga** o que pode (assinatura, TLS, cifra, atualização assinada, `04`); (b) **Linux (Lubuntu) deve ser caminho de primeira classe** no manual, não apenas "compatível".
- HW-091: instalação em Linux e em Windows têm **a mesma suíte de testes de resiliência**, no hardware de referência.
- HW-092: **Windows 7 proibido.** O Tauri lista "Windows 7 e posterior" (F1), mas: WebView2 deixou de receber versões novas em Windows 7 (última compatível tem falhas de segurança conhecidas), a cadeia de ferramentas do Rust rebaixou o alvo a um nível sem suporte oficial, e o sistema perdeu atualizações e certificados raiz TLS desatualizados que podem quebrar a conexão com a nuvem. **Decisão:** **Windows 7 é proibido no MVP e fora de escopo.** A não ser que o piloto revele demanda real inadiável, o suporte a Windows 7 **não será adicionado**. Se for considerado depois do piloto, exigiria: (a) alvo de compilação separado, (b) versão fixa do WebView2 (cerca de 180 MB, F2) e (c) teste em máquina real.

---

## 3. Novas ideias de arquitetura que reduzem risco e complexidade

### 3.1 Modo "loja com 1 computador" como padrão (ARQ-001)

O desenho dos documentos parte de um **servidor local mais clientes pela rede**, com CA própria, TLS e pareamento. Mas a maioria dos comércios pequenos tem **1 ou 2 caixas**. Para eles, esse aparato é custo e risco sem benefício.

- ARQ-001: o modo **padrão** é **1 computador**: o servidor e a interface rodam **no mesmo processo**, sem rede e sem TLS, com a interface chamando os casos de uso **diretamente** (comunicação interna do Tauri, sem HTTP). Menos processos, menos RAM, menos superfície de ataque, menos pontos de falha, instalação em 1 clique.
- ARQ-002: ao **adicionar o segundo computador**, o assistente "Ativar rede da loja" liga a API local, gera a CA, cria o certificado, pede a regra de firewall e abre o pareamento. É **evolução**, não instalação separada.
- ARQ-003: o `03` já isola casos de uso atrás de portas, então o mesmo núcleo atende os dois modos: um adaptador **em processo** e outro **HTTP**. Esse recorte também reduz o escopo do MVP (seção 8).
- ARQ-004: no modo de 1 computador, a autenticação de usuários e a verificação de capacidade **continuam existindo** (`04`, SEG-020). O que sai é a rede.

### 3.2 Contingência quando o servidor cai (multi-PC) [DECISÃO PENDENTE P12]

Hoje, se o PC servidor trava no meio do expediente, **todos os caixas param**: o rascunho persiste (RES-020), mas não é possível **finalizar** a venda sem o servidor. Para uma loja com 3 caixas e 1 servidor velho, isso é o pior cenário.

- ARQ-010: opção **"venda em contingência"**: cada caixa mantém um **diário local append-only** (pequeno SQLite, `synchronous=FULL`) das suas movimentações de sessão, com chaves de idempotência, e usa o **catálogo em cache** (PERF-091). Com o servidor fora, o caixa **continua vendendo** com limitações explícitas (somente venda à vista; sem cancelamento de venda antiga; sem desconto acima do limite; sem comissão por comanda), imprime comprovante marcado "sincronização pendente" e **sincroniza ao voltar**.
- ARQ-011: o diário do caixa **não é um segundo banco oficial**: é uma fila durável de eventos de **uma sessão** até o servidor confirmar (compatível com D1 e D2). Risco: estoque pode divergir (duas vendas do último item); aceitar, sinalizar e resolver na conciliação.
- **Recomendação:** **fora do MVP**. Entra depois do piloto **se os dados mostrarem quedas do servidor** em lojas com mais de 1 caixa. Mesmo assim, o desenho do diário já deve ser compatível com o modelo de eventos do `06` (livro por sessão).

---

## 4. Lacunas levantadas anteriormente, agora como requisitos

### 4.1 Produto

- **PROD-001 Primeira execução.** Assistente guiado: dados da loja, primeiro produto, primeira venda de teste. **Meta (hipótese): primeira venda real em até 15 minutos após a instalação.**
- **PROD-002 Modo treinamento.** Vendas de teste que **não entram no livro nem no estoque** (banco de ensaio descartável), com faixa visível "Treinamento". Útil para treinar operador novo sem risco.
- **PROD-010 Importação de planilha de produtos** (CSV; XLSX se o custo de peso for aceitável). Mapeamento de colunas, **prévia**, erros por linha, importação **idempotente** (repetir não duplica). É o principal ponto de adoção para quem migra de outro sistema.
- **PROD-011 (futuro) Preenchimento por código de barras**, consultando base pública de EAN. Exige internet e provedor, com revisão de LGPD e de custo. Fora do MVP.
- **PROD-020 Promoções e regras de preço.** O alerta de validade sugere "fazer promoção" (RF-023), mas **não existe o módulo para criá-la**. Requisito: promoção por produto, lote ou categoria, com período, preço promocional ou percentual, **criada a partir do alerta de validade com o lote já preenchido**, e **calculada no servidor**. Estratégia de preço é um *Strategy* (`03`, seção 4). O caixa mostra o selo `PROMO` (`08`, seção 7.2). Definir se comissão incide no preço promocional (parâmetro).
- **PROD-021 Fiado e crediário.** Cliente com limite, saldo devedor, recebimento parcial, comprovante. Integra contas a receber (RF-030). Ultrapassar o limite exige PIN de aprovador. Dados mínimos de cliente (LGPD, `04`, seção 10).
- **PROD-022 Devolução e troca.** Fluxo próprio ligado à venda original, com estorno de estoque, caixa e comissão (FRD-065), prazo parametrizável e permissão própria.
- **PROD-023 Produto por peso.** Preço por quilo e leitura de **código de balança etiquetadora** (EAN-13 com preço ou peso embutido) **[VERIFICAR padrão e prefixos]**. Integração direta com balança serial fica para depois.
- **PROD-024 Etiquetas.** Impressão de etiquetas de preço e de código de barras em impressora térmica, e geração de **código interno** para produto sem EAN.
- **PROD-025 Pix estático (opcional).** Exibir na tela do caixa um QR de Pix **gerado localmente** a partir da chave do lojista, com o valor preenchido. Funciona sem internet e **não** confirma pagamento (o registro continua manual, RF-011); a interface avisa que o operador deve conferir o recebimento no aplicativo do banco. **[VERIFICAR]** a especificação do BR Code estático.
- **PROD-030 Papel do contador.** Papel **somente leitura**, com capacidade `export.accounting`, sem acesso a dados pessoais de clientes além do necessário, e **pacote de fechamento do mês** (vendas, contas a pagar e a receber, XMLs de entrada) em CSV. **[DECISÃO PENDENTE P17]** formatos que os contadores da região realmente pedem.
- **PROD-040 Multi-loja (Pago).** O `00` (P7) só pergunta como consolidar sem a nuvem ler os dados. Três opções: (1) lojas **independentes** com relatório consolidado por **exportação** no PC do dono; (2) métricas agregadas **opt-in**; (3) **sincronização cifrada ponta a ponta entre as lojas** com chave de conta (continua zero-knowledge para o Mikella), que também permitiria transferência de estoque. **Recomendação:** (1) no MVP, (3) depois. Transferência de estoque entre lojas fica para a fase de multi-loja. **[DECISÃO PENDENTE P16]**.
- **PROD-050 Notificações sem conteúdo.** Se ninguém está no computador, quem recebe o alerta de validade? A nuvem **não pode ler** o conteúdo da loja. Solução: o servidor envia à nuvem só um **sinal** ("há 3 alertas pendentes"), e a nuvem manda um e-mail **sem nomes de produtos**. Revisar com a LGPD (`04`, seção 10).
- **PROD-060 (futuro) Visão do dono no celular.** Muito pedida no mercado, mas **conflita com local-first e zero-knowledge**. Só pode nascer sobre métricas agregadas opt-in (PROD-040, opção 2). **Fora do escopo agora**, registrado como ideia.

### 4.2 Fiscal: o maior risco comercial

Sem emissão fiscal de saída no MVP (D11), o produto só serve a quem **não precisa emitir** ou **emite em outro sistema**. Pelo que a fonte consultada informa (F5): a NFC-e é o documento fiscal do varejo para consumidor final; a **obrigatoriedade varia por estado e por tipo de contribuinte**; o MEI **pode** emitir, mas a obrigação depende da regra local; e existe **contingência** para emitir quando não há conexão, com transmissão posterior. A regra exata e os prazos **dependem da SEFAZ da UF e do contador** (**[VERIFICAR]**).

- NEG-001: **segmento inicial declarado.** Definir quais lojas o MVP atende e dizer isso na landing (`01`).
- NEG-002: **estratégia fiscal em três degraus.**
  - **(a)** Agora: gestão interna e **comprovante não fiscal**, para quem emite fiscal em outro emissor ou é dispensado.
  - **(b)** Depois do piloto: **módulo opcional de emissão via serviço de terceiros** (API), mantendo o núcleo local-first. Exige internet na emissão e tratamento de contingência.
  - **(c)** Emissão própria (certificado digital A1, XML, assinatura, contingência): custo e risco altos. Só com demanda comprovada.
- NEG-003: o risco de **dupla digitação** (vender no Mikella e emitir em outro sistema) é uma barreira de adoção que o piloto precisa medir.
- **Recomendação:** (a) no MVP, validar a necessidade de (b) com 5 lojistas e 1 contador **antes** de investir. **[DECISÃO PENDENTE N2]**.

### 4.3 Recuperação de conta e sucessão do dono

A conta depende de **e-mail**, **senha**, **2FA por e-mail**, **PIN** e **frase de recuperação do backup**. Falta o que acontece quando algo se perde.

- REC-001: **perdeu acesso ao e-mail.** Processo **manual** com prova de titularidade (documento e dados do cadastro), **prazo de espera** com aviso ao e-mail antigo, e registro. Sem atalho por suporte "na conversa".
- REC-002: **troca de e-mail** exige 2FA do e-mail antigo **e** do novo, e revoga sessões (`04`, SEG-008).
- REC-003: **perdeu a frase de recuperação.** O banco **local continua funcionando**; só o backup na nuvem é irrecuperável (P3). O dono com acesso ao servidor pode **gerar nova frase** (RES-044).
- REC-004: **troca de dono** (venda da loja, saída de sócio, falecimento): fluxo formal, com os dois lados autenticados quando possível, **reembrulho das chaves**, histórico preservado, e checagem da regra "um documento, uma conta gratuita" (FRD-014). Em falecimento ou ausência, **processo humano com documentos**, nunca automático.
- REC-005: **segundo fator alternativo opcional** (aplicativo autenticador) **e códigos de recuperação de 2FA** gerados na criação da conta, para não depender só do e-mail. **[DECISÃO PENDENTE P19]**.
- REC-006: **Kit de recuperação impresso.** Hoje o lojista precisa guardar: senha, PIN, códigos de 2FA e frase de backup, e vai confundir. Propor **uma única folha** (PDF para imprimir) na ativação, com a frase e os códigos e uma explicação de uma linha para cada, **sem a senha**. A tela de `08`, seção 10.6, passa a oferecer o kit.

### 4.4 Cotas: o que a regra de edição realmente protege

O `00` deixa em aberto as cotas de **edição e exclusão** (P6). A razão técnica de existirem é uma só: impedir que o usuário **recicle uma vaga de produto** editando um produto antigo para virar outro (burlando a cota de criação).

- PROD-100: **recomendação de P6.** A cota de edição conta **alterações de identidade** do produto (código de barras, nome, categoria, unidade). **Não conta** preço, custo, estoque mínimo, promoção nem foto, que são operação diária. **Arquivar** e **reativar** contam como criação (D8). Os valores numéricos vêm dos entitlements (D9). **[DECISÃO PENDENTE P6, com recomendação]**.
- PROD-101: **importação e cota (conflito real).** Um lojista que migra tem centenas de produtos. Opções:
  - **A.** A importação conta na cota mensal: honesta, mas inviabiliza a migração (2.000 produtos em 85 por mês levam dois anos).
  - **B.** **Carga inicial** única por loja, com **teto parametrizável** e janela após a ativação; depois disso, tudo conta. A carga inicial é registrada na cadeia de cotas e na nuvem (`05`, FRD-022), então **restaurar backup não permite repetir**.
  - **C.** Importação ilimitada: vira a brecha óbvia da cota.
  - **Recomendação:** **B**. O teto e a janela são **decisão de negócio**, porque o dono quer que quem tem muitos produtos pague. **[DECISÃO PENDENTE N1]**.

### 4.5 Operação e suporte

- OPS-001: botão **"Gerar pacote de suporte"**: arquivo com versão, sistema, hardware, métricas de desempenho, últimos erros (por ID de correlação), resultado do teste de disco e integridade. **Sem PII, sem dados de venda.** O dono vê o conteúdo antes de enviar.
- OPS-002: **relatório de falhas opcional** (opt-in), sem PII (`04`, SEG-082).
- OPS-003: **modo suporte com consentimento do dono:** acesso temporário e limitado, com prazo, registrado na auditoria. O suporte **nunca** pede senha, PIN nem frase de recuperação. Ferramenta de acesso remoto fica fora do MVP.
- OPS-010: **atualização segura em campo:** canais estável e beta; liberação em ondas; **servidor primeiro** (`03`, seção 8); janela fora do horário de venda; **cópia do banco antes da migração**; **retorno automático à versão anterior** se a verificação de saúde falhar; versão mínima exigida só por **aviso**, nunca por bloqueio no meio do expediente; atualização **offline por arquivo**.
- OPS-020: **distribuição:** baixar **apenas do domínio oficial**, com hash publicado, para combater instaladores falsos. **Assinatura de código** no Windows: sem ela, o sistema mostra avisos de segurança que prejudicam a instalação. **[VERIFICAR]** custo do certificado e o tempo para ganhar reputação.
- OPS-030: **página de status** da nuvem e comunicação de incidente, com a frase clara de que **a nuvem fora do ar não para as vendas** (RES-060).
- OPS-040: **ajuda em português dentro do aplicativo** e guia de instalação ilustrado, incluindo "o que fazer se a luz cair" e "como guardar a frase".
- OPS-050: **métricas de produto agregadas, opt-in:** tempo até a primeira venda, abandono do assistente, versões em uso. Sem PII.

### 4.6 Negócio

- NEG-010: **custo da nuvem do plano Gratuito.** Cada conta gratuita guarda backup para sempre. Definir **teto de armazenamento por conta**, retenção de versões (RES-032), compressão e **política para contas inativas** (aviso e prazo antes de apagar a **cópia na nuvem**, nunca o banco local, P2). Estimar o custo por conta **com medição do tamanho real do backup** (ainda não existe). **[DECISÃO PENDENTE N3]**.
- NEG-011: **preço e cobrança do Pago:** valor, plano mensal ou anual, Pix e boleto recorrentes, período de teste. **[DECISÃO PENDENTE N4]**. O próprio Mikella precisa de **CNPJ**, emissão de nota de **serviço** e contador.
- NEG-020: **termos de uso e responsabilidade.** A promessa "não perde o caixa" precisa de **cláusula de limite de responsabilidade**, de linguagem sobre nobreak e disco e de revisão por advogado (`04`, seção 10.3).
- NEG-030: **validação de mercado antes de construir tudo:** 10 entrevistas com lojistas e piloto de 3 lojas (seção 8). Testar primeiro a **suposição mais arriscada**: o lojista aceita trocar de sistema, convive com a questão fiscal e paga pelo Pago?
- NEG-040: **mapa de alternativas** (o Bling foi citado como referência de excesso, e há PDVs gratuitos e desktop). Fazer a pesquisa **antes** de posicionar. Nenhuma comparação pública sem afirmação verificada (`01`).
- NEG-050: **equipe e esforço** não estão estimados. Produzir estimativa **depois do spike** (seção 8). O escopo atual (Rust, Tauri, nuvem, cobrança, backup cifrado, comissão, multi-loja) é grande para equipe pequena.

---

## 5. Decisões anteriores revisitadas

| Item | Revisão |
|---|---|
| **D6** (servidor também opera caixa) | Com o piso de 6 GB, fica viável. No modo de 1 computador (ARQ-001) deixa de ser uma decisão: é o único PC. Em multi-PC, medir no spike o custo de servidor + interface + antivírus em i3 |
| **P5** (2FA no desktop) | Acrescentar relógio errado (HW-030) e a possibilidade de segundo fator alternativo (REC-005). Mantém a janela offline |
| **P9** (8 computadores por loja) | No Pago, 8 lojas x 8 PCs = 64 máquinas por conta. Decidir se há **teto por conta** além do teto por loja. **[DECISÃO PENDENTE P14]** |
| **P3** (frase de recuperação) | Reduzir o atrito com o **Kit de recuperação impresso** (REC-006). Recomendação de P3 mantida |
| **P10** (histórico selado) | Sem mudança, mas a decisão deve sair **antes** de qualquer lançamento do plano Gratuito com histórico limitado |
| **PERF-007** (tamanho do instalador) | Dois instaladores: leve e completo/offline (HW-050) |
| **Windows 7** | **Proibido.** WebView2 obsoleto, cadeia de ferramentas sem suporte, certificados TLS desatualizados. Decisão de produto (HW-092). Fora do MVP e fora de escopo. |

---

## 6. Novos testes obrigatórios

Somam-se aos testes de `06` (seção 8), `05` (seção 9) e `07` (seção 9). Rodam no **hardware de referência** (seção 7).

1. **Reinício forçado** do Windows no meio da venda: o caixa volta com o valor correto.
2. **Corte de energia repetido (mínimo de 100 ciclos)** em **HDD 5400 rpm**, com `synchronous=FULL`: nenhuma venda parcial, saldo confere. (O `fsync` de disco mecânico é o ponto mais frágil.)
3. **Relógio errado** (data de anos atrás e data futura): a tela guiada aparece, o evento é registrado, nada é apagado.
4. **Bateria crítica** em notebook: as gravações pausam com aviso, o disco é sincronizado e a sessão não é encerrada.
5. **Antivírus varrendo** a pasta de dados durante venda: medir o pior tempo de gravação.
6. **Pressão de memória:** com 1 GB livres e navegador com 10 abas, nenhuma travada acima de 100 ms.
7. **Rede ruim:** perda de pacotes e quedas curtas no Wi-Fi de um caixa.
8. **IP do servidor muda** no meio do expediente: os caixas reencontram o servidor.
9. **Atualização com falha simulada:** retorno automático à versão anterior com o banco íntegro.
10. **Instalação sem internet** (instalador completo) em Windows 10 e Lubuntu.

---

## 7. Laboratório e matriz de testes

| Máquina | Perfil | Para quê |
|---|---|---|
| **A (piso, pior caso)** | i3 de 5ª geração, 6 GB DDR3-1333, **HDD 5400 rpm**, Windows 10 | Define o "funciona normal" |
| **B (piso, recomendado)** | Mesma máquina com **SSD** | Mede o ganho e a recomendação de SSD |
| **C** | i5 de 5ª geração, 6 GB, SSD | Faixa superior do piso |
| **D (resistência, sem garantia)** | i5 de 2ª geração, 4 GB | Avalia se o alvo antigo ainda é possível (H1) |
| **E** | Lubuntu 22.04 em A ou C | Paridade Linux |
| **Periféricos** | 3 a 5 impressoras térmicas, 3 leitores, 2 gavetas, balança, se houver | Matriz de hardware **testado** (nenhuma promessa fora da lista) |

Estresse de fundo padrão: antivírus em varredura, navegador com abas, atualização do sistema pendente.

---

## 8. Corte de MVP e ordem de fases revisada

### 8.1 Por que mexer no roadmap

O `00` coloca **nuvem, licença, cotas e anti-bypass** (Fases 5 a 7) **antes** de qualquer validação com lojista real. É construir uma proteção forte para um produto que **ainda não tem usuário**. A proteção contra burla só importa quando houver o que proteger. **Recomendação:** validar com lojas reais **primeiro**, com licença manual, e só então construir a camada de plano.

### 8.2 Camadas de MVP

| Camada | Conteúdo | Observação |
|---|---|---|
| **MVP-0 Esqueleto** | Domínio, SQLite, livro do caixa, estoque, venda atômica, **modo de 1 computador**, caixa operável, impressão de comprovante | Gate: testes de queda de energia em HDD |
| **MVP-1 Vendável (piloto)** | MVP-0 + produtos e importação de planilha, validade com alerta, contas a pagar e a receber básicas, fechamento do caixa, backup **local** (segundo disco) e exportação, primeira execução, pacote de suporte | **Sem nuvem.** Licença manual (arquivo de licença assinado, emitido por você) |
| **MVP-2 Rede da loja** | Modo multi-PC (servidor, pareamento, TLS), concorrência de caixas | Gate: 3 a 8 caixas |
| **MVP-3 Nuvem** | Conta, 2FA, ativação, licença automática, backup cifrado, restauração, kit de recuperação, landing e portal | Gate: restauração em máquina limpa |
| **MVP-4 Planos** | Entitlements, cotas em cadeia, janela de histórico, sessão única, limite de dispositivos, testes de bypass | Só aqui entram o Gratuito e o Pago com limites |
| **MVP-5 Crescimento** | Comissões e comandas, promoções, fiado, devolução, balança, etiquetas, papel do contador | Ordem pela demanda do piloto |
| **MVP-6 Expansão** | Multi-loja, importação de XML de NF-e, fiscal por terceiros (se validado), contingência de caixa (P12) | Depende do piloto |

### 8.3 Piloto fechado (entre MVP-1 e MVP-3)

- **3 lojas reais** no hardware real, por **30 dias**, com acompanhamento.
- Perguntas e métricas (hipóteses de sucesso, a calibrar): **nenhuma divergência de caixa causada pelo sistema**; tempo médio de venda **igual ou melhor** que o sistema anterior; **uso real** (a maioria das vendas passa pelo Mikella); o lojista **diz o que pagaria**; problemas de hardware e de rede reais.
- O que o piloto **decide:** necessidade de fiscal (NEG-002), necessidade de contingência (P12), segmento inicial (NEG-001), prioridade do MVP-5 e se Windows 7 importa (H2).

### 8.4 Mapeamento com o roadmap do `00`

| Fase do `00` | Passa a ser |
|---|---|
| 0 (spike) | **Fase 0 no hardware novo** (seção 7), incluindo o spike visual do `08` e a decisão de Tauri contra Qt |
| 1 (núcleo) | MVP-0 |
| 3 e 4 (cadastros e financeiro) | MVP-1 |
| 2 (servidor da loja) | MVP-2 (**depois** do piloto) |
| 5 (conta, landing, portal) | MVP-3 |
| 6 e 7 (licença, cotas, backup) | MVP-4 e parte do MVP-3 |
| 8 (comissões) | MVP-5 |
| 9 (NF-e, multi-loja) | MVP-6 |
| 10 (hardening) | Contínuo, com revisão independente antes de cobrar |

---

## 9. Impacto nos documentos existentes

| Documento | Alteração sugerida |
|---|---|
| `00` | Novo requisito mínimo e piso de hardware. Reordenar o roadmap (seção 8). Acrescentar decisões D12 em diante e P12 a P19. Glossário: **Modo 1 computador**, **Kit de recuperação**, **Venda em contingência**, **Carga inicial**. Corrigir a nota sobre Windows 7 (seção 0.1) |
| `01` | Requisitos mínimos na landing (piso novo, Windows 10 e Linux, sem promessa de Windows 7). Declarar o segmento inicial e o escopo fiscal (NEG-001, NEG-002). Remover qualquer afirmação de 4 GB |
| `02` | Incluir requisitos de `PROD-` (importação, promoção, fiado, devolução, balança, etiquetas, Pix estático, papel do contador, modo treinamento, primeira execução) e recorte por camada de MVP |
| `03` | Modo de 1 computador com adaptador **em processo** (ARQ-001 a ARQ-004); diário local do caixa (ARQ-010, quando decidido); pacote de suporte (OPS-001) |
| `04` | Relógio errado (HW-030 a HW-032), recuperação de conta e sucessão (REC-001 a REC-006), distribuição e assinatura (OPS-020), Windows 10 sem suporte (HW-090), segundo fator opcional |
| `05` | Carga inicial na cadeia de cotas (PROD-101), regra de edição por campos de identidade (PROD-100), relógio incerto nas cotas (HW-031) |
| `06` | Atualização forçada e suspensão (HW-021), bateria (HW-070 a HW-072), disco que mente (HW-012), contingência (ARQ-010), novos testes (seção 6) |
| `07` | **Hardware de referência substituído** (seção 1). Novos critérios HW-001 a HW-009. PERF-007 com dois instaladores. Soma de processos do WebView na medição. Compilação de linha de base (HW-080). Corrigir "Windows 7 fora do alvo" |
| `08` | Telas e estados novos: "A data deste computador parece errada", "Rodando na bateria", "Sem conexão com o servidor / venda em contingência", assistente de primeira execução, modo treinamento, Kit de recuperação, selo `PROMO`. O foco nas telas continua usando os tokens e os orçamentos de CPU da seção 1.3 |

---

## 10. Requisitos de segurança e privacidade decorrentes

- Importação de planilha trata **dados do lojista**; arquivo importado é tratado como **entrada não confiável** (limite de tamanho, validação, sem execução de fórmulas).
- Fiado e cadastro de clientes: **dados mínimos**, retenção e anonimização (`04`, seção 10).
- Notificações sem conteúdo (PROD-050): o e-mail **não revela** produto, valor ou nome.
- Pacote de suporte: **revisado pelo dono antes do envio**, sem PII (OPS-001).
- Recuperação de conta (REC-001 a REC-004): processos **auditados**, com aviso ao e-mail antigo e prazo de espera; o suporte **não** tem poder de "burlar" o 2FA por conversa.
- Kit de recuperação impresso: contém segredos. Aviso explícito de guarda física; **a senha nunca entra no kit**.

---

## 11. O que não foi comprovado

- Nenhum número de desempenho, memória, custo ou prazo foi medido. Todos são metas.
- A **comparação de CPU** entre o i3 de 5ª geração e o i5 de 2ª geração é hipótese; a única pontuação consultada é do próprio i3 (F4).
- Windows 11 não suportar 5ª geração, Windows 7 e WebView2, o estado do alvo Windows 7 no Rust, o BR Code do Pix, os prefixos de código de balança e a obtenção do estado da bateria **vêm de conhecimento geral e estão marcados [VERIFICAR]**.
- A informação sobre **ESU do Windows 10** vem de **uma matéria de imprensa** (F3) e das datas de fim de suporte divulgadas; confirmar na Microsoft.
- A informação fiscal vem de **um artigo de contabilidade** (F5) e **não substitui** a consulta à SEFAZ da UF e a um contador.
- Nenhum lojista foi entrevistado; todas as hipóteses de mercado são minhas e do dono do produto.

## 12. Decisões em aberto

| # | Pergunta | Recomendação |
|---|---|---|
| H1 | O i5 de 5ª geração com 6 GB DDR3-1333 continua como piso? | Sim. O i5 de 2ª geração/4 GB antigo estaria em resistência sem garantia; não é piso oficial |
| H2 | Suportar Windows 7? | **Proibido.** Fora do MVP e fora de escopo |
| H3 | Linux como caminho de primeira classe? | **Sim**: mesma suíte de testes e manual próprio |
| P12 | Venda em contingência quando o servidor cai | **Fora do MVP.** Decidir depois do piloto, com dados de queda de servidor |
| P13 | Modo de 1 computador como padrão | **Sim** (ARQ-001) |
| P14 | Teto de computadores por conta, além do teto por loja | Definir com dados do piloto. Hoje 8 lojas x 8 PCs = 64 |
| P15 | Fluxo de recuperação de conta e prazo de espera | Implementar REC-001 a REC-004 antes da cobrança. Prazo calibrado com o jurídico |
| P16 | Multi-loja: consolidado e transferência | Exportação no MVP; sincronização cifrada ponta a ponta depois |
| P17 | Formatos de exportação para o contador | Perguntar a 3 a 5 contadores antes de definir |
| P18 | Pix estático no caixa | Opcional, pós-MVP-1, **[VERIFICAR]** especificação |
| P19 | Segundo fator alternativo e códigos de recuperação de 2FA | **Sim**, junto com o Kit de recuperação |
| N1 | Carga inicial de produtos e a cota | **Opção B**: carga inicial única, com teto e janela parametrizáveis |
| N2 | Estratégia fiscal | **(a) agora, (b) após piloto se validado** |
| N3 | Teto de armazenamento e política para contas gratuitas inativas | Definir com medição do tamanho real do backup |
| N4 | Preço, ciclo e meios de cobrança do Pago | Definir depois do piloto, com disposição a pagar medida |
| P6 (atualizado) | Cotas de edição e exclusão | Contar só campos de identidade (PROD-100) |

---

## Fontes consultadas

- F1. Tauri 2, pré-requisitos (Windows 7 e posterior, WebView2 em Windows 10 desde a versão 1803, WebKitGTK 4.1 em Linux): https://v2.tauri.app/guides/prerequisites/
- F2. Tauri 2, instalador do Windows (modos de instalação do WebView2: tamanhos e exigência de internet): https://v2.tauri.app/ko/distribute/windows-installer/
- F3. Tom's Guide, extensão do ESU do Windows 10 até 12/10/2027 e condições: https://www.tomsguide.com/computing/windows-operating-systems/microsoft-extends-windows-10-support-for-another-year-what-you-need-to-know
- F4. Technical City, especificações do Core i3-5005U: https://technical.city/en/cpu/Core-i3-5005U
- F5. Trivium Contábil, NFC-e (quem emite, MEI, contingência): https://triviumcontabil.com.br/nfc-e/
- F6. Acer, fim do suporte regular do Windows 10 em 14/10/2025 (data usada em HW-090): https://community.acer.com/pt/kb/articles/18796-o-suporte-para-windows-10-termina-em-14-de-outubro-de-2025-como-se-preparar
