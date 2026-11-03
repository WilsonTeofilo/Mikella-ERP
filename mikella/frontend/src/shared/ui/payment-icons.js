/**
 * Biblioteca de Ícones de Pagamento Vetoriais Mikella ERP
 * Estilo: Traço grosso ortogonal de alta visibilidade e contraste para PDV
 */

export const ICONES_PAGAMENTO_SVG = {
  // Dinheiro / Cédula com $ central
  DINHEIRO: `
    <svg viewBox="0 0 24 24">
      <rect x="2" y="5" width="20" height="14" rx="0" fill="#E8F5E9" />
      <circle cx="12" cy="12" r="3" fill="#C8E6C9" />
      <path d="M12 9v6M10.5 10.5h3a1 1 0 0 1 0 2h-3a1 1 0 0 0 0 2h3" />
      <line x1="2" y1="9" x2="5" y2="9" />
      <line x1="19" y1="9" x2="22" y2="9" />
      <line x1="2" y1="15" x2="5" y2="15" />
      <line x1="19" y1="15" x2="22" y2="15" />
    </svg>
  `,

  // Cartão de Débito com Chip e Seta Direta
  DEBITO: `
    <svg viewBox="0 0 24 24">
      <rect x="2" y="5" width="20" height="14" fill="#E1F5FE" />
      <line x1="2" y1="10" x2="22" y2="10" stroke-width="3" stroke="#000" />
      <rect x="4" y="13" width="4" height="3" fill="#FFD21F" stroke="#000" stroke-width="1.5" />
      <path d="M15 14l2 2 4-4" stroke-width="2.5" />
    </svg>
  `,

  // Cartão de Crédito com Chip e Tarja
  CREDITO: `
    <svg viewBox="0 0 24 24">
      <rect x="2" y="5" width="20" height="14" fill="#EDE7F6" />
      <line x1="2" y1="9" x2="22" y2="9" stroke-width="3" stroke="#000" />
      <rect x="4" y="12" width="5" height="4" fill="#FFD21F" stroke="#000" stroke-width="1.5" />
      <circle cx="15" cy="14" r="2" fill="#D32F2F" stroke="none" />
      <circle cx="18" cy="14" r="2" fill="#FFA000" stroke="none" />
    </svg>
  `,

  // Pix com Símbolo Geométrico de Losango
  PIX: `
    <svg viewBox="0 0 24 24">
      <rect x="2" y="2" width="20" height="20" fill="#E0F2F1" stroke="none" />
      <path d="M12 4L6 10l6 6 6-6z" fill="#00B4D8" stroke="#000" stroke-width="2" />
      <path d="M12 8l-3 3 3 3 3-3z" fill="#FFF" stroke="#000" stroke-width="1.5" />
      <path d="M9 15l3 3 3-3" stroke-width="2" />
    </svg>
  `,

  // Voucher / Ticket perfurado
  VOUCHER: `
    <svg viewBox="0 0 24 24">
      <rect x="3" y="6" width="18" height="12" fill="#FFF3E0" />
      <line x1="8" y1="6" x2="8" y2="18" stroke-dasharray="2 2" stroke-width="2" />
      <circle cx="3" cy="12" r="2" fill="#FFF" stroke="#000" stroke-width="2" />
      <circle cx="21" cy="12" r="2" fill="#FFF" stroke="#000" stroke-width="2" />
      <path d="M12 10h5M12 14h3" stroke-width="2" />
    </svg>
  `,

  // Crediário / Carnê da Loja
  CREDIARIO: `
    <svg viewBox="0 0 24 24">
      <rect x="4" y="3" width="16" height="18" fill="#FFF9C4" />
      <line x1="8" y1="8" x2="16" y2="8" stroke-width="2" />
      <line x1="8" y1="12" x2="16" y2="12" stroke-width="2" />
      <line x1="8" y1="16" x2="13" y2="16" stroke-width="2" />
      <path d="M16 15l2 2 4-4" stroke-width="2" />
    </svg>
  `,

  // Outro
  OUTRO: `
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="8" fill="#ECEFF1" stroke-width="2" />
      <path d="M12 8v4l3 3" stroke-width="2" />
    </svg>
  `
};

/**
 * Retorna o SVG de um método de pagamento
 */
export function obterIconePagamento(forma) {
  const chave = (forma || "OUTRO").toUpperCase();
  return ICONES_PAGAMENTO_SVG[chave] || ICONES_PAGAMENTO_SVG.OUTRO;
}
