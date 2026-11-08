/**
 * Ícones vetoriais das formas de pagamento — fonte única de verdade.
 *
 * Regras de desenho:
 * - cada forma declara `fill` e `stroke` explicitamente; nunca depende de
 *   herança global de CSS (foi o que deixou os ícones pretos/sem desenho);
 * - o retângulo de fundo usa tom pastel; o traço é preto e grosso para
 *   leitura a distância no PDV;
 * - `viewBox="0 0 24 24"` e `aria-hidden="true"` em todos.
 */

export const FORMAS_PAGAMENTO = [
  { chave: "DINHEIRO", rotulo: "Dinheiro", tecla: "F1" },
  { chave: "DEBITO", rotulo: "Débito", tecla: "F2" },
  { chave: "CREDITO", rotulo: "Crédito", tecla: "F3" },
  { chave: "PIX", rotulo: "Pix", tecla: "F4" },
  { chave: "VOUCHER", rotulo: "Voucher", tecla: "F5" },
  { chave: "CREDIARIO", rotulo: "Crediário", tecla: "F6" },
];

const ABERTURA =
  '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">';

export const ICONES_PAGAMENTO_SVG = {
  // Cédula com símbolo de cifrão.
  DINHEIRO: `${ABERTURA}
      <rect x="2" y="5" width="20" height="14" fill="#E8F5E9" stroke="#000000" stroke-width="2"/>
      <circle cx="12" cy="12" r="3" fill="#C8E6C9" stroke="#000000" stroke-width="1.5"/>
      <path d="M12 9v6M10.5 10.5h3a1 1 0 0 1 0 2h-3a1 1 0 0 0 0 2h3" fill="none" stroke="#000000" stroke-width="1.5"/>
      <line x1="2" y1="9" x2="5" y2="9" fill="none" stroke="#000000" stroke-width="1.5"/>
      <line x1="19" y1="9" x2="22" y2="9" fill="none" stroke="#000000" stroke-width="1.5"/>
      <line x1="2" y1="15" x2="5" y2="15" fill="none" stroke="#000000" stroke-width="1.5"/>
      <line x1="19" y1="15" x2="22" y2="15" fill="none" stroke="#000000" stroke-width="1.5"/>
    </svg>`,

  // Cartão de débito com chip e confirmação.
  DEBITO: `${ABERTURA}
      <rect x="2" y="5" width="20" height="14" fill="#E1F5FE" stroke="#000000" stroke-width="2"/>
      <line x1="2" y1="9" x2="22" y2="9" fill="none" stroke="#000000" stroke-width="3"/>
      <rect x="4" y="12" width="4" height="3" fill="#FFD21F" stroke="#000000" stroke-width="1.5"/>
      <path d="M16 13l2 2 3-3" fill="none" stroke="#000000" stroke-width="2"/>
    </svg>`,

  // Cartão de crédito com chip e bandeiras.
  CREDITO: `${ABERTURA}
      <rect x="2" y="5" width="20" height="14" fill="#EDE7F6" stroke="#000000" stroke-width="2"/>
      <line x1="2" y1="9" x2="22" y2="9" fill="none" stroke="#000000" stroke-width="3"/>
      <rect x="4" y="12" width="4" height="3" fill="#FFD21F" stroke="#000000" stroke-width="1.5"/>
      <circle cx="15" cy="14" r="2" fill="#D32F2F" stroke="none"/>
      <circle cx="18" cy="14" r="2" fill="#FFA000" stroke="none"/>
    </svg>`,

  // Pix: losango geométrico.
  PIX: `${ABERTURA}
      <rect x="2" y="2" width="20" height="20" fill="#E0F2F1" stroke="#000000" stroke-width="2"/>
      <path d="M12 4L6 10l6 6 6-6z" fill="#00B4D8" stroke="#000000" stroke-width="2"/>
      <path d="M12 8l-3 3 3 3 3-3z" fill="#FFFFFF" stroke="#000000" stroke-width="1.5"/>
    </svg>`,

  // Voucher: ticket perfurado.
  VOUCHER: `${ABERTURA}
      <rect x="3" y="6" width="18" height="12" fill="#FFF3E0" stroke="#000000" stroke-width="2"/>
      <line x1="8" y1="6" x2="8" y2="18" fill="none" stroke="#000000" stroke-width="2" stroke-dasharray="2 2"/>
      <circle cx="3" cy="12" r="2" fill="#FFFFFF" stroke="#000000" stroke-width="2"/>
      <circle cx="21" cy="12" r="2" fill="#FFFFFF" stroke="#000000" stroke-width="2"/>
      <path d="M12 10h5M12 14h3" fill="none" stroke="#000000" stroke-width="2"/>
    </svg>`,

  // Crediário: carnê da loja.
  CREDIARIO: `${ABERTURA}
      <rect x="4" y="3" width="16" height="18" fill="#FFF9C4" stroke="#000000" stroke-width="2"/>
      <line x1="8" y1="8" x2="16" y2="8" fill="none" stroke="#000000" stroke-width="2"/>
      <line x1="8" y1="12" x2="16" y2="12" fill="none" stroke="#000000" stroke-width="2"/>
      <line x1="8" y1="16" x2="13" y2="16" fill="none" stroke="#000000" stroke-width="2"/>
      <path d="M15 15l2 2 3-3" fill="none" stroke="#000000" stroke-width="2"/>
    </svg>`,

  OUTRO: `${ABERTURA}
      <circle cx="12" cy="12" r="8" fill="#ECEFF1" stroke="#000000" stroke-width="2"/>
      <path d="M12 8v4l3 3" fill="none" stroke="#000000" stroke-width="2"/>
    </svg>`,
};

export function obterIconePagamento(forma) {
  const chave = String(forma || "OUTRO").toUpperCase();
  return ICONES_PAGAMENTO_SVG[chave] || ICONES_PAGAMENTO_SVG.OUTRO;
}

/**
 * Monta a grade de seleção rápida dentro de `container`.
 * @param {HTMLElement} container
 * @param {(forma: string) => void} aoSelecionar
 */
export function montarGradePagamentos(container, aoSelecionar) {
  if (!container) return;

  container.replaceChildren();

  FORMAS_PAGAMENTO.forEach(({ chave, rotulo, tecla }) => {
    const botao = document.createElement("button");
    botao.type = "button";
    botao.className = "card-forma-pagamento";
    botao.dataset.addForma = chave;
    botao.setAttribute("aria-label", `Adicionar pagamento em ${rotulo}`);

    const atalho = document.createElement("span");
    atalho.className = "badge-atalho";
    atalho.textContent = tecla;

    const icone = document.createElement("span");
    icone.className = "icone-svg";
    // SVG constante interno; a chave vem da lista fixa FORMAS_PAGAMENTO.
    icone.innerHTML = obterIconePagamento(chave);

    const rotuloEl = document.createElement("span");
    rotuloEl.className = "rotulo-forma";
    rotuloEl.textContent = rotulo;

    botao.append(atalho, icone, rotuloEl);
    container.appendChild(botao);

    if (typeof aoSelecionar === "function") {
      botao.addEventListener("click", () => aoSelecionar(chave));
    }
  });
}
