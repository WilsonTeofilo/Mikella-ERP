/**
 * Consulta rápida de preço — somente leitura.
 *
 * Usada pelo menu do PDV (F9) e pela tela de venda (F6). Não cria item,
 * não altera a venda e não depende do caixa aberto. O catálogo vem de
 * `GET /v1/produtos`; o preço é sempre o inteiro em centavos devolvido pelo
 * backend — o módulo apenas formata para exibição.
 */

import { requisitarJSON } from "../../shared/services/api.js";
import { mostrarToast } from "../../shared/ui/feedback.js";
import { formatarCentavos, normalizarBusca } from "../../shared/ui/formatadores.js";

const MAX_RESULTADOS = 40;
const MIN_CARACTERES = 2;

let instancia = null;

function criarElemento(tag, classe, texto) {
  const el = document.createElement(tag);
  if (classe) el.className = classe;
  if (texto !== undefined) el.textContent = texto;
  return el;
}

function codigoExibido(produto) {
  const codigo = produto.codigo_barras || produto.sku;
  return codigo ? `COD: ${codigo}` : "Sem código cadastrado";
}

function montarOverlay() {
  const overlay = criarElemento("div", "consulta-preco-overlay");
  overlay.id = "consulta-preco-overlay";
  overlay.setAttribute("role", "dialog");
  overlay.setAttribute("aria-modal", "true");
  overlay.setAttribute("aria-labelledby", "consulta-preco-titulo");

  const painel = criarElemento("div", "consulta-preco-painel");

  const cabecalho = criarElemento("header", "consulta-preco-cabecalho");
  const blocoTitulo = criarElemento("div", "consulta-preco-bloco-titulo");
  const titulo = criarElemento("h2", "consulta-preco-titulo", "Preço do produto");
  titulo.id = "consulta-preco-titulo";
  blocoTitulo.append(
    criarElemento("span", "consulta-preco-etiqueta", "CONSULTA RÁPIDA"),
    titulo,
  );

  const btnFechar = criarElemento("button", "consulta-preco-fechar", "Esc");
  btnFechar.type = "button";
  btnFechar.setAttribute("aria-label", "Fechar consulta de preço");

  cabecalho.append(blocoTitulo, btnFechar);

  const rotulo = criarElemento(
    "label",
    "consulta-preco-rotulo",
    "Bipe o código de barras ou digite o nome / SKU",
  );
  rotulo.setAttribute("for", "consulta-preco-busca");

  const input = document.createElement("input");
  input.id = "consulta-preco-busca";
  input.className = "consulta-preco-busca";
  input.type = "text";
  input.autocomplete = "off";
  input.placeholder = "Código de barras, nome ou SKU...";

  const status = criarElemento(
    "p",
    "consulta-preco-status",
    "Digite ao menos 2 caracteres ou bipe o código.",
  );
  status.setAttribute("aria-live", "polite");

  const destaque = criarElemento("section", "consulta-preco-destaque");
  destaque.hidden = true;
  const destaqueNome = criarElemento("h3", "consulta-preco-destaque-nome");
  const destaqueCodigo = criarElemento("p", "consulta-preco-destaque-codigo");
  const destaquePreco = criarElemento("div", "consulta-preco-destaque-preco");
  const destaqueMeta = criarElemento("p", "consulta-preco-destaque-meta");
  destaque.append(destaqueNome, destaqueCodigo, destaquePreco, destaqueMeta);

  const lista = criarElemento("div", "consulta-preco-lista");
  lista.setAttribute("role", "listbox");
  lista.setAttribute("aria-label", "Produtos encontrados");

  const rodape = criarElemento(
    "p",
    "consulta-preco-rodape",
    "Consulta apenas — nenhum item é adicionado à venda.",
  );

  painel.append(cabecalho, rotulo, input, status, destaque, lista, rodape);
  overlay.appendChild(painel);

  return {
    overlay,
    input,
    status,
    destaque,
    destaqueNome,
    destaqueCodigo,
    destaquePreco,
    destaqueMeta,
    lista,
    btnFechar,
  };
}

/**
 * Cria (uma única vez) o modal de consulta e devolve a API da tela.
 *
 * @returns {{ abrir: (termoInicial?: string) => Promise<void>, fechar: () => void, estaAberta: () => boolean }}
 */
export function inicializarConsultaPreco() {
  if (instancia) return instancia;

  const dom = montarOverlay();
  document.body.appendChild(dom.overlay);

  let produtos = null;
  let promessaProdutos = null;
  let resultados = [];
  let indiceSelecionado = -1;
  let focoAnterior = null;

  function estaAberta() {
    return dom.overlay.classList.contains("aberto");
  }

  function definirStatus(texto) {
    dom.status.textContent = texto;
  }

  function limparResultados() {
    dom.lista.replaceChildren();
    resultados = [];
    indiceSelecionado = -1;
    dom.destaque.hidden = true;
  }

  function carregarProdutos() {
    if (promessaProdutos) return promessaProdutos;
    promessaProdutos = requisitarJSON("/v1/produtos")
      .then(({ ok, data }) => {
        if (!ok || !data || !Array.isArray(data.produtos)) {
          throw new Error("catálogo indisponível");
        }
        produtos = data.produtos;
        return produtos;
      })
      .catch((erro) => {
        promessaProdutos = null;
        throw erro;
      });
    return promessaProdutos;
  }

  function destacarItem(indice) {
    const botoes = dom.lista.querySelectorAll(".consulta-preco-item");
    botoes.forEach((botao, posicao) => {
      const ativo = posicao === indice;
      botao.classList.toggle("selecionado", ativo);
      botao.setAttribute("aria-selected", ativo ? "true" : "false");
      if (ativo) botao.scrollIntoView({ block: "nearest" });
    });
  }

  function selecionar(indice) {
    const produto = resultados[indice];
    if (!produto) return;

    indiceSelecionado = indice;
    destacarItem(indice);

    dom.destaque.hidden = false;
    dom.destaqueNome.textContent = produto.nome;
    dom.destaqueCodigo.textContent = codigoExibido(produto);
    dom.destaquePreco.textContent = formatarCentavos(produto.preco_centavos);

    const estoque = Number.isFinite(produto.estoque_atual)
      ? `${produto.estoque_atual} ${produto.unidade || "UN"} em estoque`
      : "Estoque não informado";
    dom.destaqueMeta.textContent = `${estoque} · Preço de venda`;
  }

  function renderizarLista(termoOriginal, total) {
    dom.lista.replaceChildren();
    indiceSelecionado = -1;
    dom.destaque.hidden = true;

    if (resultados.length === 0) {
      definirStatus(`Nenhum produto encontrado para "${termoOriginal}".`);
      return;
    }

    definirStatus(
      total > resultados.length
        ? `Mostrando ${resultados.length} de ${total} produtos.`
        : `${total} produto(s) encontrado(s).`,
    );

    resultados.forEach((produto, indice) => {
      const botao = criarElemento("button", "consulta-preco-item");
      botao.type = "button";
      botao.setAttribute("role", "option");
      botao.setAttribute("aria-selected", "false");

      const info = criarElemento("span", "consulta-preco-item-info");
      info.append(
        criarElemento("strong", "consulta-preco-item-nome", produto.nome),
        criarElemento("small", "consulta-preco-item-codigo", codigoExibido(produto)),
      );

      const preco = criarElemento(
        "span",
        "consulta-preco-item-preco",
        formatarCentavos(produto.preco_centavos),
      );

      botao.append(info, preco);
      botao.addEventListener("click", () => selecionar(indice));
      dom.lista.appendChild(botao);
    });

    selecionar(0);
  }

  function filtrar(termo) {
    const termoOriginal = String(termo ?? "").trim();
    const termoNormalizado = normalizarBusca(termoOriginal);
    const apenasDigitos = /^[0-9]+$/.test(termoOriginal);

    if (termoNormalizado.length < MIN_CARACTERES && !apenasDigitos) {
      limparResultados();
      definirStatus("Digite ao menos 2 caracteres ou bipe o código.");
      return;
    }

    if (!produtos) {
      definirStatus("Carregando catálogo de produtos...");
      return;
    }

    let encontrados;
    if (apenasDigitos) {
      encontrados = produtos.filter(
        (produto) =>
          (produto.codigo_barras && produto.codigo_barras.includes(termoOriginal)) ||
          (produto.sku && normalizarBusca(produto.sku).includes(termoOriginal)),
      );
    } else {
      const palavras = termoNormalizado.split(/\s+/).filter(Boolean);
      encontrados = produtos.filter((produto) => {
        const alvo = normalizarBusca(
          `${produto.nome} ${produto.sku ?? ""} ${produto.codigo_barras ?? ""}`,
        );
        return palavras.every((palavra) => alvo.includes(palavra));
      });
    }

    resultados = encontrados.slice(0, MAX_RESULTADOS);
    renderizarLista(termoOriginal, encontrados.length);
  }

  async function abrir(termoInicial) {
    if (estaAberta()) {
      dom.input.focus();
      dom.input.select();
      return;
    }

    focoAnterior = document.activeElement;
    limparResultados();
    const termo = typeof termoInicial === "string" ? termoInicial.trim() : "";
    dom.input.value = termo;
    dom.overlay.classList.add("aberto");
    dom.input.focus();
    if (termo) dom.input.select();

    if (produtos) {
      if (termo) {
        filtrar(termo);
      } else {
        definirStatus("Digite ao menos 2 caracteres ou bipe o código.");
      }
      return;
    }

    definirStatus("Carregando catálogo de produtos...");
    try {
      await carregarProdutos();
      if (!estaAberta()) return;
      if (dom.input.value.trim()) {
        filtrar(dom.input.value);
        return;
      }
      definirStatus(
        produtos.length > 0
          ? "Digite ao menos 2 caracteres ou bipe o código."
          : "Nenhum produto ativo cadastrado.",
      );
    } catch (_erro) {
      if (!estaAberta()) return;
      definirStatus("Não foi possível carregar o catálogo. Feche e tente novamente.");
      mostrarToast("Falha ao carregar o catálogo para consulta.");
    }
  }

  function fechar() {
    if (!estaAberta()) return;
    dom.overlay.classList.remove("aberto");
    if (focoAnterior && typeof focoAnterior.focus === "function") {
      focoAnterior.focus();
    }
    focoAnterior = null;
  }

  dom.input.addEventListener("input", () => filtrar(dom.input.value));

  dom.input.addEventListener("keydown", (evento) => {
    if (evento.key === "ArrowDown" || evento.key === "ArrowUp") {
      evento.preventDefault();
      if (resultados.length === 0) return;
      const passo = evento.key === "ArrowDown" ? 1 : -1;
      const base = indiceSelecionado < 0 ? 0 : indiceSelecionado;
      const proximo = Math.min(Math.max(base + passo, 0), resultados.length - 1);
      selecionar(proximo);
      return;
    }

    if (evento.key === "Enter") {
      evento.preventDefault();
      if (resultados.length === 0) return;
      selecionar(indiceSelecionado >= 0 ? indiceSelecionado : 0);
      // Leitor de código de barras envia Enter ao final: seleciona o texto
      // para que a próxima bipagem substitua o código anterior.
      if (/^[0-9]+$/.test(dom.input.value.trim())) dom.input.select();
    }
  });

  dom.btnFechar.addEventListener("click", fechar);

  dom.overlay.addEventListener("click", (evento) => {
    if (evento.target === dom.overlay) fechar();
  });

  // Captura o Esc antes dos atalhos globais de cada página (F1..F9).
  document.addEventListener(
    "keydown",
    (evento) => {
      if (!estaAberta() || evento.key !== "Escape") return;
      evento.preventDefault();
      evento.stopPropagation();
      fechar();
    },
    true,
  );

  instancia = { abrir, fechar, estaAberta };
  return instancia;
}
