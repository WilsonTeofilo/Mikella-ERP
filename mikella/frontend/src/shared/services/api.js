/**
 * Cliente HTTP único do frontend Mikella.
 *
 * Toda chamada à API local passa por aqui para que autenticação, base URL,
 * cabeçalhos e leitura de resposta fiquem em um só lugar. As páginas não
 * devem montar `fetch` manualmente.
 */

const BASE_URL_PADRAO = "http://127.0.0.1:47823";

export const API_URL = (window.MIKELLA_API_URL || BASE_URL_PADRAO).replace(
  /\/+$/,
  "",
);

export function obterToken() {
  return localStorage.getItem("mikella_token");
}

export function obterUsuario() {
  try {
    return JSON.parse(localStorage.getItem("mikella_usuario") || "{}");
  } catch {
    return {};
  }
}

/**
 * Encerra a sessão no backend e limpa o armazenamento local (SEC-011).
 */
export async function deslogar() {
  try {
    await requisitarJSON("/v1/auth/logout", { method: "POST" });
  } catch {
    // Falha de rede não impede a limpeza local
  }
  localStorage.removeItem("mikella_token");
  localStorage.removeItem("mikella_usuario");
  localStorage.removeItem("mikella_sessao_id");
  window.location.href = "landing.html";
}

/**
 * Redireciona para o login quando não há token.
 * @returns {boolean} true quando existe sessão.
 */
export function garantirSessao(destino = "landing.html") {
  if (obterToken()) return true;
  window.location.href = destino;
  return false;
}

export function cabecalhosAutorizacao(extras = {}) {
  const token = obterToken();
  return {
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    ...extras,
  };
}

/**
 * Executa uma requisição e devolve sempre `{ ok, status, data }`.
 * JSON inválido na resposta vira `{}` em vez de exceção.
 */
export async function requisitarJSON(
  caminho,
  { method = "GET", body, headers = {} } = {},
) {
  const cabecalhos = cabecalhosAutorizacao(headers);
  const corpo =
    typeof body === "string"
      ? body
      : body === undefined
        ? undefined
        : JSON.stringify(body);
  const temContentType = Object.keys(cabecalhos).some(
    (chave) => chave.toLowerCase() === "content-type",
  );

  if (corpo !== undefined && !temContentType) {
    cabecalhos["Content-Type"] = "application/json";
  }

  const resposta = await fetch(`${API_URL}${caminho}`, {
    method,
    headers: cabecalhos,
    body: corpo,
  });

  const data = await resposta.json().catch(() => ({}));
  return { ok: resposta.ok, status: resposta.status, data };
}

/**
 * Envia conteúdo bruto (CSV, texto) sem forçar Content-Type JSON.
 */
export async function requisitarTexto(
  caminho,
  { method = "POST", body, contentType = "text/plain" } = {},
) {
  const resposta = await fetch(`${API_URL}${caminho}`, {
    method,
    headers: cabecalhosAutorizacao({ "Content-Type": contentType }),
    body,
  });
  const data = await resposta.json().catch(() => ({}));
  return { ok: resposta.ok, status: resposta.status, data };
}
