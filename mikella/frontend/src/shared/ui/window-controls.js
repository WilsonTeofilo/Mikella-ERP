/**
 * Controles Nativos e Web da Janela Mikella (Tauri 2 + Navegador Fallback)
 * Minimizar (_), Alternar Tela Cheia (F11) e Fechar (✕)
 */
export function inicializarControlesJanela() {
  // Evita duplicar se já existir
  if (document.getElementById("mikella-window-controls")) return;

  const bar = document.createElement("nav");
  bar.id = "mikella-window-controls";
  bar.className = "mikella-window-bar";
  bar.setAttribute("aria-label", "Controles da janela");

  bar.innerHTML = `
    <button id="win-btn-fs" class="mikella-win-btn" type="button" title="Alternar Tela Cheia (F11)">
      <svg viewBox="0 0 24 24"><path d="M7 14H5v5h5v-2H7v-3zm-2-4h2V7h3V5H5v5zm12 7h-3v2h5v-5h-2v3zM14 5v2h3v3h2V5h-5z"/></svg>
      <span id="win-btn-fs-label">TELA CHEIA [F11]</span>
    </button>
    <button id="win-btn-min" class="mikella-win-btn" type="button" title="Minimizar Janela">
      <svg viewBox="0 0 24 24"><path d="M19 13H5v-2h14v2z"/></svg>
      <span>MINIMIZAR</span>
    </button>
    <button id="win-btn-close" class="mikella-win-btn btn-close" type="button" title="Fechar Sistema">
      <svg viewBox="0 0 24 24"><path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/></svg>
      <span>FECHAR</span>
    </button>
  `;

  document.body.appendChild(bar);

  const getTauriWin = () => {
    const api = window.__TAURI__;
    return api?.window?.getCurrentWindow?.()
        || api?.webviewWindow?.getCurrentWebviewWindow?.()
        || null;
  };

  const btnFs = bar.querySelector("#win-btn-fs");
  const btnFsLabel = bar.querySelector("#win-btn-fs-label");
  const btnMin = bar.querySelector("#win-btn-min");
  const btnClose = bar.querySelector("#win-btn-close");

  const atualizarEstadoTelaCheia = (estaEmTelaCheia) => {
    if (estaEmTelaCheia) {
      btnFs.classList.add("fs-active");
      btnFsLabel.textContent = "SAIR TELA CHEIA [F11]";
      btnFs.title = "Sair da Tela Cheia (F11)";
    } else {
      btnFs.classList.remove("fs-active");
      btnFsLabel.textContent = "TELA CHEIA [F11]";
      btnFs.title = "Alternar Tela Cheia (F11)";
    }
  };

  // Alternar Tela Cheia
  async function toggleFullscreen() {
    const win = getTauriWin();
    if (win) {
      try {
        const isFs = await win.isFullscreen();
        await win.setFullscreen(!isFs);
        atualizarEstadoTelaCheia(!isFs);
        return;
      } catch (e) {
        console.warn("Falha no fullscreen nativo do Tauri, usando fallback DOM", e);
      }
    }

    // Fallback padrão navegador / WebView2
    if (!document.fullscreenElement) {
      document.documentElement.requestFullscreen?.().catch(() => {});
      atualizarEstadoTelaCheia(true);
    } else {
      document.exitFullscreen?.().catch(() => {});
      atualizarEstadoTelaCheia(false);
    }
  }

  btnFs.addEventListener("click", toggleFullscreen);

  // Minimizar
  btnMin.addEventListener("click", async () => {
    const win = getTauriWin();
    if (win) {
      await win.minimize();
    } else {
      // Navegador não suporta minimizar aba por JS: exibe feedback
      const toast = document.getElementById("toast");
      if (toast) {
        toast.textContent = "Minimizar disponível no aplicativo desktop nativo.";
        toast.style.display = "block";
        setTimeout(() => { toast.style.display = "none"; }, 2500);
      }
    }
  });

  // Fechar
  btnClose.addEventListener("click", async () => {
    const win = getTauriWin();
    if (win) {
      await win.close();
    } else {
      if (confirm("Deseja realmente encerrar a sessão do Mikella ERP?")) {
        window.close();
      }
    }
  });

  // Evento nativo de fullscreen change
  document.addEventListener("fullscreenchange", () => {
    atualizarEstadoTelaCheia(Boolean(document.fullscreenElement));
  });

  // Atalho global F11
  window.addEventListener("keydown", (e) => {
    if (e.key === "F11") {
      e.preventDefault();
      toggleFullscreen();
    }
  });
}
