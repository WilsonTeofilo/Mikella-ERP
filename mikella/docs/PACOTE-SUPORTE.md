# Pacote de suporte (OPS-001)

> **Status:** implementado no P0 técnico. O pacote reduz o tempo de suporte sem
> expor banco, backup, chave da loja ou dado de cliente. Ele **não** substitui a
> revisão humana antes do envio.

## 1. Princípio

O suporte precisa saber **em que estado** o sistema estava quando o problema
aconteceu. Hoje o diagnóstico era apenas um JSON na tela. O pacote de suporte
transforma esse JSON em um `.zip` reproduzível, com o mínimo necessário para
investigar e o máximo de exclusão por padrão.

O arquivo é gerado **no servidor**, dentro de uma pasta conhecida, por um usuário
autenticado. O frontend só recebe o nome, o tamanho e o hash.

## 2. Conteúdo do ZIP

| Arquivo | Conteúdo | PII |
|---|---|---|
| `diagnostico.json` | Saúde do SQLite, versão, integridade das cadeias de hash do livro-caixa/auditoria, migrações aplicadas e contagens volumétricas | Não |
| `ambiente.json` | Versão do app, SO, arquitetura, família, perfil de build e fuso do processo | Não |
| `logs.txt` | Últimas linhas de `MIKELLA_LOG_FILE` (máx. 512 KiB / 2.000 linhas), com CPF/CNPJ, e-mail, token, senha e caminho absoluto redigidos | Best-effort |
| `manifesto.json` | Formato, data, versão e BLAKE3 de cada arquivo | Não |
| `README.txt` | O que vai no pacote, o que não vai e o aviso de revisão | Não |

### O que nunca entra

- banco `loja.db`, arquivos `-wal`/`-shm` ou backup;
- `loja.key` / `MIKELLA_STORE_KEY`;
- token de sessão, senha, código 2FA;
- CPF/CNPJ, e-mail, telefone, endereço ou nome de cliente;
- caminho absoluto do servidor;
- stack trace ou SQL de erro.

O ZIP usa método **stored** (sem compressão). O conteúdo é texto pequeno e o
formato é escrito no próprio Rust, sem adicionar a dependência `zip` ao
workspace.

## 3. Endpoint

```http
POST /v1/sistema/suporte/pacote
Authorization: Bearer <token>
```

Sem corpo. Permissão: `sistema.diagnostico` (DONO e Gerente).

Resposta `200`:

```json
{
  "status": "sucesso",
  "arquivo": "suporte_20261109_143012.zip",
  "tamanho_bytes": 18432,
  "blake3_hash": "…",
  "conteudo": [
    "diagnostico.json",
    "ambiente.json",
    "logs.txt",
    "manifesto.json",
    "README.txt"
  ],
  "timestamp": "2026-11-09T17:30:12Z"
}
```

O arquivo **nunca sobrescreve** um pacote anterior. Em caso de colisão no mesmo
segundo, recebe `_2`, `_3`, etc.

## 4. Onde o arquivo é gravado

A pasta vem de `MIKELLA_SUPPORT_DIR`. O default é `suporte`, relativo ao
diretório de trabalho do servidor.

```powershell
$env:MIKELLA_SUPPORT_DIR = "D:\Mikella\suporte"
```

O log é opcional. Sem `MIKELLA_LOG_FILE`, `logs.txt` explica que o servidor grava
em stdout e que nenhuma linha foi incluída.

```powershell
$env:MIKELLA_LOG_FILE = "D:\Mikella\dados\mikella.log"
```

## 5. Procedimento de envio

1. **Gerar** o pacote pelo endpoint acima.
2. **Guardar o hash** retornado; ele também está no `manifesto.json`.
3. **Abrir `logs.txt` e revisar.** A higienização é best-effort; mensagens
   inesperadas podem escapar do padrão.
4. **Conferir** que o ZIP não contém `loja.db`, `loja.key` ou qualquer PII.
5. **Enviar** apenas o `.zip` revisado, nunca a pasta `dados/` inteira.
6. Registrar o envio; o evento `PACOTE_SUPORTE` fica na auditoria com o nome do
   arquivo, tamanho e hash.

## 6. Segurança e limites

- A auditoria é **best-effort**: o pacote já está no disco quando o registro é
  tentado. Uma falha de auditoria vai para o log e não esconde o arquivo gerado.
- O pacote não é criptografado. Ele deve sair da loja por um canal confiável
  (e-mail corporativo ou pasta compartilhada restrita), nunca por rede social.
- Não há rotação automática da pasta `suporte`. Apague pacotes antigos depois de
  encerrar o atendimento.
- O ZIP não inclui o banco; para reproduzir um erro de dados, o suporte precisa
  de um backup **autorizado pelo dono da loja**, seguindo `docs/LGPD-RETENCAO.md`.
- A restauração de backup continua sendo uma operação offline, com o servidor
  parado, via CLI (`--restaurar-backup`).

## 7. Mapa técnico

| Item | Arquivo |
|---|---|
| ZIP stored, CRC32, BLAKE3 e higienização de log | `crates/store-server/src/suporte.rs` |
| Diagnóstico compartilhado | `crates/store-server/src/handlers/system.rs` (`montar_diagnostico`) |
| Handler e rota | `crates/store-server/src/handlers/system.rs` / `POST /v1/sistema/suporte/pacote` |
| Migrações aplicadas | `crates/infra-sqlite/src/repos/sistema.rs` (`DiagnosticoRepo::migracoes`) |
| Permissão | `sistema.diagnostico` |
| Pasta de saída | `MIKELLA_SUPPORT_DIR` (default `suporte`) |
| Log de entrada | `MIKELLA_LOG_FILE` (opcional) |

## 8. Pendências

- [x] Ação “PACOTE DE SUPORTE” no menu principal (DONO e Gerente).
- [ ] Rotação/limpeza automática da pasta `suporte`.
- [ ] Criptografia do pacote se ele for enviado por canal não confiável.
- [ ] Teste de integração do endpoint com o servidor em execução.
- [ ] Rebuild de `dist/` para o shell empacotado receber a ação nova.
