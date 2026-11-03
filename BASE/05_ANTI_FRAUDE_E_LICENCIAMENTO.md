# Mikella ERP — anti-fraude e licenciamento

**Status:** desenho de projeto. Nada implementado. Dois eixos: (A) fraude do **cliente contra o plano** (bypass de licença e cotas); (B) fraude **interna da loja** (caixa, estoque, comissão).

## 0. Honestidade sobre os limites

Software que roda no computador do cliente **não pode ser 100% à prova de bypass**. O desenho eleva o custo e cria **pontos de verificação que só a nuvem controla**. Para o público (comércio pequeno), isso é suficiente; não é proteção contra um engenheiro reverso dedicado. Onde há dúvida, decide a **nuvem**.

Princípio: **o que o cliente não pode fraudar mora na nuvem; o que a nuvem não pode saber (conteúdo da loja) é protegido por criptografia no cliente.**

## 1. Licença e entitlements

- FRD-001: a nuvem emite um **token de licença** assinado com **Ed25519**. A chave **pública** vai embutida no app (com identificador de chave e suporte a **duas chaves** para rotação). A chave privada nunca sai do servidor de assinatura.
- FRD-002: conteúdo do token: `account_id`, `store_id`, plano, **entitlements** (limites: lojas, admins, dispositivos, cotas mensais, janela de histórico, módulos habilitados), `issued_at`, `valid_until`, `offline_grace_until`, versão da lista de dispositivos, `server_time`.
- FRD-003: **nenhum limite fica hardcoded** no app (D9). O app lê os entitlements do token verificado.
- FRD-004: **janela offline** (P4): sem internet, a loja opera até `offline_grace_until`. Passando disso, o servidor entra em modo restrito (exige reconexão), **sem apagar dados**.
- FRD-005: **relógio confiável:** o servidor guarda o último `server_time` assinado visto e um relógio monotônico. Se o relógio do sistema voltar atrás desse valor, o app sinaliza adulteração, usa o tempo derivado (último confiável + monotônico) e reporta à nuvem na próxima conexão.
- FRD-006: estados da assinatura (derivados na nuvem): `ATIVO`, `EM_TOLERANCIA` (15 dias corridos sem multa/juros, conforme regra atual), `SUSPENSO`, `CANCELADO`, `RETENCAO`. O estado chega via licença renovada.
- FRD-007: **bloqueio ao suspender:** impede novas vendas e escritas; mantém leitura, exportação e backup. **Não destruir dados locais** (P2). A eliminação, se houver, vale para a **cópia na nuvem**, após aviso prévio e prazo definidos em termos e revisados juridicamente.
- FRD-008: bloqueio por **IP não é usado** (CGNAT, IP dinâmico). Vínculo é por **conta + loja + dispositivo**.

## 2. Vínculo criptográfico banco ↔ conta (anti-troca de banco)

Problema: duas contas gratuitas (CPFs diferentes) tentando passar o banco de uma para a outra, ou levar um banco gratuito para uma conta com mais capacidade.

- FRD-010: na criação da loja, a nuvem emite um **manifesto assinado** `{account_id, store_id, db_epoch, created_at}`. O servidor grava o manifesto numa tabela `instance_identity` do banco.
- FRD-011: a **chave de cifra do banco é única por loja** e **derivada/embrulhada com material da conta** (chave de conta liberada na ativação autenticada). Um banco da conta A **não abre** na conta B porque a conta B não tem a chave. É vínculo criptográfico, não apenas uma flag.
- FRD-012: ao abrir ou restaurar, o servidor verifica a assinatura do manifesto contra a chave pública embutida **e** confere `account_id` com a licença atual. Divergência → recusa, registra evento e notifica a nuvem.
- FRD-013: cada **backup** inclui o manifesto e é assinado por **cadeia de manifestos de backup** (hash do anterior), para detectar remoção, reordenação ou injeção de pedaços.
- FRD-014: **um documento (CPF/CNPJ) = uma conta gratuita** (índice cego na nuvem). O mesmo vale para trial, se houver. Contas com documentos diferentes são contas diferentes, cada uma com seu limite "e ponto final", e **não trocam dados entre si**.
- FRD-015: aplica-se também ao plano Pago: cada conta tem limites próprios; banco não migra entre contas.

## 3. Cotas à prova de zerar (produtos criados, editados, arquivados)

- FRD-020: cada ação de cota gera um evento em um **registro encadeado por hash** (`quota_events`: `id`, `store_id`, `tipo`, `período AAAA-MM`, `ator`, `prev_hash`, `hash`). Triggers do banco recusam `UPDATE/DELETE`. Quebra na cadeia é detectável.
- FRD-021: o **período** (mês) é calculado com **tempo confiável** (FRD-005) e fuso da loja, não com o relógio cru.
- FRD-022: o servidor **reporta** o resumo/cabeça da cadeia à nuvem na sincronização. A nuvem guarda o **maior contador por loja e período** (`max(local, nuvem)`). **Restaurar um backup antigo não reinicia a cota**, porque a nuvem lembra.
- FRD-023: **arquivar (excluir) não devolve cota** (D8). Criar → arquivar → recriar em laço continua consumindo.
- FRD-024: contadores são **mostrados ao usuário** ("61 de 85 neste mês"). Ao estourar, a UI explica e oferece upgrade; o que já existe continua funcionando.
- FRD-025: offline, a cota é aplicada localmente pelo servidor (com base nos entitlements do último token). A reconciliação posterior pode **marcar divergência**, e a política é: **bloquear novas criações** até regularizar, nunca apagar produtos já criados.

## 4. Histórico bloqueado no plano Gratuito (janela de 6 meses)

Requisito: no Gratuito, o lojista vê só os últimos 6 meses; ao assinar o Pago, recupera tudo. Após formatar o PC, a nuvem **não** pode entregar mais do que o plano permite.

Desenho em duas camadas:

1. **Particionamento mensal.** O backup e o arquivo morto são organizados em **pedaços por mês** (ver `06`), cada um cifrado.
2. **Entrega controlada pela nuvem (server-side gating).** No plano Gratuito, a nuvem **não entrega** pedaços mais antigos que a janela. Ao assinar, entrega.
3. **Selagem local.** No servidor da loja, meses fora da janela são movidos do banco ativo para um **arquivo selado** cifrado, cuja chave (**chave de histórico**) só é entregue/embrulhada pela nuvem quando o entitlement permite. Sem isso, o dado fora da janela **não está legível no PC** (um usuário que abra o SQLite não vê o histórico antigo).

**[DECISÃO PENDENTE P10]** Tensão de projeto: se a nuvem entrega a chave de histórico, ela tecnicamente consegue liberar a leitura dos dados selados (quebra parcial do "zero-knowledge" **para o histórico antigo do plano gratuito**). Alternativas: (a) aceitar essa exceção e documentar na política de privacidade; (b) manter zero-knowledge puro e ocultar o histórico apenas por **controle de acesso na aplicação** (mais fraco: o dado está legível no PC). Recomendação: **(a) com informação transparente ao lojista**, ou deixar a janela como regra de **exibição** assumindo que o bypass local é possível para quem inspeciona o arquivo. Decidir antes do pós-MVP (MVP-5+).

- FRD-030: relatórios e consultas respeitam a janela do entitlement em **uma única camada** de acesso a dados (não espalhar `WHERE data >= ...` pela UI).
- FRD-031: dados selados **nunca são apagados** por causa do plano; apenas inacessíveis até o upgrade.
- FRD-032: restauração em PC limpo (Gratuito) entrega **somente** a janela, mesmo que o solicitante tenha a frase de recuperação.

## 5. Limite de computadores e sessão única

- FRD-040: **registro de dispositivos** (máx. 8 por loja): `device_id` (par de chaves), nome do computador, usuário do SO, data de pareamento, última atividade. Mantido no servidor da loja e espelhado na nuvem.
- FRD-041: o 9º pareamento é recusado com explicação; o dono pode **revogar** um dispositivo (pelo desktop ou pelo portal).
- FRD-042: **sessão única por administrador**: o servidor da loja mantém a tabela de sessões. Login do mesmo admin em outro PC dispara a pergunta de assumir (RF-042); confirmando, a sessão antiga é revogada e o dispositivo antigo recebe aviso na próxima chamada.
- FRD-043: **fingerprint de máquina** (hash de identificadores estáveis) é **sinal**, não identidade. Pode sofrer mudança por troca de hardware; nunca bloquear sozinho, apenas pontuar risco e pedir 2FA.
- FRD-044: clonagem de máquina virtual/imagem de disco: dois servidores com o mesmo `store_id` ativo geram **conflito de época** (`db_epoch`) detectado na nuvem; o mais recente assume e o outro é colocado em modo restrito até o dono resolver.

## 6. Fraude interna: caixa e estoque

- FRD-050: **PIN de aprovador** para: cancelamento/estorno, desconto acima do limite do papel, sangria, ajuste de estoque, fechamento com diferença, remoção de usuário. Registra solicitante e aprovador (devem ser pessoas diferentes quando o solicitante for operador).
- FRD-051: **livro do caixa imutável:** nada é editado. Correção é **movimentação compensatória** com motivo. Triggers bloqueiam `UPDATE/DELETE`.
- FRD-052: **fechamento cego** (opcional por loja): o operador conta o dinheiro sem ver o valor esperado; a diferença é calculada depois e registrada.
- FRD-053: **cadeia de hash da auditoria** (`prev_hash`): remover ou alterar um registro quebra a cadeia; o hash de cabeça é periodicamente enviado à nuvem (âncora externa, sem conteúdo).
- FRD-054: **indicadores de anomalia** (relatório para dono/gerente): cancelamentos por operador e horário, descontos acima da média, vendas canceladas após pagamento, sangrias frequentes, diferenças recorrentes de caixa, ajustes de estoque sem venda correspondente, produtos de alto valor com baixa sem venda.
- FRD-055: **limites por papel** (desconto máximo, valor de sangria sem aprovação) como parâmetros da loja, não constantes.
- FRD-056: operação em horário fora do expediente configurado pede aprovação ou gera alerta.

## 7. Fraude de comissão (comandas)

Ameaça: o caixa (ou outro vendedor) atribui a venda a si mesmo ou a um cúmplice para ganhar comissão.

- FRD-060: **comanda aberta pelo vendedor** com autenticação dele no momento da abertura; o caixa só **consome** a comanda (a atribuição vem dela, não do caixa).
- FRD-061: item comissionável **sem comanda**: o caixa informa o **código do vendedor** (bipado do crachá ou digitado). Recomendado: **confirmação do vendedor** (PIN dele em terminal ou leitor) em vendas acima de um valor configurável.
- FRD-062: **trocar o vendedor** de uma venda já feita exige aprovação de gerente, justificativa e fica na auditoria.
- FRD-063: **regras de comissão versionadas** e aplicadas **no servidor** com snapshot na venda; o caixa não vê nem altera percentuais.
- FRD-064: indicadores: proporção de vendas comissionáveis **sem comanda** por caixa; atribuições manuais vs. por crachá; concentração de comissão em um vendedor por um único caixa; trocas de vendedor pós-venda; vendedor com comissão alta em dias em que não estava escalado.
- FRD-065: devolução e cancelamento **estornam** comissão automaticamente.
- FRD-066: o anti-fraude é **rastreabilidade e alerta**, não bloqueio total: o objetivo é tornar a fraude visível e atribuível.

## 8. Fraude de plano por restauração e replay

- FRD-070: restaurar backup antigo para "voltar no tempo" e reiniciar cotas é neutralizado pela nuvem (FRD-022).
- FRD-071: reenvio de backup de outra conta é recusado pelo manifesto (FRD-012/013).
- FRD-072: edição manual do banco para elevar plano é inócua: os entitlements vêm do token assinado, **não** do banco.
- FRD-073: substituição do binário por versão modificada: mitigada por assinatura de código/atualização e por **verificações do servidor da nuvem** (licença renovada só para clientes e versões conhecidas). Reconhecer que modificação local do binário é possível e depender da validação periódica da nuvem.

## 9. Testes de bypass obrigatórios (MVP-4 e MVP-5)

1. Editar a tabela de plano/entitlement no SQLite e verificar que nada muda.
2. Zerar a cadeia de cotas e verificar detecção e bloqueio de novas criações.
3. Restaurar backup antigo e verificar que a cota do mês não reinicia.
4. Abrir banco da conta A na conta B: deve falhar.
5. Voltar o relógio do sistema: deve ser detectado.
6. Parear o 9º computador: deve ser recusado.
7. Dois servidores com o mesmo `store_id`: conflito de época detectado.
8. Operador tenta cancelar venda sem aprovador: bloqueado.
9. Caixa tenta atribuir comissão manualmente em volume anormal: gera alerta.
10. Restauração do Gratuito: não entrega histórico fora da janela.
