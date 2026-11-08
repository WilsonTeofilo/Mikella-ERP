# Mikella ERP — Conformidade LGPD (Lei nº 13.709/2018)

**Criado em:** 03/11/2026  
**Base legal:** Lei Geral de Proteção de Dados Pessoais — Lei nº 13.709/2018 (atualizada até junho/2024, edição Senado Federal)  
**Responsável:** Dono do sistema (papel `Dono` no ERP)  
**Escopo:** Sistema desktop local-first para pequeno comércio. Os dados pessoais coletados referem-se exclusivamente a **clientes da loja** (CPF, nome, telefone, e-mail, endereço) e a **operadores** (username, hash de senha, papel, capacidades). Nenhum dado é transmitido para servidores externos no MVP-0 ao MVP-1.

---

## 1. Quais dados pessoais o Mikella trata

| Dado | Titular | Base legal (Art. 7º) | Onde fica |
|---|---|---|---|
| CPF | Cliente | Execução de contrato (inciso V) + Proteção ao crédito (inciso X) | `pessoas.documento` (SQLite local) |
| Nome completo | Cliente | Execução de contrato (inciso V) | `pessoas.nome` |
| Telefone / celular | Cliente | Execução de contrato (inciso V) | `pessoas.telefone` |
| E-mail | Cliente | Execução de contrato (inciso V) | `pessoas.email` |
| Endereço | Cliente | Execução de contrato para entrega (inciso V) — **opcional** | `pessoas.endereco` |
| Username + hash Argon2id | Operador | Contrato de trabalho / obrigação legal (inciso II) | `usuarios.username`, `usuarios.senha_hash` |
| Log de operações (caixa) | Operador | Obrigação legal / proteção ao crédito (incisos II, X) | `livro_caixa` (imutável) |
| IP / dispositivo | Sistema | Segurança / prevenção a fraude (Art. 11, §1º) | `sessoes_caixa.dispositivo_id` (UUID local) |

> [!IMPORTANT]
> **Dado pessoal sensível (Art. 5º, II):** O Mikella NÃO coleta dados de saúde, biometria, origem racial, convicção religiosa, opinião política, filiação sindical, vida sexual ou dados genéticos. Caso no futuro algum módulo necessite, requer avaliação de impacto (DPIA) e base legal explícita do Art. 11º.

---

## 2. Princípios LGPD aplicados (Art. 6º)

| Princípio | Como o Mikella implementa |
|---|---|
| **Finalidade** | CPF e nome são usados exclusivamente para identificação do cliente na venda e crediário. Endereço apenas para entrega. |
| **Adequação** | Apenas os dados mínimos necessários são solicitados. Endereço é **opcional por padrão**. |
| **Necessidade** | O campo de e-mail é opcional. O sistema funciona com nome + telefone apenas. |
| **Livre acesso** | Operador com papel ≥ Gerente pode consultar e imprimir cadastro do cliente a qualquer momento (F4 do PDV). |
| **Qualidade** | Titular pode atualizar seus dados via F4 do PDV; o operador confirma por telefone. |
| **Transparência** | A loja é a controladora; o Mikella ERP é o operador técnico. Os dados ficam **100% locais** (sem nuvem no MVP-1). |
| **Segurança** | SQLite WAL + `synchronous=FULL`, tokens Bearer com expiração, senhas Argon2id (nunca em texto), logs sem PII. |
| **Prevenção** | Livro-caixa imutável com BLAKE3 (detecta adulteração). Sem exportação não autorizada de dados. |
| **Não discriminação** | O sistema não usa dados pessoais para definir preços ou restringir atendimento. |
| **Responsabilização** | Livro de auditoria imutável por sessão de caixa; log de operadores com `usuario_id`. |

---

## 3. Direitos do Titular (Art. 18º) — Como o lojista atende

O **lojista** (controlador) é responsável por atender os direitos. O ERP provê as ferramentas técnicas:

| Direito | Ferramenta no ERP | Status |
|---|---|---|
| Confirmação de existência | Busca por CPF/telefone em F4 | ✅ Implementado |
| Acesso aos dados | Tela de cadastro de cliente (F4) | ✅ Implementado |
| Correção de dados inexatos | Edição do cadastro em F4 | ✅ Implementado |
| Eliminação / anonimização | **TODO MVP-2:** endpoint `DELETE /v1/clientes/:id` com anonimização (substitui CPF/nome/telefone/email por hashes) e exclusão lógica | ❌ Pendente |
| Portabilidade | **TODO MVP-3:** exportação de CSV com dados do cliente | ❌ Pendente |
| Revogação de consentimento | Desativação do cadastro + anonimização (ver acima) | ❌ Pendente |
| Oposição a tratamento automatizado | N/A — Mikella não usa decisão automatizada com impacto ao titular | ✅ N/A |

> [!WARNING]
> Os direitos de **eliminação** e **portabilidade** são obrigatórios pela LGPD. Devem ser implementados **antes de qualquer dado de cliente real ser armazenado em produção**. Enquanto o sistema está em **fase piloto com dados de teste**, o risco é controlado. **Implementar no MVP-2 é mandatório.**

---

## 4. Retenção e Descarte de Dados (Arts. 15º e 16º)

| Dado | Retenção necessária | Motivo |
|---|---|---|
| CPF + nome | Enquanto houver crediário em aberto + 5 anos após quitação | Proteção ao crédito, obrigação fiscal |
| Histórico de vendas | 5 anos | Obrigação fiscal (Lei 9.430/96) |
| Log do livro-caixa | 5 anos | Obrigação fiscal contábil |
| Tokens de sessão | Até expiração (configurável, default: 8h) | Segurança operacional |
| Código 2FA (memória) | ≤ 5 minutos | Segurança — descartado após uso |

> [!NOTE]
> O SQLite local não tem TTL automático. O lojista (controlador) é responsável por realizar backups e, quando necessário, o descarte após os prazos legais. No MVP-3, o sistema alertará sobre dados com prazo de retenção expirado.

---

## 5. Segurança Técnica (Art. 46º — Capítulo VII)

Medidas já implementadas:

- **Autenticação:** Argon2id (memory-hard, resistente a GPU) + 2FA BLAKE3 (6 dígitos, expira em 5 min)
- **Autorização:** Token Bearer com expiração + RBAC por capacidades (sem confundir papéis com permissões raw)
- **Integridade:** Livro-caixa com cadeia de hashes BLAKE3 (qualquer adulteração direta no banco é detectada no fechamento)
- **Transmissão:** API restrita a `127.0.0.1` (loopback) — dados **nunca saem pela rede** no MVP-1
- **Logs:** Zero PII (sem CPF, senha, token em logs `tracing`) — regra `SEG-010` do `04_SEGURANCA.md`
- **Backup:** `POST /v1/sistema/backup-local` com `VACUUM INTO` + verificação de integridade BLAKE3
- **Anti-autofill:** Campos de login com `autocomplete="off"` + inputs fantasmas (anti-WebView2/Edge)

Medidas a implementar:

- [ ] **MVP-2:** TLS autoassinado via `rcgen` para comunicação em rede local (multi-caixa)
- [ ] **MVP-2:** Endpoint de anonimização de titular (direito de eliminação)
- [ ] **MVP-3:** Backup cifrado zero-knowledge na nuvem (chave permanece na loja)
- [ ] **MVP-3:** Notificação ao titular em caso de incidente de segurança (Art. 48º)

---

## 6. Responsabilidades (Arts. 37º–40º)

| Papel LGPD | Quem é | Obrigações |
|---|---|---|
| **Controlador** | Lojista / dono da loja | Define finalidade, atende direitos dos titulares, responde à ANPD em caso de incidente |
| **Operador** | Mikella ERP (software) | Trata dados apenas conforme instruções do controlador (lojista), implementa segurança técnica |
| **Encarregado (DPO)** | Deve ser indicado pelo controlador | Para pequeno negócio: pode ser o próprio dono. Indicação opcional para MPE per Art. 41º, mas recomendada |

> [!NOTE]
> O Mikella ERP atua como **operador** (Art. 37º, 39º). O **lojista** é o controlador e deve manter registro das atividades de tratamento (Art. 37º) e indicar encarregado se processar dados em larga escala.

---

## 7. Plano de Implementação no Roadmap

| Etapa | Item LGPD | Prazo |
|---|---|---|
| **Pré-Piloto (agora)** | Documentar quais dados são coletados, remover campos desnecessários, garantir que nenhum PII vai a logs | ✅ Feito |
| **MVP-2** | Endpoint de anonimização/eliminação de titular + portabilidade CSV | **Obrigatório antes do lançamento comercial** |
| **MVP-2** | Tela de "Meus Dados" para o cliente (solicitar via lojista) | Obrigatório |
| **MVP-3** | Política de privacidade exibida no onboarding do lojista | Obrigatório |
| **MVP-3** | Notificação de incidente (Art. 48º): log de eventos de segurança com alerta ao lojista | Obrigatório |
| **MVP-3** | Registro de atividades de tratamento (Art. 37º) exportável | Obrigatório |
| **MVP-4** | Relatório de impacto à proteção de dados (DPIA) se processar >50k titulares | Avaliar |

---

## 8. O que NÃO fazer (Proibições diretas da LGPD)

1. ❌ **Nunca** coletar dado pessoal sem base legal (Art. 7º)
2. ❌ **Nunca** usar CPF para fins diferentes de identificação na venda/crediário
3. ❌ **Nunca** compartilhar lista de clientes com terceiros sem consentimento
4. ❌ **Nunca** expor endpoint LGPD falso (stub que diz "deletado" mas não persiste) — **isso foi o erro do agente anterior, já corrigido**
5. ❌ **Nunca** armazenar senha em texto claro ou log
6. ❌ **Nunca** coletar dado de criança sem consentimento dos pais (Art. 14º)
7. ❌ **Nunca** transferir dados de clientes para nuvem sem ciência do lojista (MVP-3 terá opt-in explícito)

---

## 9. Referência Legal

- Lei nº 13.709/2018 (LGPD) — edição Senado Federal 2024
- ANPD — Autoridade Nacional de Proteção de Dados: [anpd.gov.br](https://www.gov.br/anpd)
- Resolução CD/ANPD nº 2/2022 (agentes de tratamento de pequeno porte)
- Art. 55-A a 55-L: estrutura e competências da ANPD
- Art. 52º: multa de até 2% do faturamento no Brasil, limitada a R$ 50 milhões por infração

> [!CAUTION]
> A multa máxima por infração é **R$ 50 milhões**. Para pequeno comércio, o risco reputacional (perda de confiança do cliente) é igualmente grave. Implementar os itens do roadmap LGPD não é opcional.
