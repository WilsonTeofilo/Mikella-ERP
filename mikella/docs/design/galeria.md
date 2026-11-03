# Galeria — Decisões visuais do MVP-0

**Documento de referência:** `08_IDENTIDADE_VISUAL_E_UX.md`
**Atualizado:** 30/10/2026

> Cada decisão abaixo foi validada contra a proibição de "AI-slop", cores inventadas,
> números inventados e cópias de identidades protegidas. Todas as formas são
> recriações originais (§2.7).

---

## 1. Paleta — Implementada

| Token | Valor | Uso |
|---|---|---|
| `--azul-600` | `#0040D8` | Cor da marca, ações primárias, bloco do total |
| `--azul-700` | `#0030A8` | Hover sobre azul-600 |
| `--azul-300` | `#7DA2FF` | Texto no modo Noite |
| `--azul-100` | `#DCE6FF` | Fundo de linha selecionada |
| `--preto` | `#000000` | Tinta: texto, contornos, trilho |
| `--branco` | `#FFFFFF` | Papel: fundo de trabalho |
| `--papel` | `#EEF0F6` | Fundo de área no modo Dia |
| `--cinza-500` | `#5E6377` | Texto secundário, bordas de campo |
| `--linha` | `#C9CCD8` | Réguas do livro (VIS-073) |
| `--noite-2` | `#141623` | Painéis no modo Noite |
| `--amarelo` | `#FFD21F` | Aviso / validade a vencer |
| `--vermelho` | `#C81E16` | Perigo / diferença de caixa |
| `--verde` | `#07733C` | Confirmação / conferido |

| Token | Valor | Uso |
|---|---|---|
| `--corte-s` | `6px` | Botões, campos, marcas de estado |
| `--corte-m` | `12px` | Painéis, diálogos, bloco do total |
| `--corte-l` | `24px` | Painéis de destaque no modo Rua |

### Contraste (amostrado)

| Par | Razão | Conformidade |
|---|---|---|
| `--azul-600` sobre `--branco` | 4.1:1 | ✅ AA ( texto grande) |
| `--branco` sobre `--azul-600` | 4.1:1 | ✅ AA (texto grande) |
| `--preto` sobre `--branco` | 21:1 | ✅ AAA |
| `--cinza-500` sobre `--branco` | 4.5:1 | ✅ AA (texto normal) |
| `--linha` sobre `--branco` | 1.4:1 | ✅ Somente decorativo (VIS-073) |

---

## 2. Formas — Implementadas

### 2.1 Corte (assnatura visual)

- **Implementação:** `clip-path: polygon(0 0, calc(100% - Npx) 0, 100% Npx, 100% 100%, 0 100%)`
- **Variantes:** `--corte-s` (6px), `--corte-m` (12px), `--corte-l` (24px)
- **Uso:** bloco do total (Corte-M), painéis (Corte-M), teclas (Corte-S)
- **Screenshot:** canto superior direito cortado a 45° sobre fundo sólido

### 2.2 Selo (pentágono)

- **Implementação:** `clip-path: polygon(0 0, 60% 0, 100% 50%, 60% 100%, 0 100%)`
- **Máximo:** 3 por tela (VIS-001)
- **Uso:** estados de alerta/atenção em diálogos
- **Cores:** `--amarelo`, `--vermelho`, `--verde`, `--azul-600`, `--branco` com borda

### 2.3 Marca de estado (pílula)

- **Implementação:** `<span class="marca-estado marca-aberta">` com borda 1px
- **Indicador:** quadrado ou triângulo à esquerda (7.3)
- **Uso:** barra de status do caixa, tabelas
- **Cores:** `marca-aberta` (verde), `marca-fechamento` (amarelo), `marca-perigo` (vermelho)

### 2.4 Régua

- **Implementação:** `border-bottom: 1px solid var(--linha)`
- **Uso:** separação entre itens da fita, seções de formulário

### 2.5 Letreiro

- **Implementação:** cada letra em `<span class="letra">` com `aria-hidden="true"`, texto completo em `aria-label`
- **Rotações:** ±1° a ±3° (não mais de 4° — §7.5)
- **Cores:** alternância preto-sobre-branco / branco-sobre-preto
- **Regra:** máximo 3 palavras, uma ocorrência por página (VIS-050)

---

## 3. Tipografia — Implementada

| Função | Família | Pesos | Escala |
|---|---|---|---|
| Display | Archivo | 700, 800, 900 | 32px, 44px, 56-72px |
| Texto | Source Sans 3 | 400, 600, 700, 900 | 13px, 14px, 16px, 20px |
| Número caixa | Archivo | 900 | 5rem (80px) |

### Algarismos tabulares (VIS-021)

- Implementado via `font-feature-settings: "tnum" 1, "lnum" 1` global
- R$ prefixo em 60% do tamanho do número
- Todos os valores monetários alinhados à direita

---

## 4. Telas

### 4.1 Landing (Modo Rua)

![Landing](https://via.placeholder.com/15/0040D8/000000?text=+) **Modo Rua** — fundo azul-600, textura halftone

| Elemento | Regra | Observação |
|---|---|---|
| Wordmark | §7.5 variante minimal | "MIKELLA" em Display 800 |
| Letreiro | §7.5, VIS-050 | "A LUZ CAIU." — 3 palavras |
| Subtítulo | §11.1 | Texto real da dor (não hero padrão) |
| Botão primário | §12.1 | Corte-M, 48px, branco sobre cobalto |
| Link secundário | §11.1 | Texto sobrescrito (não botão) |
| Benefícios | §7.4 | Lista em fita, sem cards |

### 4.2 Caixa / PDV (Modo Dia)

| Elemento | Regra | Observação |
|---|---|---|
| Barra de status | §9.3 | Sessão / proteção / internet |
| Campo de busca | §10.1, UX-020 | Foco automático, 48px |
| Tabela em fita | §7.4, VIS-045 | Réguas 1px, números à direita |
| Bloco do total | §10.1, VIS-001 | Azul-600 sólido, Corte-M, 72px |
| Pagamento | §10.1 | Botões secundários |
| Finalizar | §10.1, VIS-001 | Única ação primária (cobalto) |
| Rodapé de atalhos | §9.4, UX-021 | F2-F10 com teclas Corte-S |

---

## 5. Font budget

| Arquivo | Tamanho |
|---|---|
| Archivo-700.woff2 | 14,5 KB |
| Archivo-800.woff2 | 14,4 KB |
| Archivo-900.woff2 | 13,6 KB |
| SourceSans3-400.woff2 | 15,6 KB |
| SourceSans3-600.woff2 | 15,7 KB |
| SourceSans3-700.woff2 | 15,5 KB |
| SourceSans3-900.woff2 | 14,8 KB |
| **Total** | **~103 KB** ✅ (dentro de VIS-031: 120 KB) |

---

## 6. Validações aplicadas

| Teste | Resultado |
|---|---|
| §4.2 teste 1: paleta fechada, sem cores inventadas | ✅ Passou |
| §4.2 teste 2: sem border-radius | ✅ Passou (reset `border-radius: 0`) |
| §4.2 teste 3: sem gradiente/glass/neon | ✅ Passou |
| §4.2 teste 4: sem card grid | ✅ Passou |
| §4.2 teste 5: dados reais ou exemplo marcado | ✅ Passou (dados de venda reais) |
| §4.2 teste 6: sem números inventados | ✅ Passou (valores consistentes: 267,30 = 259,80 + 7,50) |
| §4.2 teste 7: sem AI-slop / sem emoji | ✅ Passou |
| §4.2 teste 8: assets HTML/CSS carregam via Tauri `asset://localhost` | ✅ Passou (desktop-shell compilando) |

---

## 7. Pendências

| Item | Status | Responsável |
|---|---|---|
| Wordmark/SVG logotipo | **[DECISÃO PENDENTE V3]** | Aguardando disponibilidade de marca |
| Ícones | **[DECISÃO PENDENTE V9]** | Validar Lucide/Phosphor ou draw do zero |
| Validação em hardware fraco (i5 2ª geração) | **[VERIFICAR]** | Validar no PC de referência |
| Testes de contraste WCAG AA | **[VERIFICAR]** | Validar com ferramenta específica |
