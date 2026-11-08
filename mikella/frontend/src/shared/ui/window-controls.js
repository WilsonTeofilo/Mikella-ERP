/**
 * Controles da janela Mikella (Tauri 2 com fallback para navegador).
 *
 * Responsabilidades:
 * - injetar a barra de controles uma única vez por documento;
 * - minimizar, alternar tela cheia e fechar usando a janela real do Tauri;
 * - refletir o estado real da janela (nunca apenas uma variável local);
 * - em tela cheia, ocultar os controles e revelá-los ao aproximar o ponteiro
 *   da extremidade superior direita ou ao navegar por teclado.
 *
 * A API global (`window.__TAURI__`) exige `app.withGlobalTauri: true` e as
 * permissões `core:window:allow-close|minimize|is-fullscreen|set-fullscreen`
 * declaradas na capability da janela.
 */

import { mostrarToast } from "./feedback.js";

const ID_BARRA = "mikella-window-controls";
const ID_SENSOR = "mikella-win-sensor";
const ATRASO_OCULTAR_MS = 900;
const INTERVALO_SINCRONIA_MS = 120;

function obterJanelaNativa() {
  const api = window.__TAURI__;
  return (
    api?.window?.getCurrentWindow?.() ||
    api?.webviewWindow?.getCurrentWebviewWindow?.() ||
    null
  );
}

function avisar(mensagem) {
  mostrarToast(mensagem, { duracao: 3200 });
}

function criarBarra() {
  const barra = document.createElement("nav");
  barra.id = ID_BARRA;
  barra.className = "mikella-window-bar";
  barra.setAttribute("aria-label", "Controles da janela");
  // A janela não tem decoração nativa: a barra serve como área de arraste.
  // O atributo é lido no próprio elemento, então os botões continuam clicáveis.
  barra.setAttribute("data-tauri-drag-region", "");
  barra.innerHTML = `
    <button id="win-btn-fs" class="mikella-win-btn" type="button"
            aria-label="Alternar tela cheia (F11)" aria-pressed="false" title="Tela cheia (F11)">
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M7 14H5v5h5v-2H7v-3zm-2-4h2V7h3V5H5v5zm12 7h-3v2h5v-5h-2v3zM14 5v2h3v3h2V5h-5z"/>
      </svg>
      <span id="win-btn-fs-label">TELA CHEIA [F11]</span>
    </button>
    <button id="win-btn-min" class="mikella-win-btn" type="button"
            aria-label="Minimizar janela" title="Minimizar">
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M19 13H5v-2h14v2z"/>
      </svg>
      <span>MINIMIZAR</span>
    </button>
    <button id="win-btn-close" class="mikella-win-btn btn-close" type="button"
            aria-label="Fechar sistema" title="Fechar">
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
      </svg>
      <span>FECHAR</span>
    </button>
  `;
  return barra;
}

function criarSensor() {
  const sensor = document.createElement("div");
  sensor.id = ID_SENSOR;
  sensor.className = "mikella-win-sensor";
  sensor.setAttribute("aria-hidden", "true");
  return sensor;
}

export function inicializarControlesJanela() {
  if (document.getElementById(ID_BARRA)) return null;

  const barra = criarBarra();
  const sensor = criarSensor();
  document.body.append(sensor, barra);

  const botaoTelaCheia = barra.querySelector("#win-btn-fs");
  const rotuloTelaCheia = barra.querySelector("#win-btn-fs-label");
  const botaoMinimizar = barra.querySelector("#win-btn-min");
  const botaoFechar = barra.querySelector("#win-btn-close");

  let transicaoEmAndamento = false;
  let timerOcultar = null;
  let timerSincronia = null;

  function revelar() {
    if (!document.body.classList.contains("em-tela-cheia")) return;
    barra.classList.add("revelada");
  }

  function agendarOcultar() {
    if (timerOcultar) clearTimeout(timerOcultar);
    timerOcultar = setTimeout(() => {
      timerOcultar = null;
      const ponteiroNaBarra = barra.matches(":hover");
      const focoNaBarra = barra.contains(document.activeElement);
      if (!ponteiroNaBarra && !focoNaBarra) barra.classList.remove("revelada");
    }, ATRASO_OCULTAR_MS);
  }

  function aplicarEstadoVisual(emTelaCheia) {
    document.body.classList.toggle("em-tela-cheia", emTelaCheia);
    botaoTelaCheia.classList.toggle("fs-active", emTelaCheia);
    botaoTelaCheia.setAttribute("aria-pressed", String(emTelaCheia));
    botaoTelaCheia.setAttribute(
      "aria-label",
      emTelaCheia ? "Sair da tela cheia (F11)" : "Entrar em tela cheia (F11)",
    );
    botaoTelaCheia.title = emTelaCheia
      ? "Sair da tela cheia (F11)"
      : "Tela cheia (F11)";
    rotuloTelaCheia.textContent = emTelaCheia
      ? "SAIR DA TELA CHEIA [F11]"
      : "TELA CHEIA [F11]";

    if (emTelaCheia) {
      // O foco no botão manteria a barra visível pelo :focus-within.
      if (barra.contains(document.activeElement)) document.activeElement.blur();
    } else {
      barra.classList.remove("revelada");
    }
  }

  async function lerEstadoTelaCheia() {
    const janela = obterJanelaNativa();
    if (janela) {
      try {
        return await janela.isFullscreen();
      } catch (erro) {
        console.warn(
          "[mikella] isFullscreen falhou, usando estado do documento",
          erro,
        );
      }
    }
    return Boolean(document.fullscreenElement);
  }

  async function sincronizar() {
    const emTelaCheia = await lerEstadoTelaCheia();
    aplicarEstadoVisual(Boolean(emTelaCheia));
    return Boolean(emTelaCheia);
  }

  function agendarSincronia() {
    if (timerSincronia) clearTimeout(timerSincronia);
    timerSincronia = setTimeout(() => {
      timerSincronia = null;
      sincronizar();
    }, INTERVALO_SINCRONIA_MS);
  }

  async function alternarTelaCheia() {
    if (transicaoEmAndamento) return;
    transicaoEmAndamento = true;

    try {
      const janela = obterJanelaNativa();
      const estaEmTelaCheia = await lerEstadoTelaCheia();
      const alvo = !estaEmTelaCheia;

      if (janela) {
        await janela.setFullscreen(alvo);
      } else if (alvo) {
        await document.documentElement.requestFullscreen();
      } else if (document.fullscreenElement) {
        await document.exitFullscreen();
      }

      aplicarEstadoVisual(alvo);
    } catch (erro) {
      console.error("[mikella] Falha ao alternar tela cheia", erro);
      avisar("Não foi possível alterar o modo de tela cheia.");
      await sincronizar();
    } finally {
      transicaoEmAndamento = false;
    }
  }

  botaoTelaCheia.addEventListener("click", alternarTelaCheia);

  botaoMinimizar.addEventListener("click", async () => {
    const janela = obterJanelaNativa();
    if (!janela) {
      avisar("Minimizar está disponível apenas no aplicativo desktop.");
      return;
    }
    try {
      await janela.minimize();
    } catch (erro) {
      console.error("[mikella] Falha ao minimizar", erro);
      avisar("Não foi possível minimizar a janela.");
    }
  });

  botaoFechar.addEventListener("click", async () => {
    const janela = obterJanelaNativa();
    if (janela) {
      try {
        await janela.close();
      } catch (erro) {
        console.error("[mikella] Falha ao fechar", erro);
        avisar("Não foi possível fechar a janela.");
      }
      return;
    }

    if (window.confirm("Deseja realmente encerrar a sessão do Mikella ERP?")) {
      window.close();
      setTimeout(
        () => avisar("Feche a aba pelo navegador para encerrar."),
        400,
      );
    }
  });

  sensor.addEventListener("pointerenter", revelar);
  sensor.addEventListener("pointerleave", agendarOcultar);
  barra.addEventListener("pointerenter", revelar);
  barra.addEventListener("pointerleave", agendarOcultar);
  barra.addEventListener("focusin", revelar);
  barra.addEventListener("focusout", agendarOcultar);

  document.addEventListener("fullscreenchange", sincronizar);
  window.addEventListener("resize", agendarSincronia);
  window.addEventListener("focus", agendarSincronia);

  window.addEventListener("keydown", (evento) => {
    if (evento.key !== "F11" || evento.repeat) return;
    evento.preventDefault();
    alternarTelaCheia();
  });

  sincronizar();

  return { sincronizar, alternarTelaCheia };
}
