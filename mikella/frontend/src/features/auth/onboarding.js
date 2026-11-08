/**
 * Primeiro acesso: cria a identidade da loja e o usuário dono.
 *
 * O backend é a autoridade: aqui só fazemos validação de UX (campo vazio,
 * confirmação de senha) e mostramos a mensagem que o servidor devolver.
 * Sucesso responde 201 `{ onboarded: true, token_pending: true }` e o dono
 * segue para o login normal (com o código 2FA enviado por e-mail).
 */

import { requisitarJSON } from "../../shared/services/api.js";
import { inicializarControlesJanela } from "../../shared/ui/window-controls.js";

inicializarControlesJanela();

const form = document.getElementById("form-onboarding");
const inputNome = document.getElementById("onb-nome");
const inputUsuario = document.getElementById("onb-usuario");
const inputEmail = document.getElementById("onb-email");
const inputSenha = document.getElementById("onb-senha");
const inputConfirmar = document.getElementById("onb-confirmar");
const btnCriar = document.getElementById("btn-criar");
const btnLabel = document.getElementById("btn-label");
const msgErro = document.getElementById("msg-erro");
const msgOk = document.getElementById("msg-ok");

const EMAIL_VALIDO = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const USUARIO_VALIDO = /^[A-Za-z0-9_]{3,64}$/;

function mostrarErro(texto, campo) {
  msgErro.textContent = texto;
  msgErro.style.display = "block";
  msgOk.style.display = "none";
  if (campo) {
    campo.setAttribute("aria-invalid", "true");
    campo.focus();
  }
}

function mostrarOk(texto) {
  msgOk.textContent = texto;
  msgOk.style.display = "block";
  msgErro.style.display = "none";
}

function limparMensagens() {
  msgErro.style.display = "none";
  msgOk.style.display = "none";
  [inputNome, inputUsuario, inputEmail, inputSenha, inputConfirmar].forEach(
    (campo) => campo.removeAttribute("aria-invalid"),
  );
}

/**
 * Validação local mínima para não gastar uma ida ao servidor com campo vazio.
 * As regras definitivas (força de senha, e-mail, unicidade) ficam no backend.
 */
function validarLocalmente() {
  const nome = inputNome.value.trim();
  const usuario = inputUsuario.value.trim();
  const email = inputEmail.value.trim();
  const senha = inputSenha.value;

  if (nome.length < 2) {
    return ["Informe o nome do responsável.", inputNome];
  }
  if (!USUARIO_VALIDO.test(usuario)) {
    return [
      "Usuário deve ter de 3 a 64 caracteres, usando apenas letras, números e underscore.",
      inputUsuario,
    ];
  }
  if (!EMAIL_VALIDO.test(email)) {
    return ["Informe um e-mail válido para receber o código de segurança.", inputEmail];
  }
  if (senha.length < 8) {
    return ["A senha deve ter pelo menos 8 caracteres.", inputSenha];
  }
  if (senha !== inputConfirmar.value) {
    return ["As senhas não conferem.", inputConfirmar];
  }
  return null;
}

async function criarLoja(evento) {
  evento.preventDefault();
  limparMensagens();

  const invalido = validarLocalmente();
  if (invalido) {
    mostrarErro(invalido[0], invalido[1]);
    return;
  }

  btnCriar.disabled = true;
  btnLabel.textContent = "CRIANDO...";

  const corpo = new URLSearchParams({
    nome: inputNome.value.trim(),
    username: inputUsuario.value.trim(),
    email: inputEmail.value.trim(),
    senha: inputSenha.value,
  }).toString();

  try {
    const { ok, status, data } = await requisitarJSON("/v1/onboarding", {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: corpo,
    });

    if (ok && data.onboarded) {
      mostrarOk(
        "Loja criada! Enviamos o primeiro código de segurança para o e-mail informado. Redirecionando para o login...",
      );
      window.setTimeout(() => {
        window.location.href = "landing.html?onboarding=1";
      }, 2600);
      return;
    }

    if (status === 409) {
      mostrarOk("Esta loja já foi configurada. Abrindo o login...");
      window.setTimeout(() => {
        window.location.href = "landing.html";
      }, 1200);
      return;
    }

    mostrarErro(data.erro || "Não foi possível concluir a configuração. Tente novamente.");
  } catch {
    mostrarErro(
      "Servidor não responde em 127.0.0.1:47823. Verifique se o iniciar_loja.bat está aberto.",
    );
  } finally {
    btnCriar.disabled = false;
    btnLabel.textContent = "CRIAR LOJA E USUÁRIO";
  }
}

/**
 * Se a loja já estiver configurada, o primeiro acesso não faz sentido.
 * Falha de rede é ignorada: o envio do formulário mostra o erro real.
 */
async function redirecionarSeJaConfigurada() {
  try {
    const { ok, data } = await requisitarJSON("/v1/onboarding/status");
    if (ok && data.onboarded === true) {
      window.location.href = "landing.html";
    }
  } catch {
    // servidor iniciando — o submit trata a indisponibilidade
  }
}

form.addEventListener("submit", criarLoja);
redirecionarSeJaConfigurada();
