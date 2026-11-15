/**
 * Diálogos modais acessíveis.
 *
 * Centraliza o que cada tela vinha repetindo à mão: abrir/fechar, foco
 * inicial, retorno de foco, fechamento por ESC e ciclo de Tab. O estado
 * visual é `data-aberto="true"`; o CSS da tela decide como exibir.
 */

const pilha = [];

const SELETOR_FOCAVEL = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled]):not([type='hidden'])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

function focaveis(raiz) {
  return Array.from(raiz.querySelectorAll(SELETOR_FOCAVEL)).filter(
    (elemento) => elemento.offsetParent !== null,
  );
}

/** Abre o diálogo e move o foco para o primeiro campo (ou para `focoInicial`). */
export function abrirModal(modal, { focoInicial } = {}) {
  if (!modal || pilha.some((item) => item.modal === modal)) return;

  const anterior = document.activeElement;
  modal.dataset.aberto = "true";
  pilha.push({ modal, anterior });

  const alvo = focoInicial || focaveis(modal)[0] || modal;
  if (alvo === modal && !modal.hasAttribute("tabindex")) {
    modal.setAttribute("tabindex", "-1");
  }
  alvo.focus?.();
}

/** Fecha o diálogo e devolve o foco ao elemento que o abriu. */
export function fecharModal(modal) {
  const indice = pilha.findIndex((item) => item.modal === modal);
  if (indice === -1) return;

  const [{ anterior }] = pilha.splice(indice, 1);
  modal.dataset.aberto = "false";

  // Em telas que recarregam a lista, o botão de origem pode ter saído do DOM.
  if (anterior && document.contains(anterior)) anterior.focus?.();
}

/** Indica se há diálogo aberto (usado para suspender atalhos globais). */
export function modalEstaAberto() {
  return pilha.length > 0;
}

function prenderFoco(evento, modal) {
  const lista = focaveis(modal);
  if (lista.length === 0) {
    evento.preventDefault();
    modal.focus?.();
    return;
  }

  const primeiro = lista[0];
  const ultimo = lista[lista.length - 1];

  if (!modal.contains(document.activeElement)) {
    evento.preventDefault();
    (evento.shiftKey ? ultimo : primeiro).focus();
    return;
  }

  if (evento.shiftKey && document.activeElement === primeiro) {
    evento.preventDefault();
    ultimo.focus();
  } else if (!evento.shiftKey && document.activeElement === ultimo) {
    evento.preventDefault();
    primeiro.focus();
  }
}

// Captura antes dos atalhos globais da página: ESC fecha sem navegar para o
// menu e Tab respeita o ciclo interno do diálogo.
document.addEventListener(
  "keydown",
  (evento) => {
    const topo = pilha[pilha.length - 1];
    if (!topo) return;

    if (evento.key === "Escape") {
      evento.preventDefault();
      evento.stopPropagation();
      fecharModal(topo.modal);
      return;
    }

    if (evento.key === "Tab") prenderFoco(evento, topo.modal);
  },
  true,
);
