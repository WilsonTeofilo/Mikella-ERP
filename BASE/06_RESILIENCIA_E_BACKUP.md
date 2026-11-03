# Mikella ERP — resiliência, backup e recuperação

**Status:** desenho. Nenhum teste de queda de energia foi executado ainda. A promessa de "não perder o caixa" só pode ser publicada **depois** dos testes da seção 8.

## 1. Requisito central

Se a luz cair ou a internet cair no meio do expediente, o caixa **volta exatamente de onde parou**: mesmo valor em caixa, mesmas vendas, mesma sessão. A sessão **não é finalizada nem reiniciada** automaticamente. Evitar que o operador responda por diferença causada pelo sistema.

## 2. Modelo de falhas e resposta

| Falha | Efeito esperado | Mecanismo |
|---|---|---|
| Queda de energia no **servidor da loja** | Perde no máximo a transação em andamento; banco íntegro; caixa retoma | SQLite WAL com `synchronous=FULL`, transações atômicas, livro imutável |
| Queda de energia em um **PC caixa** | Sessão retoma; carrinho em andamento recuperado | Estado do caixa no servidor; **rascunho local** persistido |
| Queda de **internet** | Loja continua vendendo | Local-first; só o servidor usa a nuvem; janela offline da licença |
| Queda da **rede local** entre caixa e servidor | Caixa segue montando a venda e reenvia | Rascunho local + idempotência; indicador de "sem conexão com o servidor" |
| **Travamento** do app | Reabrir e retomar | Estado derivado do livro; recuperação na inicialização |
| **Disco corrompido** no servidor | Restaurar do último snapshot/backup | Snapshot local em segundo disco + backup na nuvem |
| **PC servidor quebrado** | Loja migra para outra máquina | Restauração completa (seção 6) |
| **Ransomware/exclusão** | Voltar no tempo | Backups versionados e imutáveis |

**Limite declarado:** software não impede dano físico causado por corte de energia durante escrita em disco ruim. **Nobreak e segundo disco são requisitos recomendados na instalação.** Discos baratos que mentem sobre `fsync` podem comprometer a garantia; avisar o lojista.

## 3. Persistência de dados no servidor da loja

- RES-001: SQLite em **WAL**; `synchronous=FULL` para dados financeiros e de estoque (priorizar durabilidade sobre velocidade; medir impacto no hardware-alvo, ver `07`).
- RES-002: **uma transação por caso de uso** (venda, sangria, estorno, entrada de estoque). Ou grava tudo, ou nada.
- RES-003: **único processo escritor** (o serviço do servidor). Nenhum outro processo abre o arquivo. Sem compartilhamento por rede.
- RES-004: operações com **chave de idempotência** (UUID gerado antes). Reenvio após timeout devolve o resultado original.
- RES-005: **verificação na inicialização**: `PRAGMA integrity_check` (ou `quick_check` se o custo for alto), conferência de cadeia de hash da auditoria e das cotas, checagem de migração pendente. Falha → modo seguro (somente leitura) + restauração guiada.
- RES-006: **checkpoint de WAL** controlado para não crescer indefinidamente; arquivo WAL monitorado.
- RES-007: espaço em disco monitorado; alerta ao dono antes de faltar espaço (banco sem espaço corrompe fluxo).

## 4. Livro do caixa (event sourcing mínimo)

O estado do caixa **não é um número solto**; é a soma de movimentações imutáveis.

| Tipo | Efeito no saldo de dinheiro físico |
|---|---|
| `ABERTURA` | + valor inicial |
| `VENDA_DINHEIRO` | + valor recebido − troco |
| `SUPRIMENTO` | + |
| `SANGRIA` | − |
| `ESTORNO` / `CANCELAMENTO` | compensação |
| `FECHAMENTO` | registra contagem, esperado e diferença |

- RES-010: `saldo_esperado = Σ movimentações da sessão`, **sempre calculado**; qualquer coluna de saldo em tabela é **projeção** atualizada na mesma transação e reconciliável.
- RES-011: ao religar, o app reconstrói e exibe a sessão aberta com o valor correto. **Não há "voltar ao valor inicial".**
- RES-012: sessão de caixa **só fecha por ação explícita** (operador ou gerente) com conferência. Nenhum timeout encerra sessão.
- RES-013: `UPDATE/DELETE` bloqueados por trigger nas tabelas de movimento; correções por compensação.
- RES-014: múltiplos caixas: cada sessão tem seu livro; o consolidado da loja é derivado.

## 5. Rascunho local no PC caixa

- RES-020: o carrinho em andamento é salvo localmente **a cada alteração** (arquivo pequeno cifrado ou banco local mínimo), com ID da venda já gerado.
- RES-021: ao reabrir, o caixa oferece **retomar a venda**. Ao concluir, envia ao servidor com a mesma chave de idempotência.
- RES-022: **o rascunho não é fonte oficial de preço**; o servidor recalcula no fechamento. Se o preço mudou durante a queda, avisar o operador.
- RES-023: rascunhos antigos e órfãos são expurgados por política de retenção.

## 6. Backup e restauração

### 6.1 Camadas

1. **Snapshot local automático** em **segundo disco ou pendrive** configurado (API de backup online do SQLite ou `VACUUM INTO`), com retenção rotativa (ex.: horários + diários + semanais, parâmetros ajustáveis).
2. **Backup cifrado na nuvem** (sincronização incremental).
3. **Exportação manual** para o lojista (arquivo cifrado) quando desejar.

### 6.2 Formato do backup na nuvem

- RES-030: **pedaços por mês** (`AAAA-MM`), cada um cifrado individualmente e com hash. Mudou só o mês corrente? Só ele sobe. Permite a janela de 6 meses do plano e restauração parcial.
- RES-031: um **manifesto** assinado lista pedaços, hashes e versão, encadeado ao manifesto anterior (`05`, FRD-013).
- RES-032: **versionado e imutável**: guarda as últimas N versões; nova versão não sobrescreve a anterior. Ransomware que cifre o banco local e sincronize não destrói as versões boas.
- RES-033: envio por **fila (outbox)** com retry, backoff, limite de tentativas, estado visível ("último backup: ontem 22:10") e idempotência por hash.
- RES-034: **verificação periódica:** baixar e validar amostra de pedaços (hash e decifragem) para detectar backup corrompido antes do desastre.

### 6.3 Chaves

- RES-040: a **chave de backup** é gerada no cliente e **embrulhada** pela **frase de recuperação** (alta entropia, ex.: lista de palavras) exibida uma vez na ativação e **guardada pelo lojista em papel**. A nuvem **nunca** recebe a chave em claro. **[Decisão P3: recomendada; perder a frase = perder o backup, comunicado claramente.]**
- RES-041: algoritmo: AEAD moderno (AES-256-GCM ou XChaCha20-Poly1305) via biblioteca madura, nonce único por pedaço, **sem implementação própria de criptografia**.
- RES-042: a **senha de login** e a **chave de backup são independentes**: invadir a conta na nuvem não entrega dados legíveis.
- RES-043: rotação de chave possível: re-embrulhar a chave de dados com nova frase sem recifrar tudo.
- RES-044: perda de dispositivo e frase: sem recuperação. Documentar e permitir ao dono **gerar nova frase** (invalida o embrulho antigo) enquanto ainda tiver acesso ao servidor.

### 6.4 Restauração (formatou o PC ou trocou de máquina)

1. Instalar em modo servidor.
2. Login na conta com **2FA por e-mail**.
3. Informar a **frase de recuperação**.
4. A nuvem verifica o plano e entrega **apenas os pedaços permitidos** (Gratuito: últimos 6 meses; Pago: todos) (FRD-032).
5. O servidor valida o manifesto, verifica hashes, decifra, reconstrói o banco, aplica migrações e roda `integrity_check`.
6. Compara cadeia de cotas e época (`db_epoch`) com a nuvem.
7. Pareia novamente os caixas.
8. Registra evento de restauração na auditoria e notifica o e-mail do dono.

- RES-050: restauração exige **novo dispositivo autorizado** e gera alerta ao dono (proteção contra invasor que roubou a conta).
- RES-051: **tempo-alvo e teste:** a restauração precisa ser ensaiada de ponta a ponta em máquina limpa antes do lançamento, com medição de tempo.
- RES-052: o modo restrito de somente leitura impede vendas até o `integrity_check` passar.

### 6.5 Migração para outro PC servidor (sem desastre)

- Fluxo guiado: exportar do antigo (ou restaurar da nuvem), ativar no novo, **revogar o servidor antigo**, reparear clientes. A época (`db_epoch`) incrementa para impedir dois servidores ativos.

## 7. Comportamento com a nuvem indisponível

- RES-060: nuvem fora do ar **não afeta vendas**. O backup acumula na fila.
- RES-061: o dono vê alerta se ficar **N dias sem backup bem-sucedido** (parâmetro).
- RES-062: se o `offline_grace_until` expirar, entra em modo restrito explicado, sem apagar dados.

## 8. Plano de testes de resiliência (gate do MVP-0, MVP-2 e pós-MVP)

1. **Kill -9 aleatório** do servidor durante milhares de vendas simuladas: nenhuma venda parcial; saldo confere.
2. **Corte de energia real ou em VM** durante venda, sangria e fechamento.
3. Queda do PC caixa no meio de uma venda: retoma o rascunho; sem duplicar ao reenviar.
4. Desconectar o cabo de rede do caixa durante o pagamento: reenvio idempotente.
5. 3 a 8 caixas simultâneos por tempo prolongado: sem erro de banco, estoque coerente.
6. WAL corrompido/truncado (injeção de falha): detecção e restauração guiada.
7. Disco cheio: alerta e modo seguro, sem corrupção.
8. Restauração em máquina limpa, com tempo medido, nos dois sistemas.
9. Backup corrompido (um pedaço adulterado): detecção por hash.
10. Ransomware simulado (banco cifrado e sincronizado): recuperação de versão anterior.

## 9. Instalação: requisitos recomendados ao lojista (entram na tela de instalação)

- Nobreak no computador servidor (e, se possível, nos caixas).
- Segundo disco ou pendrive dedicado a snapshots.
- Servidor com IP fixo ou reserva de DHCP na rede local.
- Anotar a frase de recuperação em papel e guardar em local seguro.
- Não desligar o servidor com sessões de caixa abertas sem antes fechar os caixas, quando possível; se ocorrer, o sistema se recupera.
