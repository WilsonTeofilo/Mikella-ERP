# ADR-0001 — SQLite local com um único escritor

## Contexto

O sistema é local-first e precisa funcionar em HD de 5400 rpm com 6 GB de RAM.
O banco fica na máquina da loja e é acessado apenas pelo processo servidor
(`mikella-server`). Não há réplica local nem acesso concorrente de vários
processos ao mesmo arquivo.

## Decisão

- Um único `rusqlite::Connection` por banco, aberto no composition root.
- `PRAGMA journal_mode = WAL` e `synchronous = FULL`: o commit só retorna depois
  que o WAL foi sincronizado, então queda de energia não perde venda confirmada.
- `foreign_keys = ON`, `busy_timeout = 5000`, `trusted_schema = OFF`,
  `secure_delete = ON`.
- A conexão não é compartilhada entre threads sem sincronização. O servidor
  envolve o `Database` em `Mutex`; a camada de aplicação recebe `&Connection`
  ou `&Transaction` e nunca abre conexões próprias.
- Toda operação que grava mais de uma linha roda em transação explícita.

## Consequências

- Sem `SQLITE_BUSY` entre processos da própria loja, porque só existe um escritor.
- Leitores concorrentes continuam possíveis via WAL, mas o servidor serializa
  tudo pelo mesmo `Mutex` — previsível e suficiente para o porte da loja.
- Backup não pode ser um `copy` ingênuo do `.db` com WAL ativo; o backup
  criptografado deve usar a API de backup do SQLite ou `VACUUM INTO`
  (implementação pendente na fase de nuvem).
- Migrações são embutidas no binário e verificadas por checksum blake3. Editar
  uma migração já aplicada faz o banco recusar a abertura; a correção é criar
  uma nova migração.
