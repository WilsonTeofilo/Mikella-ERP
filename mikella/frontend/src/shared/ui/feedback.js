/**
 * Feedback visual compartilhado.
 *
 * O elemento `#toast` existe em todas as páginas operacionais, mas cada uma
 * tem um CSS próprio (classe `.visivel` ou `display`). Aplicamos os dois
 * mecanismos para manter compatibilidade sem duplicar código.
 */

let timerAtual = null;

export function mostrarToast(mensagem, { duracao = 2500 } = {}) {
  const elemento = document.getElementById("toast");
  if (!elemento) {
    console.warn("[mikella]", mensagem);
    return;
  }

  elemento.textContent = mensagem;
  elemento.style.display = "block";
  elemento.classList.add("visivel");

  if (timerAtual) clearTimeout(timerAtual);
  timerAtual = setTimeout(() => {
    elemento.classList.remove("visivel");
    elemento.style.display = "none";
    timerAtual = null;
  }, duracao);
}
