/**
 * Painel de diagnóstico e pacote de suporte (OPS-001).
 *
 * Aberto com F10 em qualquer tela operacional. Mostra, em linguagem de loja, o
 * que o servidor verificou (SQLite, livro-caixa, auditoria, schema, fuso) e
 * explica para que serve o `.zip` do pacote de suporte antes de gerá-lo.
 *
 * O painel nunca exibe caminho absoluto, SQL, token ou dado de cliente: a API
 * já devolve só rótulos e contagens (SEC-017). Erros aparecem como texto curto;
 * o detalhe fica no log do servidor.
 */

import { requisitarJSON } from "../services/api.js";
import { mostrarToast } from "./feedback.js";

const ID_PAINEL = "mikella-diagnostico";

let painel = null;
let aberto = false;
let buscando = false;

function elementos() {
  return {
    resumo: painel.querySelector("#diag-resumo"),
    integridade: painel.querySelector("#diag-integridade"),
    livro: painel.querySelector("#diag-livro"),
    auditoria: painel.querySelector("#diag-auditoria"),
    versao: painel.querySelector("#diag-versao"),
    fuso: painel.querySelector("#diag-fuso"),
    produtos: painel.querySelector("#diag-produtos"),
    vendas: painel.querySelector("#diag-vendas"),
    movimentacoes: painel.querySelector("#diag-movimentacoes"),
    auditoriaRegistros: painel.querySelector("#diag-registros-auditoria"),
    pacoteResultado: painel.querySelector("#diag-pacote-resultado"),
  };
}

function criarPainel() {
  const overlay = document.createElement("section");
  overlay.id = ID_PAINEL;
  overlay.className = "diag-overlay";
  overlay.setAttribute("aria-hidden", "true");
  // Marcação estática: nenhum dado dinâmico entra por innerHTML.
  overlay.innerHTML = `
    <div class="diag-caixa" role="dialog" aria-modal="true" aria-labelledby="diag-titulo">
      <header class="diag-topo">
        <div>
          <h2 id="diag-titulo">Diagnóstico do sistema</h2>
          <p class="diag-subtitulo">Saúde do banco local e pacote para a equipe técnica.</p>
        </div>
        <span id="diag-resumo" class="diag-resumo" role="status" aria-live="polite">—</span>
      </header>

      <div class="diag-corpo">
        <section class="diag-bloco">
          <h3>Saúde</h3>
          <ul class="diag-lista">
            <li><span>Banco SQLite</span><strong id="diag-integridade">—</strong></li>
            <li><span>Livro-caixa</span><strong id="diag-livro">—</strong></li>
            <li><span>Auditoria</span><strong id="diag-auditoria">—</strong></li>
            <li><span>Versão / schema</span><strong id="diag-versao">—</strong></li>
            <li><span>Fuso da loja</span><strong id="diag-fuso">—</strong></li>
          </ul>
        </section>

        <section class="diag-bloco">
          <h3>Volume de dados</h3>
          <ul class="diag-lista diag-lista-compacta">
            <li><span>Produtos</span><strong id="diag-produtos">—</strong></li>
            <li><span>Vendas</span><strong id="diag-vendas">—</strong></li>
            <li><span>Movimentações de estoque</span><strong id="diag-movimentacoes">—</strong></li>
            <li><span>Registros de auditoria</span><strong id="diag-registros-auditoria">—</strong></li>
          </ul>
        </section>

        <section class="diag-bloco diag-bloco-suporte">
          <h3>Pacote de suporte (.zip)</h3>
          <p>
            Gera um único arquivo .zip na pasta <code>suporte</code> desta máquina com
            diagnóstico, versão do app, migrações do banco e as últimas linhas do log
            já higienizadas (sem CPF, e-mail, telefone ou token).
          </p>
          <p>
            <strong>Não</strong> inclui banco, backup, chave da loja nem qualquer dado de
            cliente, e nada é enviado pela internet: o arquivo fica só no disco. Gere
            quando a equipe técnica pedir e envie o .zip por fora do sistema.
          </p>
          <p id="diag-pacote-resultado" class="diag-pacote-resultado" role="status" aria-live="polite"></p>
        </section>
      </div>

      <footer class="diag-acoes">
        <button type="button" id="diag-btn-atualizar" class="diag-btn">R · Atualizar</button>
        <button type="button" id="diag-btn-pacote" class="diag-btn primario">P · Gerar pacote</button>
        <button type="button" id="diag-btn-fechar" class="diag-btn fechar">ESC · Fechar</button>
      </footer>
    </div>
  `;
  return overlay;
}

function formatarBytes(bytes) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 KB";
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function texto(valor, padrao = "—") {
  return valor === undefined || valor === null || valor === "" ? padrao : String(valor);
}

async function carregar() {
  if (!painel || buscando) return;
  buscando = true;

  const el = elementos();
  el.resumo.textContent = "Consultando…";
  el.resumo.className = "diag-resumo";

  try {
    const { ok, status, data } = await requisitarJSON("/v1/sistema/diagnostico");
    if (!ok) {
      if (status === 401) {
        window.location.href = "landing.html";
        return;
      }
      el.resumo.textContent =
        status === 403
          ? "Seu perfil não pode ver o diagnóstico"
          : "Falha ao consultar";
      el.resumo.className = "diag-resumo alerta";
      return;
    }

    const banco = data.banco || {};
    const ambiente = data.ambiente || {};
    const estatisticas = banco.estatisticas || {};

    const integro = banco.integrity_check === "ok";
    el.integridade.textContent = integro ? "íntegro" : "falha";
    el.integridade.className = integro ? "estado-ok" : "estado-alerta";

    el.livro.textContent = banco.cadeia_hash_livro_caixa_integra ? "íntegra" : "falha";
    el.livro.className = banco.cadeia_hash_livro_caixa_integra ? "estado-ok" : "estado-alerta";

    el.auditoria.textContent = banco.cadeia_hash_auditoria_integra ? "íntegra" : "falha";
    el.auditoria.className = banco.cadeia_hash_auditoria_integra
      ? "estado-ok"
      : "estado-alerta";

    el.versao.textContent = `${texto(data.app_versao)} · ${texto(banco.sqlite_versao)}`;
    el.fuso.textContent = texto(ambiente.fuso_horario_loja, "padrão do sistema");

    el.produtos.textContent = texto(estatisticas.total_produtos, "0");
    el.vendas.textContent = texto(estatisticas.total_vendas, "0");
    el.movimentacoes.textContent = texto(estatisticas.total_movimentacoes_estoque, "0");
    el.auditoriaRegistros.textContent = texto(estatisticas.total_registros_auditoria, "0");

    el.resumo.textContent = integro ? "Sistema saudável" : "Atenção: banco com falha";
    el.resumo.className = integro ? "diag-resumo ok" : "diag-resumo alerta";
  } catch {
    el.resumo.textContent = "Sem conexão com o servidor local";
    el.resumo.className = "diag-resumo alerta";
  } finally {
    buscando = false;
  }
}

async function gerarPacote() {
  const el = elementos();
  el.pacoteResultado.textContent = "Gerando pacote…";

  try {
    const { ok, status, data } = await requisitarJSON("/v1/sistema/suporte/pacote", {
      method: "POST",
    });
    if (!ok) {
      const mensagem =
        status === 403
          ? "Seu perfil não pode gerar o pacote de suporte."
          : data.erro || "Falha ao gerar o pacote de suporte.";
      el.pacoteResultado.textContent = mensagem;
      mostrarToast(mensagem);
      return;
    }
    const nome = texto(data.arquivo, "suporte.zip");
    const tamanho = formatarBytes(Number(data.tamanho_bytes));
    el.pacoteResultado.textContent = `Gerado: ${nome} (${tamanho}) na pasta suporte.`;
    mostrarToast(`Pacote gerado: ${nome} (${tamanho})`);
  } catch {
    el.pacoteResultado.textContent = "Erro de comunicação com a API.";
    mostrarToast("Erro de comunicação com a API.");
  }
}

function abrir() {
  if (!painel || aberto) return;
  aberto = true;
  painel.classList.add("aberto");
  painel.setAttribute("aria-hidden", "false");
  carregar();
  const fechar = painel.querySelector("#diag-btn-fechar");
  if (fechar) fechar.focus();
}

function fechar() {
  if (!painel || !aberto) return;
  aberto = false;
  painel.classList.remove("aberto");
  painel.setAttribute("aria-hidden", "true");
}

/**
 * Instala o painel na página atual e liga o atalho F10.
 * Idempotente: chamar duas vezes não duplica o painel nem o listener.
 */
export function inicializarDiagnostico() {
  if (painel && document.getElementById(ID_PAINEL)) return;

  painel = criarPainel();
  document.body.append(painel);

  painel.querySelector("#diag-btn-atualizar").addEventListener("click", carregar);
  painel.querySelector("#diag-btn-pacote").addEventListener("click", gerarPacote);
  painel.querySelector("#diag-btn-fechar").addEventListener("click", fechar);

  painel.addEventListener("pointerdown", (evento) => {
    if (evento.target === painel) fechar();
  });

  // Captura antes dos atalhos da tela: Esc fecha o painel em vez de sair da
  // página e as letras R/P não disparam ações do menu por baixo.
  document.addEventListener(
    "keydown",
    (evento) => {
      if (evento.key === "F10" && !evento.repeat) {
        evento.preventDefault();
        evento.stopPropagation();
        if (aberto) fechar();
        else abrir();
        return;
      }
      if (!aberto) return;

      // Com o painel aberto, nenhum atalho da tela de fundo deve disparar.
      evento.stopPropagation();

      if (evento.key === "Escape") {
        evento.preventDefault();
        fechar();
      } else if (evento.key === "r" || evento.key === "R") {
        evento.preventDefault();
        carregar();
      } else if (evento.key === "p" || evento.key === "P") {
        evento.preventDefault();
        gerarPacote();
      }
    },
    { capture: true },
  );
}
