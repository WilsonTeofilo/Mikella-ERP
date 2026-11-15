/**
 * Gráficos SVG do Mikella — sem biblioteca, sem CDN, sem canvas.
 *
 * Por que SVG puro:
 * - a CSP é restritiva (`style-src 'self'`), então nada de `<style>` ou `style=`;
 * - o app roda offline em máquina fraca (HDD/6 GB): zero dependência nova;
 * - o desenho usa apenas tokens de cor do design system.
 *
 * Regras: nenhum dado do servidor vira HTML; tudo é criado com createElementNS
 * e recebe valor por textContent/atributo. Cada gráfico expõe `role="img"` com
 * um resumo em `aria-label`, e cada elemento tem `<title>` para o tooltip nativo.
 */

import { formatarCentavos } from "./formatadores.js";

const SVG_NS = "http://www.w3.org/2000/svg";

/** Rótulo curto para eixos: "R$ 1,2 mil", "350". */
export function formatarEixo(centavos) {
  const reais = Math.abs(Number(centavos) || 0) / 100;
  const sinal = Number(centavos) < 0 ? "-" : "";
  if (reais >= 1_000_000) {
    return `${sinal}R$ ${(reais / 1_000_000).toFixed(1).replace(".", ",")} mi`;
  }
  if (reais >= 1000) {
    const valor = reais / 1000;
    const casas = valor >= 10 ? 0 : 1;
    return `${sinal}R$ ${valor.toFixed(casas).replace(".", ",")} mil`;
  }
  return `${sinal}R$ ${Math.round(reais)}`;
}

function truncar(texto, limite) {
  const valor = String(texto ?? "");
  return valor.length > limite ? `${valor.slice(0, limite - 1)}…` : valor;
}

function criarSvg(largura, altura, descricao) {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${largura} ${altura}`);
  svg.setAttribute("class", "grafico");
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", descricao);
  return svg;
}

function texto(x, y, conteudo, classe, ancora) {
  const el = document.createElementNS(SVG_NS, "text");
  el.setAttribute("x", String(x));
  el.setAttribute("y", String(y));
  el.setAttribute("class", classe);
  if (ancora) el.setAttribute("text-anchor", ancora);
  el.textContent = conteudo;
  return el;
}

function retangulo(x, y, largura, altura, classe) {
  const el = document.createElementNS(SVG_NS, "rect");
  el.setAttribute("x", String(x));
  el.setAttribute("y", String(y));
  el.setAttribute("width", String(Math.max(0, largura)));
  el.setAttribute("height", String(Math.max(0, altura)));
  el.setAttribute("class", classe);
  return el;
}

function titulo(conteudo) {
  const el = document.createElementNS(SVG_NS, "title");
  el.textContent = conteudo;
  return el;
}

function linha(x1, y1, x2, y2, classe) {
  const el = document.createElementNS(SVG_NS, "line");
  el.setAttribute("x1", String(x1));
  el.setAttribute("y1", String(y1));
  el.setAttribute("x2", String(x2));
  el.setAttribute("y2", String(y2));
  el.setAttribute("class", classe);
  return el;
}

/** Mensagem amigável no lugar do desenho (loja nova, período sem venda). */
export function estadoVazio(alvo, mensagem) {
  if (!alvo) return;
  const caixa = document.createElement("div");
  caixa.className = "grafico-vazio";
  caixa.textContent = mensagem;
  alvo.replaceChildren(caixa);
}

function gradeVertical(svg, dados) {
  const { largura, altura, margem, max } = dados;
  const area = altura - margem.t - margem.b;
  [0, 0.5, 1].forEach((fracao) => {
    const y = margem.t + area * (1 - fracao);
    svg.appendChild(linha(margem.l, y, largura - margem.r, y, "grafico-grade"));
    svg.appendChild(
      texto(margem.l - 8, y + 4, formatarEixo(max * fracao), "grafico-eixo-rotulo", "end"),
    );
  });
}

function rotulosX(svg, itens, dados) {
  const { largura, altura, margem } = dados;
  const area = largura - margem.l - margem.r;
  const passo = area / Math.max(1, itens.length);
  const salto = Math.ceil(itens.length / 9);
  itens.forEach((item, indice) => {
    if (indice % salto !== 0 && indice !== itens.length - 1) return;
    const x = margem.l + passo * indice + passo / 2;
    svg.appendChild(
      texto(x, altura - 12, item.rotulo, "grafico-rotulo", "middle"),
    );
  });
}

function baseDaEscala(itens, campos) {
  let max = 0;
  itens.forEach((item) => {
    campos.forEach((campo) => {
      max = Math.max(max, Number(item[campo]) || 0);
    });
  });
  return max > 0 ? max : 1;
}

/**
 * Barras verticais agrupadas (ex.: entradas x saídas por dia).
 * `itens`: [{ rotulo, a, b }]
 */
export function graficoBarrasComparadas(alvo, { itens, descricao, rotuloA = "Série A", rotuloB = "Série B" }) {
  if (!alvo) return;
  if (!itens.length) {
    estadoVazio(alvo, "Sem dados no período.");
    return;
  }

  const largura = 680;
  const altura = 230;
  const margem = { t: 18, r: 14, b: 34, l: 62 };
  const areaAltura = altura - margem.t - margem.b;
  const areaLargura = largura - margem.l - margem.r;
  const max = baseDaEscala(itens, ["a", "b"]);
  const passo = areaLargura / itens.length;
  const barra = Math.max(3, Math.min(12, (passo - 8) / 2));

  const svg = criarSvg(largura, altura, descricao);
  gradeVertical(svg, { largura, altura, margem, max });

  itens.forEach((item, indice) => {
    const centro = margem.l + passo * indice + passo / 2;
    const valorA = Number(item.a) || 0;
    const valorB = Number(item.b) || 0;

    const altA = (valorA / max) * areaAltura;
    const altB = (valorB / max) * areaAltura;

    const barraA = retangulo(
      centro - barra - 1,
      margem.t + areaAltura - altA,
      barra,
      altA,
      "grafico-barra-entrada",
    );
    barraA.appendChild(titulo(`${item.rotulo} — ${rotuloA}: ${formatarCentavos(valorA)}`));
    svg.appendChild(barraA);

    const barraB = retangulo(
      centro + 1,
      margem.t + areaAltura - altB,
      barra,
      altB,
      "grafico-barra-saida",
    );
    barraB.appendChild(titulo(`${item.rotulo} — ${rotuloB}: ${formatarCentavos(valorB)}`));
    svg.appendChild(barraB);
  });

  rotulosX(svg, itens, { largura, altura, margem });

  // Legenda explícita: as duas séries usam cores diferentes e o título nativo
  // do SVG não é descoberto por quem usa teclado ou leitor de tela.
  const legenda = document.createElement("ul");
  legenda.className = "grafico-legenda grafico-legenda-linha";
  [
    { rotulo: rotuloA, cor: "grafico-cor-1" },
    { rotulo: rotuloB, cor: "grafico-cor-5" },
  ].forEach((serie) => {
    const item = document.createElement("li");
    const amostra = document.createElement("span");
    amostra.className = `grafico-cor ${serie.cor}`;
    const nome = document.createElement("span");
    nome.className = "grafico-legenda-rotulo";
    nome.textContent = serie.rotulo;
    item.append(amostra, nome);
    legenda.appendChild(item);
  });

  alvo.replaceChildren(svg, legenda);
}

/** Barras verticais simples (ex.: faturamento por hora). */
export function graficoBarras(alvo, { itens, descricao, classe = "grafico-barra" }) {
  if (!alvo) return;
  if (!itens.length) {
    estadoVazio(alvo, "Sem dados no período.");
    return;
  }

  const largura = 680;
  const altura = 200;
  const margem = { t: 16, r: 12, b: 32, l: 58 };
  const areaAltura = altura - margem.t - margem.b;
  const areaLargura = largura - margem.l - margem.r;
  const max = baseDaEscala(itens, ["valor"]);
  const passo = areaLargura / itens.length;
  const barra = Math.max(3, Math.min(26, passo - 4));

  const svg = criarSvg(largura, altura, descricao);
  gradeVertical(svg, { largura, altura, margem, max });

  itens.forEach((item, indice) => {
    const valor = Number(item.valor) || 0;
    if (valor <= 0) return;
    const alt = (valor / max) * areaAltura;
    const x = margem.l + passo * indice + (passo - barra) / 2;
    const elemento = retangulo(
      x,
      margem.t + areaAltura - alt,
      barra,
      alt,
      classe,
    );
    elemento.appendChild(titulo(`${item.rotulo}: ${formatarCentavos(valor)}`));
    svg.appendChild(elemento);
  });

  rotulosX(svg, itens, { largura, altura, margem });
  alvo.replaceChildren(svg);
}

/**
 * Rosca de participação com legenda em HTML.
 * `fatias`: [{ rotulo, valor }]
 */
export function graficoRosca(alvo, { fatias, descricao, tituloCentro = "Total" }) {
  if (!alvo) return;

  const validas = fatias.filter((fatia) => Number(fatia.valor) > 0);
  const total = validas.reduce((soma, fatia) => soma + Number(fatia.valor), 0);
  if (total <= 0) {
    estadoVazio(alvo, "Sem vendas no período.");
    return;
  }

  const tamanho = 220;
  const centro = tamanho / 2;
  const raio = 68;
  const espessura = 30;
  const circunferencia = 2 * Math.PI * raio;

  const svg = criarSvg(tamanho, tamanho, descricao);
  const grupo = document.createElementNS(SVG_NS, "g");
  grupo.setAttribute("transform", `rotate(-90 ${centro} ${centro})`);

  let deslocamento = 0;
  validas.forEach((fatia, indice) => {
    const fracao = Number(fatia.valor) / total;
    const comprimento = fracao * circunferencia;

    const circulo = document.createElementNS(SVG_NS, "circle");
    circulo.setAttribute("cx", String(centro));
    circulo.setAttribute("cy", String(centro));
    circulo.setAttribute("r", String(raio));
    circulo.setAttribute("fill", "none");
    circulo.setAttribute("class", `grafico-fatia grafico-fatia-${(indice % 6) + 1}`);
    circulo.setAttribute("stroke-width", String(espessura));
    circulo.setAttribute("stroke-dasharray", `${comprimento} ${circunferencia - comprimento}`);
    circulo.setAttribute("stroke-dashoffset", String(-deslocamento));
    circulo.appendChild(
      titulo(
        `${fatia.rotulo}: ${formatarCentavos(fatia.valor)} (${Math.round(fracao * 100)}%)`,
      ),
    );
    grupo.appendChild(circulo);
    deslocamento += comprimento;
  });
  svg.appendChild(grupo);

  svg.appendChild(
    texto(centro, centro - 2, formatarEixo(total), "grafico-centro-valor", "middle"),
  );
  svg.appendChild(texto(centro, centro + 16, tituloCentro, "grafico-centro-rotulo", "middle"));

  const legenda = document.createElement("ul");
  legenda.className = "grafico-legenda";
  validas.forEach((fatia, indice) => {
    const linhaLegenda = document.createElement("li");

    const cor = document.createElement("span");
    cor.className = `grafico-cor grafico-cor-${(indice % 6) + 1}`;

    const nome = document.createElement("span");
    nome.className = "grafico-legenda-rotulo";
    nome.textContent = fatia.rotulo;

    const valor = document.createElement("span");
    valor.className = "grafico-legenda-valor";
    const pct = Math.round((Number(fatia.valor) / total) * 100);
    valor.textContent = `${formatarCentavos(fatia.valor)} · ${pct}%`;

    linhaLegenda.append(cor, nome, valor);
    legenda.appendChild(linhaLegenda);
  });

  alvo.replaceChildren(svg, legenda);
}

/**
 * Cascata (waterfall) do DRE.
 * `etapas`: [{ rotulo, delta, tipo: "inicio" | "delta" | "total" }]
 */
export function graficoCascata(alvo, { etapas, descricao }) {
  if (!alvo) return;
  if (!etapas.length) {
    estadoVazio(alvo, "Sem dados para o período.");
    return;
  }

  const largura = 680;
  const altura = 250;
  const margem = { t: 34, r: 14, b: 40, l: 62 };
  const areaAltura = altura - margem.t - margem.b;
  const areaLargura = largura - margem.l - margem.r;

  let acumulado = 0;
  const barras = etapas.map((etapa) => {
    const delta = Number(etapa.delta) || 0;
    if (etapa.tipo === "total") {
      acumulado = delta;
      return { ...etapa, inicio: 0, fim: delta };
    }
    const inicio = acumulado;
    acumulado += delta;
    return { ...etapa, inicio, fim: acumulado };
  });

  const valores = barras.flatMap((barra) => [barra.inicio, barra.fim, 0]);
  const max = Math.max(...valores, 1);
  const min = Math.min(...valores, 0);
  const faixa = max - min || 1;
  const alturaDe = (valor) => (valor / faixa) * areaAltura;
  const baseZero = margem.t + alturaDe(max);

  const svg = criarSvg(largura, altura, descricao);
  svg.appendChild(linha(margem.l, baseZero, largura - margem.r, baseZero, "grafico-eixo"));

  const passo = areaLargura / barras.length;
  const larguraBarra = Math.min(68, passo - 18);

  barras.forEach((barra, indice) => {
    const topo = Math.max(barra.inicio, barra.fim);
    const fundo = Math.min(barra.inicio, barra.fim);
    const x = margem.l + passo * indice + (passo - larguraBarra) / 2;
    const y = margem.t + alturaDe(max - topo);
    const alturaBarra = Math.max(2, alturaDe(topo - fundo));

    let classe = "grafico-barra-delta-negativo";
    if (barra.tipo === "total" || barra.tipo === "inicio") {
      classe = "grafico-barra-total";
    } else if (Number(barra.delta) > 0) {
      classe = "grafico-barra-delta-positivo";
    }

    const retanguloBarra = retangulo(x, y, larguraBarra, alturaBarra, classe);
    const sinal = Number(barra.delta) < 0 ? "−" : "";
    retanguloBarra.appendChild(
      titulo(`${barra.rotulo}: ${sinal}${formatarCentavos(Math.abs(Number(barra.delta) || 0))}`),
    );
    svg.appendChild(retanguloBarra);

    const rotuloValor = formatarEixo(barra.fim);
    svg.appendChild(
      texto(
        x + larguraBarra / 2,
        y - 6,
        rotuloValor,
        "grafico-valor",
        "middle",
      ),
    );

    // Rótulo em duas linhas curtas para caber em telas de 1024px.
    const partes = String(barra.rotulo).split(" ");
    const primeira = partes.slice(0, Math.ceil(partes.length / 2)).join(" ");
    const segunda = partes.slice(Math.ceil(partes.length / 2)).join(" ");
    svg.appendChild(
      texto(x + larguraBarra / 2, altura - 22, primeira, "grafico-rotulo", "middle"),
    );
    if (segunda) {
      svg.appendChild(
        texto(x + larguraBarra / 2, altura - 8, segunda, "grafico-rotulo", "middle"),
      );
    }
  });

  alvo.replaceChildren(svg);
}

/**
 * Barras horizontais (ranking de produtos, aging de crediário).
 * `itens`: [{ rotulo, valor, detalhe }]
 */
export function graficoBarrasHorizontais(alvo, { itens, descricao }) {
  if (!alvo) return;
  if (!itens.length) {
    estadoVazio(alvo, "Sem dados no período.");
    return;
  }

  const largura = 680;
  const linhaAltura = 34;
  const margem = { t: 10, r: 150, b: 8, l: 170 };
  const altura = margem.t + margem.b + itens.length * linhaAltura;
  const areaLargura = largura - margem.l - margem.r;
  const max = Math.max(...itens.map((item) => Number(item.valor) || 0), 1);

  const svg = criarSvg(largura, altura, descricao);
  itens.forEach((item, indice) => {
    const y = margem.t + indice * linhaAltura;
    const valor = Number(item.valor) || 0;
    const comprimento = (valor / max) * areaLargura;

    svg.appendChild(
      texto(0, y + linhaAltura / 2 + 4, truncar(item.rotulo, 26), "grafico-rotulo-horizontal"),
    );

    svg.appendChild(
      retangulo(margem.l, y + linhaAltura / 2 - 7, areaLargura, 14, "grafico-trilho"),
    );
    const barra = retangulo(
      margem.l,
      y + linhaAltura / 2 - 7,
      comprimento,
      14,
      "grafico-barra",
    );
    barra.appendChild(
      titulo(
        `${item.rotulo}: ${formatarCentavos(valor)}${item.detalhe ? ` — ${item.detalhe}` : ""}`,
      ),
    );
    svg.appendChild(barra);

    const rotulo = item.detalhe
      ? `${formatarCentavos(valor)} · ${item.detalhe}`
      : formatarCentavos(valor);
    svg.appendChild(
      texto(largura - 2, y + linhaAltura / 2 + 4, rotulo, "grafico-valor-horizontal", "end"),
    );
  });

  alvo.replaceChildren(svg);
}
