# Mikella ERP — regras de segurança e LGPD

**Status:** política-alvo. Nada aqui está implementado ou testado. Cada item vira tarefa com evidência na fase correspondente. Parâmetros numéricos são **pontos de partida a calibrar**.

## 1. Premissa de ameaça (leia primeiro)

O Mikella roda **na máquina do cliente**. Quem controla o computador consegue, em tese, inspecionar e alterar o programa e o banco. Portanto:

- **Não existe proteção local absoluta.** O objetivo é elevar o custo de ataque a um nível que não compensa para o público-alvo, e **mover para a nuvem as decisões que o cliente não pode fraudar** (licença, plano, cotas, entrega de backup).
- A autoridade local é o **núcleo Rust do Servidor da Loja**, não a interface React.
- Detecção e rastreabilidade valem tanto quanto bloqueio: o que não dá para impedir deve deixar trilha.

### Ativos

Credenciais e chaves; banco da loja (vendas, caixa, estoque, financeiro); dados pessoais de clientes e funcionários; licença/entitlements; backups; trilha de auditoria; canal de atualização do app.

### Atores e ameaças prioritárias

| Ameaça | Exemplo | Defesa principal |
|---|---|---|
| Funcionário desonesto | Cancelar venda e ficar com dinheiro; roubar comissão | Aprovação por PIN, auditoria imutável, anomalias (`05`) |
| Cliente tentando burlar plano | Zerar cota, mover banco para outra conta, editar licença | Cotas em cadeia de hash reconciliada, vínculo criptográfico banco↔conta, licença assinada (`05`) |
| Invasor na rede local | Outro dispositivo da rede falando com o servidor da loja | TLS interno, pareamento, token por dispositivo, escuta restrita |
| Invasor na conta na nuvem | Credential stuffing, roubo de sessão | Argon2id, 2FA, rate limit, backup cifrado com chave que a conta não tem |
| Malware/ransomware no PC | Cifra ou apaga o banco | Backups versionados e imutáveis, cópia em segundo disco |
| Roubo físico do PC/disco | Copiar o banco | Cifra em repouso |
| Cadeia de suprimentos | Dependência maliciosa, atualização falsa | Dependências fixadas e auditadas, atualização assinada |
| Vazamento por log/erro | PII em arquivos de log | Redação, mensagens de erro estáveis |

## 2. Autenticação e identidade

### 2.1 Conta na nuvem (portal e ativação do desktop)

- SEG-001: login por **usuário ou e-mail + senha + 2FA por e-mail**, tanto no portal quanto na ativação/entrada do desktop.
- SEG-002: senha com **Argon2id** (biblioteca madura, sem implementação própria). Ponto de partida: mínimo OWASP (19 MiB de memória, 2 iterações, paralelismo 1), **calibrado** para aproximadamente 250–500 ms no servidor da nuvem. No desktop, calibrar também no hardware-alvo fraco, pois 19 MiB é irrelevante e o custo de CPU é o que pesa.
- SEG-003: política de senha: tamanho mínimo, aceitar frases longas, **bloquear senhas comuns e vazadas**, sem regras de composição previsíveis e sem troca periódica obrigatória. Limite máximo explícito de tamanho.
- SEG-004: código 2FA de 6 dígitos gerado com CSPRNG, **armazenado só como hash/HMAC**, expiração curta, tentativas limitadas, uso único, vinculado à finalidade.
- SEG-005: respostas de login, recuperação e cadastro **não revelam se e-mail, documento ou usuário existem**.
- SEG-006: rate limit e bloqueio progressivo por conta e por dispositivo (não depender só de IP: CGNAT é comum no Brasil). Identificadores de abuso com HMAC (chave secreta rotacionável).
- SEG-007: sessão na nuvem opaca, de alta entropia, **somente o hash no banco**, com rotação e **detecção de reuso** (reuso revoga a família). Cookie `__Host-`, `Secure`, `HttpOnly`, `SameSite`, com CSRF no portal.
- SEG-008: troca de senha, e-mail, 2FA ou dispositivo revoga sessões e exige reautenticação.

### 2.2 Login local (desktop)

- SEG-010: usuário + senha validados pelo Servidor da Loja. Para ações de balcão, **PIN numérico** por usuário (armazenado com KDF, com bloqueio por tentativas).
- SEG-011: **2FA por e-mail no desktop** é exigido na ativação do servidor, na adição de novo computador administrativo, na restauração, em troca de dono e **periodicamente (a cada N dias) quando houver internet**. Sem internet, vale a **janela offline** da licença para não parar a loja. **[DECISÃO PENDENTE P5]** — a exigência original é 2FA em todo login administrativo; a versão acima preserva a operação offline e deve ser confirmada pelo dono do produto.
- SEG-012: **sessão única por administrador** com revogação da sessão anterior (RF-042). Tokens de sessão local de curta duração, atrelados ao dispositivo.
- SEG-013: bloqueio de tela por inatividade configurável; operações sensíveis pedem PIN de aprovador novamente (step-up).

## 3. Autorização

- SEG-020: **negar por padrão.** Toda ação passa por verificação de **capacidade** no servidor da loja.
- SEG-021: **hierarquia estrita** (RF-001): ator só altera papéis abaixo do seu; ninguém altera o dono; ninguém se autoeleva.
- SEG-022: **o cliente (UI/PC caixa) nunca envia papel, plano, preço, desconto final, custo ou estado**; o servidor deriva do contexto autenticado.
- SEG-023: operações de alto risco exigem **aprovador com PIN** e geram auditoria: cancelamento de venda, estorno, desconto acima do limite, sangria, ajuste de estoque, alteração de preço em massa, fechamento com diferença, troca de vendedor em comissão, remoção de usuário.
- SEG-024: auditoria **append-only**: quem, o quê, quando, dispositivo, resultado, sem dados sensíveis. Triggers do banco recusam alteração e exclusão (ver `05` para encadeamento por hash).

## 4. Rede local (Servidor da Loja ↔ Caixas)

- SEG-030: API do servidor escuta **somente na interface da rede local** configurada; nunca exposta à internet. Instrução de instalação: sem redirecionamento de porta no roteador.
- SEG-031: **TLS obrigatório** na rede local. No ativação, o servidor cria uma **CA própria** e emite certificado do servidor; clientes confiam nessa CA após o **pareamento** (nunca "aceitar qualquer certificado").
- SEG-032: **pareamento** por código de uso único exibido no servidor, com expiração curta e tentativas limitadas. Resulta em **identidade do dispositivo** (par de chaves) e token revogável.
- SEG-033: o servidor mantém o **registro de dispositivos** (máximo 8 por loja), com nome do computador e usuário do SO, última atividade, e permite revogação.
- SEG-034: limites de tamanho de corpo, tempo de requisição, concorrência e taxa por dispositivo.
- SEG-035: proteção contra replay em operações sensíveis (nonce/chave de idempotência por operação).

## 5. Dados em repouso e em trânsito

### 5.1 Banco da loja

- SEG-040: SQLite **cifrado em repouso** (SQLCipher ou equivalente maduro) com chave por loja. **Avaliar custo de CPU/RAM no hardware-alvo no spike.**
- SEG-041: a chave do banco fica no **chaveiro do sistema** (DPAPI no Windows, Secret Service/libsecret no Linux, Keychain no macOS). Em Linux leve pode não haver serviço de chaves ativo: oferecer **modo de desbloqueio manual** (senha ao iniciar o servidor) como alternativa. O dono escolhe entre conveniência e proteção. **Limite declarado:** a cifra protege contra cópia do arquivo ou disco roubado; **não** protege contra quem já está logado no servidor com permissão.
- SEG-042: o banco carrega uma **identidade assinada** (store_id, account_id, época) e a chave do banco é **criptograficamente vinculada à conta** (`05`).
- SEG-043: `PRAGMA foreign_keys=ON`, tabelas `STRICT`, `integrity_check` na inicialização, `secure_delete` avaliado conforme custo.
- SEG-044: segredos nunca no código, no binário nem em arquivo legível. No binário vão **apenas chaves públicas** (verificação de licença e de atualização).

### 5.2 Backup

- Ver `06`. Cifra **no cliente** (AES-256-GCM ou XChaCha20-Poly1305 via biblioteca madura), chave separada da senha de login, derivada de **frase de recuperação**.

### 5.3 Em trânsito para a nuvem

- SEG-050: TLS moderno com verificação de certificado; opcionalmente **pinning** de chave pública da nuvem com chave de contingência para rotação.
- SEG-051: o app só conversa com domínios definidos em configuração assinada.

## 6. Endurecimento do Tauri/WebView

- SEG-060: **allowlist de comandos Tauri** mínima (capabilities por janela). Cada comando recebe DTO com `#[serde(deny_unknown_fields)]`, valida e verifica capacidade.
- SEG-061: **CSP restrita**, sem `unsafe-inline`/`unsafe-eval`; **nenhum conteúdo remoto** carregado na interface; assets empacotados.
- SEG-062: DevTools **desativado** em build de produção. Sem `dangerouslyDisableSandbox`, sem protocolo de arquivo irrestrito, sem acesso livre ao filesystem pela UI.
- SEG-063: a UI não acessa SQLite, chaveiro nem rede externa diretamente. Tudo passa pelo núcleo Rust.
- SEG-064: renderização de texto de usuário (nome de produto, observações) **sempre escapada**; sem `dangerouslySetInnerHTML` com dados não confiáveis.
- SEG-065: links externos abrem no navegador do sistema, com allowlist.

## 7. Cadeia de suprimentos e distribuição

- SEG-070: dependências com **versões fixadas** (`Cargo.lock`, `package-lock.json`), `cargo audit`/`cargo deny` e `npm audit` no CI, SBOM gerado por release, revisão de licença.
- SEG-071: scanner de **segredos** (ex.: Gitleaks) no CI e opcionalmente em pre-commit; segredo encontrado é tratado como **comprometido** (rotacionar primeiro).
- SEG-072: **atualização automática com assinatura própria** (updater do Tauri, chave de assinatura privada guardada fora do repositório e do CI comum, preferencialmente em cofre/hardware). Verificar assinatura antes de aplicar; proteger contra downgrade.
- SEG-073: assinatura de código: Windows (certificado de assinatura) e macOS (Apple Developer, quando houver demanda). Sem assinatura, instruir o usuário e aceitar o aviso do SO como custo inicial conhecido.
- SEG-074: builds reproduzíveis na medida do possível; pipeline com permissões mínimas.

## 8. Logs, erros e telemetria

- SEG-080: logs **sem** senha, PIN, token, 2FA, chave, CPF/CNPJ, telefone, e-mail completo ou conteúdo de venda de cliente identificável. Sanitizar entrada para evitar injeção de log.
- SEG-081: erros expostos à UI e à rede são **estáveis e genéricos**, com ID de correlação; detalhe técnico fica em log local protegido.
- SEG-082: **telemetria desligada por padrão**; se existir, opt-in, agregada e sem PII, com documentação do que é coletado.
- SEG-083: rotação e limite de tamanho de logs (disco do cliente é pequeno).

## 9. Nuvem (requisitos mínimos)

- SEG-090: banco da nuvem com roles separadas (aplicação sem privilégio de DDL), TLS, backups, segredos em cofre, credenciais por ambiente.
- SEG-091: CPF/CNPJ **cifrado em repouso** com índice cego HMAC para unicidade (padrão já desenhado no Peditto). Documento válido matematicamente não prova titularidade.
- SEG-092: armazenamento de backup **privado**, objetos imutáveis/versionados, acesso por URL assinada de curta duração após autorização.
- SEG-093: cobrança por **checkout hospedado** do provedor; **nunca** armazenar PAN/CVV/cartão. Webhook com assinatura HMAC verificada em tempo constante, janela de tempo, ID de evento único (replay) e conferência no provedor.
- SEG-094: operação interna (console do vendor) com identidade separada, MFA, privilégio mínimo e auditoria.

## 9.1 Tabela de verificação (antes de liberar release)

1. Banco copiado para outra máquina não abre sem a chave.
2. Dispositivo não pareado não consegue falar com a API local.
3. Payload com campo desconhecido é rejeitado.
4. Usuário sem capacidade recebe negação sem vazar existência de recurso.
5. Gerente não altera nem remove dono ou outro gerente.
6. Logs de uma sessão completa não contêm segredo nem PII.
7. Atualização com assinatura inválida é recusada.
8. Rate limit bloqueia força bruta de login, 2FA e PIN.
9. DevTools e navegação remota indisponíveis no build de produção.
10. `integrity_check` e restauração de backup testados.

## 10. LGPD (Lei 13.709/2018)

**Natureza deste texto:** diretriz técnica de engenharia, **não parecer jurídico**. Bases legais, contratos, termos e retenção exigem revisão de advogado antes de operar com dados reais.

### 10.1 Papéis

- **Lojista = controlador** dos dados de seus clientes e funcionários.
- **Mikella = operador** quanto ao backup do conteúdo da loja (que é cifrado e ilegível para o Mikella) e **controlador** dos dados de conta, cobrança, licença e segurança da plataforma. Pode haver zonas de controladoria conjunta; definir em contrato/DPA.
- Dados na loja: nome, CPF na nota (opcional), telefone, endereço, histórico de compras de clientes; dados de funcionários (nome, usuário, comissões).

### 10.2 Princípios aplicados ao desenho

| Princípio | Aplicação |
|---|---|
| **Finalidade e necessidade** | CPF do consumidor só quando o lojista precisar emitir documento com CPF; cadastro de cliente com campos mínimos e opcionais |
| **Segurança (art. 46)** | Cifra em repouso e em trânsito, controle de acesso por capacidade, backup cifrado no cliente, auditoria |
| **Transparência** | Aviso de privacidade na landing e no app; **aceite versionado** (versão, data, hora) no cadastro |
| **Livre acesso e direitos do titular (art. 18)** | Ferramentas para o lojista atender: exportar dados de um cliente, corrigir, anonimizar ou eliminar cadastro de cliente, respeitando guarda fiscal/contábil obrigatória |
| **Retenção e eliminação** | Política de retenção por tipo de dado, expurgo automatizado e documentado; dados fiscais mantêm prazo legal |
| **Prevenção** | Nada de PII em log; dados de teste sintéticos; ambientes sem dados reais |
| **Responsabilização** | Registro de operações de tratamento, auditoria, runbook de incidente |

### 10.3 Pendências obrigatórias antes de dado real

1. **Encarregado (DPO)** indicado e canal público para titulares/lojistas.
2. **Aviso de privacidade e termos** com aceite versionado.
3. **Contrato de operador/DPA** com o lojista.
4. **Avaliação de legítimo interesse** (LIA) para antifraude, segurança e licenciamento.
5. **Política de retenção** (conta, cobrança, backups, auditoria, dados de cliente) e implementação de expurgo.
6. **Runbook de incidente** (art. 48): critério de "risco relevante", prazos de comunicação à ANPD e aos titulares, registro.
7. **Inventário de operadores e transferência internacional** (provedor de nuvem, e-mail, pagamento) com contratos adequados.
8. **Exclusão de conta:** definir o que é apagado, o que é anonimizado e o que é retido por obrigação legal. Não prometer exclusão imediata de backup ou de dado fiscal.

### 10.4 Lembretes técnicos

- **Cifra e blind index (HMAC) não são anonimização.** Pseudonimização continua sendo dado pessoal.
- Consentimento só para o que é opcional (ex.: marketing). Cadastro, venda e cobrança usam execução de contrato, obrigação legal ou legítimo interesse.
- Marketing por e-mail: opt-in separado, desmarcado por padrão, descadastro imediato e lista de supressão.
- Dados de menores e dados sensíveis (art. 5º, II): **não coletar** sem decisão explícita e base legal; o produto não precisa deles.
- O caso "excluir banco local após 15 dias sem pagamento" **conflita** com deveres fiscais e de guarda do lojista; ver P2.

## 11. Resposta a incidentes (esqueleto)

1. Detectar e classificar (alerta de auditoria, relato, telemetria opt-in).
2. Conter (revogar sessões/dispositivos, rotacionar chaves e segredos, suspender integração afetada).
3. Investigar com logs e trilha.
4. Comunicar (lojista, titulares e ANPD quando houver risco relevante, nos prazos legais).
5. Corrigir, documentar causa raiz e atualizar este documento.
