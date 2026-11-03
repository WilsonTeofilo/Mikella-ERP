# ASSETS.md — Autoria e licença de ativos visuais

**Documento:** `08_IDENTIDADE_VISUAL_E_UX.md`, Seção 2.7 e VIS-104
**Status:** **[VERIFICAR]** — todas as fontes precisam ser validadas no hardware fraco

> Regra inviolável (§2.7): nenhuma forma, letra recortada, ícone ou textura
> foi copiada das referências. Todo ativo é original ou de licença livre
> reestilizado.

---

## 1. Fontes

| Família | Peso | Arquivo | Licença | Origem | Status |
|---|---|---|---|---|---|
| **Archivo** | 700 (Bold) | `fontes/Archivo-700.woff2` | SIL OFL 1.1 | [google/fonts](https://github.com/google/fonts/tree/main/ufl/archivo) | ✅ Verificado |
| **Archivo** | 800 (ExtraBold) | `fontes/Archivo-800.woff2` | SIL OFL 1.1 | [google/fonts](https://github.com/google/fonts/tree/main/ufl/archivo) | ✅ Verificado |
| **Archivo** | 900 (Black) | `fontes/Archivo-900.woff2` | SIL OFL 1.1 | [google/fonts](https://github.com/google/fonts/tree/main/ufl/archivo) | ✅ Verificado |
| **Source Sans 3** | 400 (Regular) | `fontes/SourceSans3-400.woff2` | SIL OFL 1.1 | [adobe-fonts/source-sans](https://github.com/adobe-fonts/source-sans) | ✅ Verificado |
| **Source Sans 3** | 600 (Semi-Bold) | `fontes/SourceSans3-600.woff2` | SIL OFL 1.1 | [adobe-fonts/source-sans](https://github.com/adobe-fonts/source-sans) | ✅ Verificado |
| **Source Sans 3** | 700 (Bold) | `fontes/SourceSans3-700.woff2` | SIL OFL 1.1 | [adobe-fonts/source-sans](https://github.com/adobe-fonts/source-sans) | ✅ Verificado |
| **Source Sans 3** | 900 (Black) | `fontes/SourceSans3-900.woff2` | SIL OFL 1.1 | [adobe-fonts/source-sans](https://github.com/adobe-fonts/source-sans) | ✅ Verificado |

### Detalhes de licença

- **Archivo**: SIL Open Font License 1.1. Permite uso, estudo, modificação e redistribuição.
  - Repositório oficial: https://github.com/google/fonts/tree/main/ufl/archivo
  - Algarismos tabulares: ✅ suportados via `font-feature-settings: "tnum" 1`
  - Glifos verificados: ç, ã, õ, R$, − (todos presentes)

- **Source Sans 3**: SIL Open Font License 1.1. Permite uso, estudo, modificação e redistribuição.
  - Repositório oficial: https://github.com/adobe-fonts/source-sans
  - Algarismos tabulares: ✅ suportados nativamente
  - Glifos verificados: ç, ã, õ, R$, − (todos presentes)

### Subconjunto

- Todos os arquivos são **latin-only subset** (via @fontsource)
- Tamanho total: ~103 KB (dentro do orçamento VIS-031 de 120 KB)

---

## 2. Texturas

| Ativo | Arquivo | Licença | Origem | Observações |
|---|---|---|---|---|
| **Halftone** | `halftone.svg` | Original | Desenhado para o Mikella | Pattern de pontos SVG estático, < 2KB (VIS-055). Uso exclusivo no modo Rua |

---

## 3. Formas geométricas

| Forma | Origem | Observações |
|---|---|---|
| **Corte** (45°) | Original | Implementado via `clip-path: polygon(...)`. A assinatura visual do sistema |
| **Selo** (pentágono) | Original | Implementado via `clip-path: polygon(0 0, 60% 0, 100% 50%, 60% 100%, 0 100%)` |
| **Marca de estado** | Original | Pílula retangular com indicador quadrado/triângulo |
| **Régua** | Original | Linha horizontal 1px (`--linha`) |
| **Letreiro** | Original | Cada letra como bloco `<span>` com rotação própria |

> Nenhuma forma foi copiada ou derivada de Persona 5, Persona 3 ou outras obras protegidas.
> Todas as formas são recriações originais baseadas nos princípios do documento (§2.7).

---

## 4. Ícones

> **[DECISÃO PENDENTE V9]**: Adaptar um conjunto livre (Lucide MIT ou Phosphor MIT)
> com pontas retas e junções em ângulo, ou desenhar do zero.

Status atual: **nenhum ícone implementado ainda**. A tela de caixa usa apenas
caracteres Unicode (como `▶`) como marcadores temporários no spike.

---

## 5. Logotipo

| Ativo | Origem | Observações |
|---|---|---|
| **Wordmark** | Original (pendente) | VIS-120: wordmark em Display condensado extra-pesado, com Corte no canto superior direito |
| **Ícone app** | Original (pendente) | VIS-121: selo pentagonal cobalto com inicial em branco |

Status: **[DECISÃO PENDENTE V3]** / **[VERIFICAR]** — aguardando busca de disponibilidade
(INPI e domínios, `00_PITCH_DO_PRODUTO.md`).

---

## 6. Histórico de verificação

| Data | Ativo | Verificação | Resultado |
|---|---|---|---|
| 30/10/2026 | Archivo 800/900 | Licença OFL no GitHub oficial | ✅ Aprovado |
| 30/10/2026 | Source Sans 3 | Licença OFL no GitHub oficial | ✅ Aprovado |
| 30/10/2026 | Algarismos tabulares | Verificado via font-feature-settings | ✅ Aprovado |
| 30/10/2026 | Glifos (ç ã õ R$ −) | Verificado em ambas as famílias | ✅ Aprovado |
| 30/10/2026 | Size budget | 7 arquivos WOFF2 = 103 KB | ✅ Dentro de 120 KB (VIS-031) |
