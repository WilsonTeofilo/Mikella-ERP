import { garantirSessao, requisitarJSON } from "../../shared/services/api.js";
import { mostrarToast } from "../../shared/ui/feedback.js";
import { inicializarControlesJanela } from "../../shared/ui/window-controls.js";
import { inicializarDiagnostico } from "../../shared/ui/diagnostico.js";

inicializarControlesJanela();
inicializarDiagnostico();
garantirSessao();

let usuarioAtual = null;
let indiceSelecionado = 0;
let itensVisiveis = [];

const avisar = mostrarToast;

// Catálogo mestre de seções do sistema. O painel do dono vive dentro do
// Financeiro (visão geral), então o menu começa pelo Caixa.
const TODAS_SECOES = [
  {
    id: "caixa",
    titulo: "CAIXA",
    subtitulo: "PDV / VENDER",
    rota: "pos-menu.html",
    permissao: ["DONO", "GERENTE", "OPERADOR_CAIXA", "VENDEDOR"],
  },
  {
    id: "produtos",
    titulo: "PRODUTOS",
    subtitulo: "ESTOQUE & CSV",
    rota: "produtos.html",
    permissao: ["DONO", "GERENTE", "ESTOQUISTA"],
  },
  {
    id: "financeiro",
    titulo: "FINANCEIRO",
    subtitulo: "VISÃO, CONTAS & PARÂMETROS",
    rota: "financeiro.html",
    permissao: ["DONO", "GERENTE"],
  },
  {
    id: "backup",
    titulo: "BACKUP LOCAL",
    subtitulo: "CÓPIA 1-CLIQUE",
    acao: executarBackupLocal,
    permissao: ["DONO", "GERENTE"],
  },
  {
    id: "sair",
    titulo: "DESCONECTAR",
    subtitulo: "TROCAR USUÁRIO",
    acao: solicitarConfirmacaoLogout,
    permissao: ["DONO", "GERENTE", "OPERADOR_CAIXA", "ESTOQUISTA", "VENDEDOR"],
  },
];

async function carregarDadosUsuario() {
  // Tenta recuperar do localStorage primeiro
  const local = localStorage.getItem("mikella_usuario");
  if (local) {
    try {
      usuarioAtual = JSON.parse(local);
      atualizarHUD();
      construirMenu();
    } catch (_) {}
  }

  // Valida e atualiza com o backend via /v1/auth/me
  try {
    const { ok, status, data } = await requisitarJSON("/v1/auth/me");
    if (ok) {
      usuarioAtual = data;
      localStorage.setItem("mikella_usuario", JSON.stringify(usuarioAtual));
      atualizarHUD();
      construirMenu();
    } else if (status === 401) {
      localStorage.removeItem("mikella_token");
      window.location.href = "landing.html";
    }
  } catch {
    if (!usuarioAtual) {
      avisar("Sem conexão com o servidor local.");
    }
  }
}

function atualizarHUD() {
  if (!usuarioAtual) return;
  document.getElementById("usuario-nome").textContent =
    usuarioAtual.nome || usuarioAtual.username;
  document.getElementById("usuario-papel").textContent =
    usuarioAtual.papel || "OPERADOR";
}

function construirMenu() {
  const papel =
    usuarioAtual && usuarioAtual.papel
      ? usuarioAtual.papel.toUpperCase()
      : "OPERADOR_CAIXA";
  const container = document.getElementById("menu-lista");
  container.replaceChildren();

  // Filtra as seções de acordo com a permissão estrita
  itensVisiveis = TODAS_SECOES.filter((secao) =>
    secao.permissao.includes(papel),
  );

  itensVisiveis.forEach((secao, idx) => {
    const itemEl = document.createElement("div");
    itemEl.className = `p3-menu-item ${idx === indiceSelecionado ? "ativo" : ""}`;
    itemEl.dataset.idx = idx;

    const textoTitulo = document.createElement("span");
    textoTitulo.className = "p3-menu-texto";
    textoTitulo.textContent = secao.titulo;

    const textoAtalho = document.createElement("span");
    textoAtalho.className = "p3-menu-atalho";
    textoAtalho.textContent = secao.subtitulo;

    itemEl.append(textoTitulo, textoAtalho);

    itemEl.addEventListener("mouseenter", () => {
      selecionarItem(idx);
    });

    itemEl.addEventListener("click", () => {
      selecionarItem(idx);
      confirmarSelecao();
    });

    container.appendChild(itemEl);
  });

  atualizarSelecaoVisual();
}

function selecionarItem(idx) {
  if (itensVisiveis.length === 0) return;
  indiceSelecionado = (idx + itensVisiveis.length) % itensVisiveis.length;
  atualizarSelecaoVisual();
}

function atualizarSelecaoVisual() {
  const elementos = document.querySelectorAll(".p3-menu-item");
  elementos.forEach((el, i) => {
    if (i === indiceSelecionado) {
      el.classList.add("ativo");
    } else {
      el.classList.remove("ativo");
    }
  });
}

function confirmarSelecao() {
  if (!itensVisiveis[indiceSelecionado]) return;
  const secao = itensVisiveis[indiceSelecionado];

  if (secao.rota) {
    window.location.href = secao.rota;
  } else if (secao.acao) {
    secao.acao();
  }
}


function solicitarConfirmacaoLogout() {
  abrirModal(
    "DESCONECTAR DO SISTEMA",
    "Deseja realmente sair e desconectar este terminal? Pressione ENTER para confirmar ou ESC para continuar.",
    desconectar,
    "SIM, DESCONECTAR (ENTER)"
  );
}

function desconectar() {
  localStorage.removeItem("mikella_token");
  localStorage.removeItem("mikella_usuario");
  localStorage.removeItem("mikella_sessao_id");
  window.location.href = "landing.html";
}

// Ações dos painéis administrativos
async function executarBackupLocal() {
  abrirModal(
    "GERAR CÓPIA DE SEGURANÇA",
    "Esta ação criará um arquivo de backup consistente com hash BLAKE3 na pasta de dados da loja.",
    async () => {
      avisar("Gerando cópia de segurança...");
      try {
        const { ok, data } = await requisitarJSON("/v1/sistema/backup-local", {
          method: "POST",
          body: {},
        });
        if (ok) {
          avisar(`Backup gerado com sucesso! Arquivo: ${data.arquivo}`);
        } else {
          avisar(data.erro || "Falha ao gerar backup.");
        }
      } catch {
        avisar("Erro de comunicação com a API.");
      }
    },
  );
}

// Modal de diálogo
const modalAcao = document.getElementById("modal-acao");
const modalTitulo = document.getElementById("modal-titulo");
const modalDescricao = document.getElementById("modal-descricao");
const modalBtnConfirmar = document.getElementById("modal-btn-confirmar");
const modalBtnCancelar = document.getElementById("modal-btn-cancelar");
let acaoModalAtual = null;

function abrirModal(titulo, desc, onConfirm, textoConfirmar = "EXECUTAR") {
  modalTitulo.textContent = titulo;
  modalDescricao.textContent = desc;
  modalBtnConfirmar.textContent = textoConfirmar;
  acaoModalAtual = onConfirm;
  modalAcao.style.display = "flex";
  modalBtnConfirmar.focus();
}

function fecharModal() {
  modalAcao.style.display = "none";
  acaoModalAtual = null;
}

modalBtnConfirmar.onclick = () => {
  if (acaoModalAtual) acaoModalAtual();
  fecharModal();
};

modalBtnCancelar.onclick = fecharModal;

// Controles de Teclado Estilo Jogo Persona
window.addEventListener("keydown", (e) => {
  // Se modal estiver aberto
  if (modalAcao.style.display === "flex") {
    if (e.key === "Escape") {
      e.preventDefault();
      fecharModal();
    } else if (e.key === "Enter") {
      e.preventDefault();
      modalBtnConfirmar.click();
    }
    return;
  }

  if (e.key === "ArrowDown" || e.key === "s" || e.key === "S") {
    e.preventDefault();
    selecionarItem(indiceSelecionado + 1);
  } else if (e.key === "ArrowUp" || e.key === "w" || e.key === "W") {
    e.preventDefault();
    selecionarItem(indiceSelecionado - 1);
  } else if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    confirmarSelecao();
  } else if (e.key === "Escape") {
    e.preventDefault();
    solicitarConfirmacaoLogout();
  }
});

carregarDadosUsuario();
