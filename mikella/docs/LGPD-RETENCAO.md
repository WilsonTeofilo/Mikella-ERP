# Retenção e expurgo de dados pessoais (LGPD)

> **Status:** implementação técnica concluída no P0; **revisão jurídica e
> contábil pendente**. Os prazos abaixo são a política operacional adotada pelo
> produto e devem ser confirmados por advogado e contador antes da publicação
> comercial. Este documento não substitui parecer jurídico.

## 1. Princípio

O Mikella ERP separa duas coisas que a LGPD trata de forma diferente:

- **registro fiscal/contábil** — precisa ser preservado por obrigação legal;
- **dado pessoal de cliente** — deve ser eliminado ou anonimizado quando deixa
  de ser necessário para a finalidade que justificou a coleta.

O expurgo nunca apaga `vendas`, `venda_itens`, `venda_pagamentos`, `livro_caixa`
ou `auditoria`. Ele **anonimiza** a linha correspondente em `pessoas`, mantendo
o `id` para não quebrar chaves estrangeiras nem relatórios históricos.

## 2. Tabela de retenção

| Dado | Onde fica | Prazo | O que acontece no fim |
|---|---|---|---|
| Venda, itens, pagamentos | `vendas`, `venda_itens`, `venda_pagamentos` | 5 anos (guarda fiscal) | Preservado integralmente |
| Livro-caixa | `livro_caixa` | 5 anos (guarda contábil) | Preservado; trigger impede UPDATE/DELETE |
| Trilha de auditoria | `auditoria` | 5 anos | Preservado; append-only |
| Cadastro de cliente sem movimentação | `pessoas` | 5 anos após a última movimentação/atualização | Nome, documento, telefone, e-mail e endereço anonimizados; `id` mantido |
| Cliente com crediário em aberto | `pessoas` + `contas_receber`/`parcelas` | Enquanto houver saldo | Nunca anonimizado automaticamente |
| Consentimento de marketing | `pessoas.consentimento_marketing` | Enquanto vigente + 5 anos | Zerado na anonimização |
| Backups locais | pasta `MIKELLA_BACKUP_DIR` | 30 dias (recomendado) | Rotação manual/automática; o expurgo não reescreve backup antigo |
| Backup offsite | fora da loja | 30 dias | Fase P1 — criptografado e com retenção própria |

### O que conta como "movimentação"

O corte usa, no fuso IANA da loja (`loja_config.fuso_horario`):

- `pessoas.updated_at` anterior à data de corte;
- nenhuma `contas_receber` criada dentro do prazo;
- nenhuma parcela de recebimento **aberta**, com vencimento dentro do prazo ou
  paga dentro do prazo.

Essa regra é conservadora de propósito: na dúvida, o cliente **não** é
anonimizado.

## 3. Base legal (a confirmar)

- LGPD, Art. 6º, X — necessidade e adequação.
- LGPD, Art. 7º, II — cumprimento de obrigação legal ou regulatória.
- LGPD, Art. 16 — conservação para cumprimento de obrigação legal e para
  estudo por órgão de pesquisa (dados anonimizados).
- LGPD, Art. 18, VI — direito à eliminação dos dados tratados com consentimento.
- Guarda fiscal/contábil: confirmar o prazo aplicável com o contador da loja
  (referências usuais: CTN e legislação previdenciária/trabalhista).

## 4. Procedimento operacional

Pré-requisitos: ser **DONO** (permissão `sistema.configurar`) e ter um backup
verificado do banco atual.

1. **Gerar e verificar o backup** antes de qualquer expurgo:
   - `POST /v1/sistema/backup-local` (gera a cópia local);
   - `POST /v1/sistema/backup/verificar` com o nome do arquivo.
2. **Simular** (não altera nada, é o padrão):

   ```http
   POST /v1/sistema/lgpd/expurgo
   Content-Type: application/json

   { "anos": 5 }
   ```

   Resposta: `modo = "simulacao"`, `clientes_elegiveis` e
   `clientes_anonimizados = 0`.

3. **Conferir** a data de corte e a quantidade. Se o número parecer alto,
   investigue antes de aplicar.
4. **Aplicar**:

   ```http
   POST /v1/sistema/lgpd/expurgo
   Content-Type: application/json

   { "anos": 5, "aplicar": true }
   ```

   A anonimização e a entrada `EXPURGO_LGPD` na auditoria entram na **mesma
   transação**: se a auditoria falhar, nada é gravado.

5. **Registrar a evidência**: guarde a resposta JSON e o comprovante de backup.
   A auditoria fica em `auditoria` com `acao = 'EXPURGO_LGPD'`.

### Restauração de backup

O expurgo é irreversível nos dados atuais. Se foi aplicado por engano, restaure
o backup com o servidor **parado**:

```powershell
mikella-store-server.exe --verificar-backup "backups\loja-AAAAMMDD.db"
mikella-store-server.exe --restaurar-backup "backups\loja-AAAAMMDD.db" --destino "dados\loja.db"
```

A restauração preserva o banco anterior como `loja.db.pre-restauracao-<ts>.db`.

## 5. Direito de eliminação individual

Para um titular específico, use `DELETE /v1/clientes/:id` (permissão
`clientes.gerenciar`). O handler:

- recusa a exclusão se houver crediário aberto;
- anonimiza os mesmos campos;
- grava `ANONIMIZACAO_TITULAR_LGPD` na auditoria;
- preserva vendas, livro-caixa e auditoria.

O expurgo em lote é apenas a versão automática e periódica dessa mesma regra.

## 6. Limitações conhecidas

- **Backups antigos continuam com PII.** O expurgo não reescreve cópias já
  geradas. A política de rotação de 30 dias é o controle compensatório.
- **A view `vw_resumo_dia` / relatórios antigos** não são reescritos; eles não
  expõem PII de cliente, apenas totais.
- **Revisão jurídica pendente:** confirmar prazo fiscal, hipóteses de guarda
  obrigatória e redação do aviso de privacidade da loja.
- **Encarregado (DPO):** o produto ainda não tem canal dedicado; usar o contato
  da loja até a fase de portal do titular (P2).

## 7. Mapa técnico

| Item | Arquivo |
|---|---|
| Regra de retenção e anonimização | `crates/infra-sqlite/src/lgpd.rs` |
| Endpoint de simulação/aplicação | `crates/store-server/src/handlers/system.rs` |
| Rota | `POST /v1/sistema/lgpd/expurgo` |
| Permissão | `sistema.configurar` |
| Coluna de consentimento | migração `0011_lgpd_consentimento.sql` |
| Teste de unidade | `lgpd.rs` (`simulacao_nao_altera_e_aplicacao_anonimiza`, etc.) |
