/**
 * Tela de login / verificação 2FA.
 *
 * Fluxo: usuário + senha -> token pendente -> código de 6 dígitos -> token
 * persistido no localStorage e redirecionamento para o menu principal.
 */

import { requisitarJSON } from "../../shared/services/api.js";
import { inicializarControlesJanela } from "../../shared/ui/window-controls.js";

inicializarControlesJanela();

const form = document.getElementById("form-login");
const inputUser = document.getElementById("input-user");
const inputSenha = document.getElementById("input-senha");
const labelSenha = document.getElementById("label-senha");
const grupoUser = document.getElementById("grupo-user");
const btnSubmit = document.getElementById("btn-submit");
const btnLabel = document.getElementById("btn-label");
const msgErro = document.getElementById("msg-erro");
const msgOk = document.getElementById("msg-ok");

let etapa = "login";
let usernameSalvo = "";

function mostrarErro(texto) {
  msgErro.textContent = texto;
  msgErro.style.display = "block";
  msgOk.style.display = "none";
}

function mostrarOk(texto) {
  msgOk.textContent = texto;
  msgOk.style.display = "block";
  msgErro.style.display = "none";
}

function limparMensagens() {
  msgErro.style.display = "none";
  msgOk.style.display = "none";
}

function voltarParaLogin() {
  etapa = "login";
  grupoUser.style.display = "flex";
  labelSenha.textContent = "SENHA";
  inputSenha.classList.add("campo-senha-mascarado");
  inputSenha.placeholder = "";
  inputSenha.removeAttribute("maxlength");
  inputSenha.value = "";
  btnLabel.textContent = "ENTRAR";
  limparMensagens();
  inputUser.focus();
}

/**
 * Loja sem `instance_identity` ainda não tem usuário: o login não tem como
 * funcionar. Consulta o servidor e encaminha para o primeiro acesso.
 * Falha de rede não bloqueia a tela: o login mostra o erro real.
 */
async function encaminharParaOnboarding() {
  try {
    const { ok, data } = await requisitarJSON("/v1/onboarding/status");
    if (ok && data.onboarded === false) {
      window.location.href = "onboarding.html";
    }
  } catch {
    // servidor iniciando — o submit do login trata a indisponibilidade
  }
}

function avisarOnboardingConcluido() {
  const params = new URLSearchParams(window.location.search);
  if (params.get("onboarding") === "1") {
    mostrarOk("Loja configurada! Entre com o usuário que você acabou de criar.");
  }
}

async function autenticarUsuario(username, senha) {
  const corpo = new URLSearchParams({ username, senha }).toString();
  return requisitarJSON("/v1/login", {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: corpo,
  });
}

async function verificarCodigo2fa(codigo) {
  const corpo = new URLSearchParams({
    username: usernameSalvo,
    codigo,
  }).toString();
  return requisitarJSON("/v1/2fa/verificar", {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: corpo,
  });
}

function entrarEmEtapa2fa(data, usernameDigitado) {
  usernameSalvo = data.username || usernameDigitado;
  etapa = "2fa";
  grupoUser.style.display = "none";
  labelSenha.textContent = "CÓDIGO 2FA";
  inputSenha.classList.remove("campo-senha-mascarado");
  inputSenha.maxLength = 6;
  inputSenha.placeholder = "000000";
  btnLabel.textContent = "VERIFICAR (ENTER)";
  inputSenha.value = "";

  mostrarOk("Código enviado. Digite os 6 dígitos.");
  inputSenha.focus();
}

async function concluirLogin(data) {
  localStorage.setItem("mikella_token", data.token);
  if (data.usuario) {
    localStorage.setItem("mikella_usuario", JSON.stringify(data.usuario));
  }
  mostrarOk("Acesso autorizado!");
  setTimeout(() => {
    window.location.href = "main-menu.html";
  }, 350);
}

form.addEventListener("submit", async (evento) => {
  evento.preventDefault();
  limparMensagens();

  if (etapa === "login") {
    const username = inputUser.value.trim();
    const senha = inputSenha.value;

    if (!username || !senha) {
      mostrarErro("Preencha USER e SENHA.");
      return;
    }

    btnSubmit.disabled = true;
    btnLabel.textContent = "VERIFICANDO...";

    try {
      const { ok, data } = await autenticarUsuario(username, senha);
      if (ok && data.token) {
        // Login direto (quando 2FA estiver desativado no ambiente local de balcão)
        concluirLogin(data);
      } else if (ok && data.token_pending) {
        entrarEmEtapa2fa(data, username);
        if (data.codigo_dev) {
          inputSenha.value = data.codigo_dev;
          mostrarOk(`Código 2FA (Local): ${data.codigo_dev}`);
        }
      } else {
        mostrarErro(data.erro || "Usuário ou senha incorretos.");
      }
    } catch {
      mostrarErro("Servidor não responde em 127.0.0.1:47823.");
    } finally {
      btnSubmit.disabled = false;
      if (etapa === "login") btnLabel.textContent = "ENTRAR";
    }
    return;
  }

  const codigo = inputSenha.value.trim();
  if (codigo.length < 6) {
    mostrarErro("Digite o código de 6 dígitos.");
    return;
  }

  btnSubmit.disabled = true;
  btnLabel.textContent = "VALIDANDO...";

  try {
    const { ok, data } = await verificarCodigo2fa(codigo);
    if (ok && data.token) {
      await concluirLogin(data);
      return;
    }
    mostrarErro(data.erro || "Código inválido ou expirado.");
  } catch {
    mostrarErro("Falha de conexão com a API local.");
  }

  btnSubmit.disabled = false;
  btnLabel.textContent = "VERIFICAR (ENTER)";
});

window.addEventListener("keydown", (evento) => {
  if (evento.key === "Escape" && etapa === "2fa") {
    evento.preventDefault();
    voltarParaLogin();
  }
});

avisarOnboardingConcluido();
encaminharParaOnboarding();
