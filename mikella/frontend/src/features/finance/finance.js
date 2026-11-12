import { garantirSessao, requisitarJSON } from "../../shared/services/api.js";
import { mostrarToast } from "../../shared/ui/feedback.js";
import {
  formatarCentavos,
  parsearCentavos,
} from "../../shared/ui/formatadores.js";
import { inicializarControlesJanela } from "../../shared/ui/window-controls.js";

inicializarControlesJanela();
garantirSessao();

const avisar = mostrarToast;

// Estado global da tela
let ABA_ATIVA = "vendas"; // "vendas" | "fluxo" | "pagar" | "receber" | "resultado"
let CONTAS_CACHE = [];
let FILTRO_PAGAR = "TODAS"; // "TODAS" | "7DIAS" | "VENCIDAS"
let FILTRO_RECEBER = "TODAS"; // "TODAS" | "HOJE" | "VENCIDAS"
let PERIODO_CHOKKO_ATIVO = "hoje";

// Elementos das Abas
const tabs = {
  vendas: document.getElementById("tab-vendas"),
  fluxo: document.getElementById("tab-fluxo"),
  pagar: document.getElementById("tab-pagar"),
  receber: document.getElementById("tab-receber"),
  resultado: document.getElementById("tab-resultado"),
};

const paineis = {
  vendas: document.getElementById("painel-vendas"),
  fluxo: document.getElementById("painel-fluxo"),
  pagar: document.getElementById("painel-pagar"),
  receber: document.getElementById("painel-receber"),
  resultado: document.getElementById("painel-resultado"),
};

function alternarAba(nomeAba) {
  ABA_ATIVA = nomeAba;
  Object.entries(tabs).forEach(([nome, btn]) => {
    if (!btn) return;
    const ativa = nome === nomeAba;
    btn.classList.toggle("ativa", ativa);
    btn.setAttribute("aria-selected", ativa ? "true" : "false");
  });

  Object.entries(paineis).forEach(([nome, painel]) => {
    if (!painel) return;
    painel.classList.toggle("oculto", nome !== nomeAba);
  });

  if (nomeAba === "vendas") {
    carregarPainelChokko(PERIODO_CHOKKO_ATIVO);
  } else if (nomeAba === "fluxo") {
    carregarFluxoCaixa();
  } else if (nomeAba === "pagar" || nomeAba === "receber") {
    carregarContas();
  } else if (nomeAba === "resultado") {
    carregarResultadoDRE();
  }
}

if (tabs.vendas) tabs.vendas.addEventListener("click", () => alternarAba("vendas"));
if (tabs.fluxo) tabs.fluxo.addEventListener("click", () => alternarAba("fluxo"));
if (tabs.pagar) tabs.pagar.addEventListener("click", () => alternarAba("pagar"));
if (tabs.receber) tabs.receber.addEventListener("click", () => alternarAba("receber"));
if (tabs.resultado) tabs.resultado.addEventListener("click", () => alternarAba("resultado"));

// -------------------------------------------------------------
// ABA 1: VISÃO GERAL & VENDAS (INSPIRADO CHOKKO-MELT)
// -------------------------------------------------------------
const btnHoje = document.getElementById("btn-periodo-hoje");
const btnSemana = document.getElementById("btn-periodo-semana");
const btnMes = document.getElementById("btn-periodo-mes");
const inputChokkoInicio = document.getElementById("chokko-data-inicio");
const inputChokkoFim = document.getElementById("chokko-data-fim");
const btnFiltrarChokko = document.getElementById("btn-filtrar-chokko");

function marcarBotaoPeriodo(btnAtivo) {
  [btnHoje, btnSemana, btnMes].forEach((b) => {
    if (b) b.classList.toggle("ativo", b === btnAtivo);
  });
}

if (btnHoje) {
  btnHoje.addEventListener("click", () => {
    marcarBotaoPeriodo(btnHoje);
    PERIODO_CHOKKO_ATIVO = "hoje";
    carregarPainelChokko("hoje");
  });
}

if (btnSemana) {
  btnSemana.addEventListener("click", () => {
    marcarBotaoPeriodo(btnSemana);
    PERIODO_CHOKKO_ATIVO = "semana";
    carregarPainelChokko("semana");
  });
}

if (btnMes) {
  btnMes.addEventListener("click", () => {
    marcarBotaoPeriodo(btnMes);
    PERIODO_CHOKKO_ATIVO = "mes";
    carregarPainelChokko("mes");
  });
}

if (btnFiltrarChokko) {
  btnFiltrarChokko.addEventListener("click", () => {
    const ini = inputChokkoInicio.value;
    const fim = inputChokkoFim.value;
    if (!ini) {
      avisar("Informe pelo menos a data inicial para filtrar.");
      return;
    }
    marcarBotaoPeriodo(null);
    carregarPainelChokko("custom", ini, fim || ini);
  });
}

async function carregarPainelChokko(periodo, dataInicio = null, dataFim = null) {
  let url = `/v1/finance/resumo?periodo=${encodeURIComponent(periodo)}`;
  if (dataInicio) {
    url += `&data_inicio=${encodeURIComponent(dataInicio)}`;
    if (dataFim) url += `&data_fim=${encodeURIComponent(dataFim)}`;
  }

  const { ok, data } = await requisitarJSON(url);
  if (!ok) {
    avisar(data.erro || "Falha ao carregar indicadores de vendas.");
    return;
  }

  const elFat = document.getElementById("card-fat-bruto");
  if (elFat) elFat.textContent = formatarCentavos(data.faturamento_centavos);

  const elLucro = document.getElementById("card-lucro-liquido");
  if (elLucro) elLucro.textContent = formatarCentavos(data.lucro_bruto_centavos);

  const elMargem = document.getElementById("card-margem-detalhe");
  if (elMargem) {
    elMargem.textContent = `Margem: ${Number(data.margem_bruta_pct || 0).toFixed(1)}%`;
  }

  const elPix = document.getElementById("card-total-pix");
  if (elPix) elPix.textContent = formatarCentavos(data.total_pix_centavos);

  const elCartao = document.getElementById("card-total-cartao");
  if (elCartao) elCartao.textContent = formatarCentavos(data.total_cartao_centavos);

  const elDinheiro = document.getElementById("card-total-dinheiro");
  if (elDinheiro) elDinheiro.textContent = formatarCentavos(data.total_dinheiro_centavos);

  const elQtd = document.getElementById("card-qtd-vendas");
  if (elQtd) elQtd.textContent = String(data.num_vendas || 0);

  const elTicket = document.getElementById("card-ticket-medio");
  if (elTicket) {
    elTicket.textContent = `Ticket Médio: ${formatarCentavos(data.ticket_medio_centavos)}`;
  }

  const elSaldoTopo = document.getElementById("saldo-resumo");
  if (elSaldoTopo) {
    elSaldoTopo.textContent = `Faturamento: ${formatarCentavos(data.faturamento_centavos)}`;
  }

  // Renderizar tabela de entradas Chokko-Melt
  const tbody = document.getElementById("tabela-entradas-chokko");
  if (!tbody) return;
  tbody.replaceChildren();

  const entradas = data.entradas_detalhadas || [];
  if (entradas.length === 0) {
    const trVazio = document.createElement("tr");
    const tdVazio = document.createElement("td");
    tdVazio.colSpan = 6;
    tdVazio.style.textAlign = "center";
    tdVazio.style.padding = "24px";
    tdVazio.style.color = "var(--cinza-500)";
    tdVazio.textContent = "Nenhuma venda registrada no período selecionado.";
    trVazio.appendChild(tdVazio);
    tbody.appendChild(trVazio);
    return;
  }

  entradas.forEach((v) => {
    const tr = document.createElement("tr");

    // Coluna 1: COO / ID
    const tdId = document.createElement("td");
    tdId.style.fontWeight = "bold";
    tdId.textContent = v.coo ? `#${String(v.coo).padStart(6, "0")}` : v.id.slice(0, 8);

    // Coluna 2: Data/Hora formatada em horário local (Brasília)
    const tdData = document.createElement("td");
    const d = new Date(v.created_at);
    tdData.textContent = !isNaN(d.getTime())
      ? d.toLocaleString("pt-BR", { timeZone: "America/Sao_Paulo" })
      : v.created_at;

    // Coluna 3: Cliente
    const tdCli = document.createElement("td");
    tdCli.textContent = v.cliente_nome || "Consumidor Final";

    // Coluna 4: Badge Forma de Pagamento
    const tdForma = document.createElement("td");
    const badge = document.createElement("span");
    badge.className = "badge-metodo";
    const formaUpper = (v.forma_pagamento || "").toUpperCase();
    if (formaUpper.includes("PIX")) {
      badge.classList.add("badge-pix");
      badge.textContent = "PIX";
    } else if (
      formaUpper.includes("CREDITO") ||
      formaUpper.includes("DEBITO") ||
      formaUpper.includes("CARTAO")
    ) {
      badge.classList.add("badge-cartao");
      badge.textContent = formaUpper;
    } else if (formaUpper.includes("DINHEIRO")) {
      badge.classList.add("badge-dinheiro");
      badge.textContent = "DINHEIRO";
    } else if (formaUpper.includes("CREDIARIO")) {
      badge.classList.add("badge-crediario");
      badge.textContent = "CREDIÁRIO";
    } else {
      badge.classList.add("badge-outro");
      badge.textContent = formaUpper || "OUTRO";
    }
    tdForma.appendChild(badge);

    // Coluna 5: Valor Bruto
    const tdBruto = document.createElement("td");
    tdBruto.style.textAlign = "right";
    tdBruto.style.fontWeight = "bold";
    tdBruto.textContent = formatarCentavos(v.valor_bruto_centavos);

    // Coluna 6: Lucro Líquido
    const tdLucro = document.createElement("td");
    tdLucro.style.textAlign = "right";
    tdLucro.style.fontWeight = "900";
    tdLucro.style.color = v.lucro_liquido_centavos >= 0 ? "var(--verde)" : "var(--vermelho)";
    tdLucro.textContent = formatarCentavos(v.lucro_liquido_centavos);

    tr.append(tdId, tdData, tdCli, tdForma, tdBruto, tdLucro);
    tbody.appendChild(tr);
  });
}

// -------------------------------------------------------------
// ABA 1: FLUXO DE CAIXA
// -------------------------------------------------------------
const inputFcInicio = document.getElementById("fc-data-inicio");
const inputFcFim = document.getElementById("fc-data-fim");
const btnFiltrarFc = document.getElementById("btn-filtrar-fc");

// Inicializa datas com os últimos 30 dias até hoje
const dataHoje = new Date();
const data30DiasAtras = new Date(Date.now() - 30 * 24 * 60 * 60 * 1000);
inputFcFim.value = dataHoje.toISOString().slice(0, 10);
inputFcInicio.value = data30DiasAtras.toISOString().slice(0, 10);

btnFiltrarFc.addEventListener("click", () => carregarFluxoCaixa());

async function carregarFluxoCaixa() {
  const inicio = inputFcInicio.value;
  const fim = inputFcFim.value;
  const url = `/v1/finance/fluxo-caixa?data_inicio=${encodeURIComponent(inicio)}&data_fim=${encodeURIComponent(fim)}`;

  const { ok, data } = await requisitarJSON(url);
  if (!ok) {
    avisar(data.erro || "Falha ao carregar fluxo de caixa.");
    return;
  }

  document.getElementById("fc-saldo-inicial").textContent = formatarCentavos(
    data.saldo_inicial_centavos,
  );
  document.getElementById("fc-total-entradas").textContent = formatarCentavos(
    data.total_entradas_centavos,
  );
  document.getElementById("fc-total-saidas").textContent = formatarCentavos(
    data.total_saidas_centavos,
  );
  document.getElementById("fc-saldo-final").textContent = formatarCentavos(
    data.saldo_final_centavos,
  );

  document.getElementById("saldo-resumo").textContent =
    `Saldo Final: ${formatarCentavos(data.saldo_final_centavos)}`;

  renderTabelaFluxo(data.dias || []);
  renderGraficoSvgFluxo(data.dias || []);
}

function renderTabelaFluxo(dias) {
  const tbody = document.getElementById("tabela-fc-corpo");
  tbody.replaceChildren();

  if (dias.length === 0) {
    const tr = document.createElement("tr");
    const td = document.createElement("td");
    td.colSpan = 5;
    td.style.textAlign = "center";
    td.style.padding = "var(--espaco-4)";
    td.textContent = "Nenhuma movimentação no período selecionado.";
    tr.appendChild(td);
    tbody.appendChild(tr);
    return;
  }

  dias.forEach((dia) => {
    const tr = document.createElement("tr");

    const tdData = document.createElement("td");
    tdData.textContent = dia.data;

    const tdEntradas = document.createElement("td");
    tdEntradas.className = "direita valor-positivo";
    tdEntradas.textContent = formatarCentavos(dia.entradas_centavos);

    const tdSaidas = document.createElement("td");
    tdSaidas.className = "direita valor-negativo";
    tdSaidas.textContent = formatarCentavos(dia.saidas_centavos);

    const tdSaldoDia = document.createElement("td");
    tdSaldoDia.className = "direita";
    tdSaldoDia.textContent = formatarCentavos(dia.saldo_dia_centavos);
    if (dia.saldo_dia_centavos > 0) {
      tdSaldoDia.classList.add("valor-positivo");
    } else if (dia.saldo_dia_centavos < 0) {
      tdSaldoDia.classList.add("valor-negativo");
    }

    const tdSaldoAcumulado = document.createElement("td");
    tdSaldoAcumulado.className = "direita";
    tdSaldoAcumulado.style.fontWeight = "bold";
    tdSaldoAcumulado.textContent = formatarCentavos(dia.saldo_acumulado_centavos);

    tr.append(tdData, tdEntradas, tdSaidas, tdSaldoDia, tdSaldoAcumulado);
    tbody.appendChild(tr);
  });
}

function renderGraficoSvgFluxo(dias) {
  const container = document.getElementById("container-svg-fluxo");
  container.replaceChildren();

  if (dias.length < 2) {
    const p = document.createElement("p");
    p.style.color = "var(--cinza-500)";
    p.style.fontStyle = "italic";
    p.textContent = "Histórico insuficiente para curva de saldo (mínimo de 2 dias com dados).";
    container.appendChild(p);
    return;
  }

  const valores = dias.map((d) => d.saldo_acumulado_centavos);
  const minVal = Math.min(...valores);
  const maxVal = Math.max(...valores);
  const range = maxVal - minVal === 0 ? 1 : maxVal - minVal;

  const width = 760;
  const height = 120;
  const paddingX = 40;
  const paddingY = 20;

  const points = dias.map((d, i) => {
    const x = paddingX + (i / (dias.length - 1)) * (width - 2 * paddingX);
    const y =
      height -
      paddingY -
      ((d.saldo_acumulado_centavos - minVal) / range) * (height - 2 * paddingY);
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });

  const svgNS = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(svgNS, "svg");
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  svg.setAttribute("width", "100%");
  svg.setAttribute("height", "120");
  svg.style.display = "block";

  // Linha de base zero se estiver no alcance
  if (minVal <= 0 && maxVal >= 0) {
    const yZero =
      height - paddingY - ((0 - minVal) / range) * (height - 2 * paddingY);
    const zeroLine = document.createElementNS(svgNS, "line");
    zeroLine.setAttribute("x1", String(paddingX));
    zeroLine.setAttribute("y1", yZero.toFixed(1));
    zeroLine.setAttribute("x2", String(width - paddingX));
    zeroLine.setAttribute("y2", yZero.toFixed(1));
    zeroLine.setAttribute("stroke", "#CBD5E1");
    zeroLine.setAttribute("stroke-dasharray", "4");
    svg.appendChild(zeroLine);
  }

  // Linha da série do saldo em cobalto
  const polyline = document.createElementNS(svgNS, "polyline");
  polyline.setAttribute("fill", "none");
  polyline.setAttribute("stroke", "#0040D8");
  polyline.setAttribute("stroke-width", "3");
  polyline.setAttribute("points", points.join(" "));
  svg.appendChild(polyline);

  // Pontos nos extremos e valores
  dias.forEach((d, i) => {
    if (i === 0 || i === dias.length - 1 || i % Math.ceil(dias.length / 5) === 0) {
      const coord = points[i].split(",");
      const circle = document.createElementNS(svgNS, "circle");
      circle.setAttribute("cx", coord[0]);
      circle.setAttribute("cy", coord[1]);
      circle.setAttribute("r", "4");
      circle.setAttribute("fill", "#0040D8");
      circle.setAttribute("stroke", "#000");
      circle.setAttribute("stroke-width", "1.5");
      svg.appendChild(circle);

      const text = document.createElementNS(svgNS, "text");
      text.setAttribute("x", coord[0]);
      text.setAttribute("y", String(parseFloat(coord[1]) - 8));
      text.setAttribute("text-anchor", "middle");
      text.setAttribute("font-size", "10");
      text.setAttribute("font-family", "sans-serif");
      text.setAttribute("fill", "#000");
      text.textContent = formatarCentavos(d.saldo_acumulado_centavos);
      svg.appendChild(text);
    }
  });

  container.appendChild(svg);
}

// -------------------------------------------------------------
// ABA 2 E 3: CONTAS (A PAGAR E A RECEBER)
// -------------------------------------------------------------
async function carregarContas() {
  const { ok, data } = await requisitarJSON("/v1/finance/contas-abertas");
  if (!ok) {
    avisar(data.erro || "Falha ao carregar contas.");
    return;
  }

  CONTAS_CACHE = data.contas || [];
  atualizarAbaPagar();
  atualizarAbaReceber();
}

function atualizarAbaPagar() {
  const hoje = new Date().toISOString().slice(0, 10);
  const data7Dias = new Date(Date.now() + 7 * 24 * 60 * 60 * 1000)
    .toISOString()
    .slice(0, 10);

  let totalPagarCentavos = 0;
  let pagar7DiasCentavos = 0;
  let pagarVencidasCentavos = 0;

  const contasPagar = CONTAS_CACHE.filter(
    (c) => c.tipo === "PAGAMENTO" || c.tipo === "PAGAR",
  );

  contasPagar.forEach((c) => {
    const parcelas = (c.parcelas || []).filter((p) => p.status === "ABERTA");
    if (parcelas.length > 0) {
      parcelas.forEach((p) => {
        const vcto = (p.vencimento || "").slice(0, 10);
        totalPagarCentavos += p.valor_centavos;
        if (vcto <= data7Dias && vcto >= hoje) {
          pagar7DiasCentavos += p.valor_centavos;
        }
        if (vcto < hoje) {
          pagarVencidasCentavos += p.valor_centavos;
        }
      });
    } else if (c.valor_em_aberto > 0) {
      const vcto = (c.primeiro_vencimento || "").slice(0, 10);
      totalPagarCentavos += c.valor_em_aberto;
      if (vcto <= data7Dias && vcto >= hoje) {
        pagar7DiasCentavos += c.valor_em_aberto;
      }
      if (vcto < hoje) {
        pagarVencidasCentavos += c.valor_em_aberto;
      }
    }
  });

  document.getElementById("pagar-kpi-total").textContent =
    formatarCentavos(totalPagarCentavos);
  document.getElementById("pagar-kpi-7dias").textContent =
    formatarCentavos(pagar7DiasCentavos);
  document.getElementById("pagar-kpi-vencidas").textContent =
    formatarCentavos(pagarVencidasCentavos);

  renderTabelaPagar(contasPagar, FILTRO_PAGAR);
}

function renderTabelaPagar(contas, filtro) {
  const tbody = document.getElementById("tabela-pagar-corpo");
  tbody.replaceChildren();

  const hoje = new Date().toISOString().slice(0, 10);
  const data7Dias = new Date(Date.now() + 7 * 24 * 60 * 60 * 1000)
    .toISOString()
    .slice(0, 10);

  let linhas = 0;

  contas.forEach((c) => {
    const parcelas = (c.parcelas || []).filter((p) => p.status === "ABERTA");
    const itens =
      parcelas.length > 0
        ? parcelas.map((p) => ({
            indice: p.indice,
            numero: p.numero,
            vencimento: (p.vencimento || "").slice(0, 10),
            valor: p.valor_centavos,
          }))
        : [
            {
              indice: 0,
              numero: 1,
              vencimento: (c.primeiro_vencimento || "").slice(0, 10),
              valor: c.valor_em_aberto,
            },
          ];

    itens.forEach((item) => {
      const vencida = item.vencimento < hoje;
      const em7Dias = item.vencimento <= data7Dias && !vencida;

      if (filtro === "7DIAS" && !em7Dias) return;
      if (filtro === "VENCIDAS" && !vencida) return;

      linhas++;
      const tr = document.createElement("tr");

      const tdDesc = document.createElement("td");
      const strong = document.createElement("strong");
      strong.textContent = c.descricao;
      tdDesc.appendChild(strong);

      const tdVcto = document.createElement("td");
      tdVcto.textContent = item.vencimento || "-";
      if (vencida) {
        const tag = document.createElement("span");
        tag.className = "tag pagar";
        tag.style.marginLeft = "8px";
        tag.textContent = "VENCIDA";
        tdVcto.appendChild(tag);
      } else if (em7Dias) {
        const tag = document.createElement("span");
        tag.style.color = "#D97706";
        tag.style.fontWeight = "bold";
        tag.style.marginLeft = "8px";
        tag.textContent = "(7 DIAS)";
        tdVcto.appendChild(tag);
      }

      const tdParc = document.createElement("td");
      tdParc.textContent = String(item.numero);

      const tdVal = document.createElement("td");
      tdVal.className = "direita valor-negativo";
      tdVal.textContent = formatarCentavos(item.valor);

      const tdAcao = document.createElement("td");
      tdAcao.className = "direita";
      const btnLiq = document.createElement("button");
      btnLiq.type = "button";
      btnLiq.className = "botao";
      btnLiq.style.minHeight = "36px";
      btnLiq.style.padding = "4px 8px";
      btnLiq.textContent = "Liquidar";
      btnLiq.addEventListener("click", () =>
        abrirModalLiquidar(
          c.id,
          c.tipo,
          item.indice,
          item.valor,
          c.descricao,
          item.numero,
        ),
      );
      tdAcao.appendChild(btnLiq);

      tr.append(tdDesc, tdVcto, tdParc, tdVal, tdAcao);
      tbody.appendChild(tr);
    });
  });

  if (linhas === 0) {
    const tr = document.createElement("tr");
    const td = document.createElement("td");
    td.colSpan = 5;
    td.style.textAlign = "center";
    td.style.padding = "var(--espaco-4)";
    td.textContent = "Nenhuma conta a pagar encontrada neste filtro.";
    tr.appendChild(td);
    tbody.appendChild(tr);
  }
}

// Filtros da aba A Pagar
const btnFiltroPagarTodas = document.getElementById("filtro-pagar-todas");
const btnFiltroPagar7Dias = document.getElementById("filtro-pagar-7dias");
const btnFiltroPagarVencidas = document.getElementById("filtro-pagar-vencidas");

function setFiltroPagar(filtro) {
  FILTRO_PAGAR = filtro;
  btnFiltroPagarTodas.className =
    filtro === "TODAS" ? "botao primario" : "botao";
  btnFiltroPagar7Dias.className =
    filtro === "7DIAS" ? "botao primario" : "botao";
  btnFiltroPagarVencidas.className =
    filtro === "VENCIDAS" ? "botao primario" : "botao";
  atualizarAbaPagar();
}

btnFiltroPagarTodas.addEventListener("click", () => setFiltroPagar("TODAS"));
btnFiltroPagar7Dias.addEventListener("click", () => setFiltroPagar("7DIAS"));
btnFiltroPagarVencidas.addEventListener("click", () =>
  setFiltroPagar("VENCIDAS"),
);

function atualizarAbaReceber() {
  const hoje = new Date().toISOString().slice(0, 10);

  let totalReceberCentavos = 0;
  let receberHojeCentavos = 0;
  let receberVencidoCentavos = 0;

  let agingEmDia = 0;
  let aging1a30 = 0;
  let aging31a60 = 0;
  let agingMais60 = 0;

  const contasReceber = CONTAS_CACHE.filter(
    (c) => c.tipo !== "PAGAMENTO" && c.tipo !== "PAGAR",
  );

  contasReceber.forEach((c) => {
    const parcelas = (c.parcelas || []).filter((p) => p.status === "ABERTA");
    const itens =
      parcelas.length > 0
        ? parcelas.map((p) => ({
            valor: p.valor_centavos,
            vencimento: (p.vencimento || "").slice(0, 10),
          }))
        : [
            {
              valor: c.valor_em_aberto,
              vencimento: (c.primeiro_vencimento || "").slice(0, 10),
            },
          ];

    itens.forEach((item) => {
      totalReceberCentavos += item.valor;
      if (item.vencimento === hoje) {
        receberHojeCentavos += item.valor;
      }
      if (item.vencimento < hoje) {
        receberVencidoCentavos += item.valor;

        const diasAtraso = Math.floor(
          (new Date(hoje) - new Date(item.vencimento)) / (1000 * 60 * 60 * 24),
        );
        if (diasAtraso <= 30) {
          aging1a30 += item.valor;
        } else if (diasAtraso <= 60) {
          aging31a60 += item.valor;
        } else {
          agingMais60 += item.valor;
        }
      } else {
        agingEmDia += item.valor;
      }
    });
  });

  document.getElementById("receber-kpi-total").textContent =
    formatarCentavos(totalReceberCentavos);
  document.getElementById("receber-kpi-hoje").textContent =
    formatarCentavos(receberHojeCentavos);
  document.getElementById("receber-kpi-vencido").textContent =
    formatarCentavos(receberVencidoCentavos);

  document.getElementById("aging-em-dia").textContent =
    formatarCentavos(agingEmDia);
  document.getElementById("aging-1-30").textContent =
    formatarCentavos(aging1a30);
  document.getElementById("aging-31-60").textContent =
    formatarCentavos(aging31a60);
  document.getElementById("aging-mais-60").textContent =
    formatarCentavos(agingMais60);

  renderTabelaReceber(contasReceber, FILTRO_RECEBER);
}

function renderTabelaReceber(contas, filtro) {
  const tbody = document.getElementById("tabela-receber-corpo");
  tbody.replaceChildren();

  const hoje = new Date().toISOString().slice(0, 10);
  let linhas = 0;

  contas.forEach((c) => {
    const parcelas = (c.parcelas || []).filter((p) => p.status === "ABERTA");
    const itens =
      parcelas.length > 0
        ? parcelas.map((p) => ({
            indice: p.indice,
            numero: p.numero,
            vencimento: (p.vencimento || "").slice(0, 10),
            valor: p.valor_centavos,
          }))
        : [
            {
              indice: 0,
              numero: 1,
              vencimento: (c.primeiro_vencimento || "").slice(0, 10),
              valor: c.valor_em_aberto,
            },
          ];

    itens.forEach((item) => {
      const vencida = item.vencimento < hoje;
      const ehHoje = item.vencimento === hoje;

      if (filtro === "HOJE" && !ehHoje) return;
      if (filtro === "VENCIDAS" && !vencida) return;

      linhas++;
      const tr = document.createElement("tr");

      const tdDesc = document.createElement("td");
      const strong = document.createElement("strong");
      strong.textContent = c.descricao;
      tdDesc.appendChild(strong);

      const tdVcto = document.createElement("td");
      tdVcto.textContent = item.vencimento || "-";
      if (vencida) {
        const tag = document.createElement("span");
        tag.className = "tag pagar";
        tag.style.marginLeft = "8px";
        tag.textContent = "VENCIDA";
        tdVcto.appendChild(tag);
      } else if (ehHoje) {
        const tag = document.createElement("span");
        tag.style.color = "var(--azul-600)";
        tag.style.fontWeight = "bold";
        tag.style.marginLeft = "8px";
        tag.textContent = "(VENCE HOJE)";
        tdVcto.appendChild(tag);
      }

      const tdParc = document.createElement("td");
      tdParc.textContent = String(item.numero);

      const tdVal = document.createElement("td");
      tdVal.className = "direita valor-positivo";
      tdVal.textContent = formatarCentavos(item.valor);

      const tdAcao = document.createElement("td");
      tdAcao.className = "direita";
      const btnLiq = document.createElement("button");
      btnLiq.type = "button";
      btnLiq.className = "botao";
      btnLiq.style.minHeight = "36px";
      btnLiq.style.padding = "4px 8px";
      btnLiq.textContent = "Receber";
      btnLiq.addEventListener("click", () =>
        abrirModalLiquidar(
          c.id,
          c.tipo,
          item.indice,
          item.valor,
          c.descricao,
          item.numero,
        ),
      );
      tdAcao.appendChild(btnLiq);

      tr.append(tdDesc, tdVcto, tdParc, tdVal, tdAcao);
      tbody.appendChild(tr);
    });
  });

  if (linhas === 0) {
    const tr = document.createElement("tr");
    const td = document.createElement("td");
    td.colSpan = 5;
    td.style.textAlign = "center";
    td.style.padding = "var(--espaco-4)";
    td.textContent = "Nenhuma conta a receber encontrada neste filtro.";
    tr.appendChild(td);
    tbody.appendChild(tr);
  }
}

// Filtros da aba A Receber
const btnFiltroReceberTodas = document.getElementById("filtro-receber-todas");
const btnFiltroReceberHoje = document.getElementById("filtro-receber-hoje");
const btnFiltroReceberVencidas = document.getElementById(
  "filtro-receber-vencidas",
);

function setFiltroReceber(filtro) {
  FILTRO_RECEBER = filtro;
  btnFiltroReceberTodas.className =
    filtro === "TODAS" ? "botao primario" : "botao";
  btnFiltroReceberHoje.className =
    filtro === "HOJE" ? "botao primario" : "botao";
  btnFiltroReceberVencidas.className =
    filtro === "VENCIDAS" ? "botao primario" : "botao";
  atualizarAbaReceber();
}

btnFiltroReceberTodas.addEventListener("click", () =>
  setFiltroReceber("TODAS"),
);
btnFiltroReceberHoje.addEventListener("click", () => setFiltroReceber("HOJE"));
btnFiltroReceberVencidas.addEventListener("click", () =>
  setFiltroReceber("VENCIDAS"),
);

// -------------------------------------------------------------
// ABA 4: RESULTADO (DRE SIMPLIFICADA)
// -------------------------------------------------------------
const inputDreMes = document.getElementById("dre-mes-input");
const btnAtualizarDre = document.getElementById("btn-atualizar-dre");

// Inicializa com mês atual no formato YYYY-MM
inputDreMes.value = new Date().toISOString().slice(0, 7);
btnAtualizarDre.addEventListener("click", () => carregarResultadoDRE());

async function carregarResultadoDRE() {
  const mes = inputDreMes.value || new Date().toISOString().slice(0, 7);
  const url = `/v1/finance/resultado?mes=${encodeURIComponent(mes)}`;

  const { ok, data } = await requisitarJSON(url);
  if (!ok) {
    avisar(data.erro || "Falha ao calcular DRE.");
    return;
  }

  document.getElementById("dre-receita-bruta").textContent = formatarCentavos(
    data.receita_bruta_centavos,
  );
  document.getElementById("dre-deducoes").textContent = formatarCentavos(
    data.deducoes_devolucoes_centavos,
  );
  document.getElementById("dre-receita-liquida").textContent = formatarCentavos(
    data.receita_liquida_centavos,
  );
  document.getElementById("dre-cmv").textContent = formatarCentavos(
    data.cmv_centavos,
  );
  document.getElementById("dre-lucro-bruto").textContent = formatarCentavos(
    data.lucro_bruto_centavos,
  );
  document.getElementById("dre-despesas").textContent = formatarCentavos(
    data.despesas_operacionais_centavos,
  );

  const elResultado = document.getElementById("dre-resultado-liquido");
  elResultado.textContent = formatarCentavos(data.resultado_liquido_centavos);
  elResultado.className =
    data.resultado_liquido_centavos >= 0
      ? "direita valor-positivo"
      : "direita valor-negativo";

  document.getElementById("dre-margem-bruta").textContent =
    `${data.margem_bruta_percentual.toFixed(1)}%`;
  document.getElementById("dre-margem-liquida").textContent =
    `${data.margem_liquida_percentual.toFixed(1)}%`;

  const elDiag = document.getElementById("dre-diagnostico");
  if (data.resultado_liquido_centavos > 0) {
    elDiag.textContent = "Superávit Operacional";
    elDiag.style.color = "var(--verde)";
  } else if (data.resultado_liquido_centavos < 0) {
    elDiag.textContent = "Déficit / Prejuízo";
    elDiag.style.color = "var(--vermelho)";
  } else {
    elDiag.textContent = "Equilíbrio Exato";
    elDiag.style.color = "var(--azul-600)";
  }
}

// -------------------------------------------------------------
// MODAIS: NOVA CONTA & LIQUIDAR
// -------------------------------------------------------------
const modalConta = document.getElementById("modal-conta");
const btnNovaConta = document.getElementById("btn-nova-conta");
const btnCancelaConta = document.getElementById("btn-cancelar-conta");
const formConta = document.getElementById("form-conta");

btnNovaConta.addEventListener("click", () => {
  formConta.reset();
  document.getElementById("conta-vencimento").value = new Date()
    .toISOString()
    .slice(0, 10);
  if (ABA_ATIVA === "pagar") {
    document.getElementById("conta-tipo").value = "PAGAR";
  } else if (ABA_ATIVA === "receber") {
    document.getElementById("conta-tipo").value = "RECEBER";
  }
  modalConta.style.display = "flex";
  document.getElementById("conta-desc").focus();
});

btnCancelaConta.addEventListener("click", () => {
  modalConta.style.display = "none";
});

document.getElementById("conta-valor").addEventListener("input", (e) => {
  const v = e.target.value.replace(/\D/g, "");
  if (v.length > 0) {
    e.target.value = (parseInt(v, 10) / 100).toFixed(2).replace(".", ",");
  }
});

formConta.addEventListener("submit", async (e) => {
  e.preventDefault();
  const tipo = document.getElementById("conta-tipo").value;
  const endpoint =
    tipo === "PAGAR"
      ? "/v1/finance/contas-pagar"
      : "/v1/finance/contas-receber";

  const totalCentavos = parsearCentavos(
    document.getElementById("conta-valor").value,
  );
  if (totalCentavos <= 0) {
    avisar("Valor da conta deve ser maior que zero.");
    return;
  }

  const qtdParcelas =
    parseInt(document.getElementById("conta-parcelas").value, 10) || 1;
  const primeiroVenc = document.getElementById("conta-vencimento").value;

  const valorBaseParcela = Math.floor(totalCentavos / qtdParcelas);
  const resto = totalCentavos % qtdParcelas;

  const parcelas = [];
  const d = new Date(primeiroVenc + "T12:00:00Z");

  for (let i = 0; i < qtdParcelas; i++) {
    const vcto = new Date(d);
    vcto.setMonth(vcto.getMonth() + i);
    const valorParc = i === 0 ? valorBaseParcela + resto : valorBaseParcela;
    parcelas.push({
      valor_centavos: valorParc,
      vencimento: vcto.toISOString(),
    });
  }

  const payload = {
    descricao: document.getElementById("conta-desc").value.trim(),
    plano_codigo: tipo === "PAGAR" ? "2.1.01" : "3.1.01",
    plano_nome: tipo === "PAGAR" ? "Fornecedores" : "Vendas",
    parcelas: parcelas,
  };

  const { ok, data } = await requisitarJSON(endpoint, {
    method: "POST",
    body: JSON.stringify(payload),
  });

  if (ok) {
    avisar("Conta registrada com sucesso!");
    modalConta.style.display = "none";
    carregarContas();
  } else {
    avisar(data.erro || "Falha ao registrar conta.");
  }
});

const modalLiq = document.getElementById("modal-liquidar");
const formLiq = document.getElementById("form-liquidar");
const btnCancelaLiq = document.getElementById("btn-cancelar-liq");

function abrirModalLiquidar(id, tipo, idx, valorCents, desc, numeroParcela) {
  document.getElementById("liq-id").value = id;
  document.getElementById("liq-tipo").value = tipo;
  document.getElementById("liq-idx").value = String(idx);
  const tipoTexto =
    tipo === "PAGAMENTO" || tipo === "PAGAR" ? "PAGAR" : "RECEBER";
  document.getElementById("liquidar-info").textContent =
    `${tipoTexto}: ${desc} — Parcela ${numeroParcela || idx + 1}`;
  document.getElementById("liq-valor").value = (valorCents / 100)
    .toFixed(2)
    .replace(".", ",");
  modalLiq.style.display = "flex";
  document.getElementById("liq-forma").focus();
}

btnCancelaLiq.addEventListener("click", () => {
  modalLiq.style.display = "none";
});

formLiq.addEventListener("submit", async (e) => {
  e.preventDefault();
  const id = document.getElementById("liq-id").value;
  const tipo = document.getElementById("liq-tipo").value;
  const idx = document.getElementById("liq-idx").value;

  const endpoint =
    tipo === "PAGAMENTO" || tipo === "PAGAR"
      ? `/v1/finance/contas-pagar/${id}/parcelas/${idx}/liquidar`
      : `/v1/finance/contas-receber/${id}/parcelas/${idx}/liquidar`;

  const { ok, data } = await requisitarJSON(endpoint, {
    method: "POST",
  });

  if (ok) {
    avisar("Parcela liquidada com sucesso!");
    modalLiq.style.display = "none";
    carregarContas();
  } else {
    avisar(data.erro || "Falha ao liquidar parcela.");
  }
});

// Botão de Backup Local
document.getElementById("btn-backup").addEventListener("click", async () => {
  avisar("Gerando cópia de segurança local...");
  const { ok, data } = await requisitarJSON("/v1/sistema/backup-local", {
    method: "POST",
    body: JSON.stringify({}),
  });
  if (ok) {
    avisar(`Backup gerado: ${data.arquivo} (${data.tamanho_bytes} bytes).`);
  } else {
    avisar(data.erro || "Falha ao gerar backup.");
  }
});

// Botão Atualizar
document.getElementById("btn-atualizar").addEventListener("click", () => {
  if (ABA_ATIVA === "fluxo") carregarFluxoCaixa();
  else if (ABA_ATIVA === "pagar" || ABA_ATIVA === "receber") carregarContas();
  else if (ABA_ATIVA === "resultado") carregarResultadoDRE();
});

// Botão Voltar
document.getElementById("btn-voltar").addEventListener("click", () => {
  window.location.href = "main-menu.html";
});

// Atalhos globais do teclado
document.addEventListener("keydown", (e) => {
  if (modalConta.style.display === "flex" || modalLiq.style.display === "flex") {
    if (e.key === "Escape") {
      e.preventDefault();
      modalConta.style.display = "none";
      modalLiq.style.display = "none";
    }
    return;
  }

  const tagAtiva = document.activeElement ? document.activeElement.tagName : "";
  const ehInput = tagAtiva === "INPUT" || tagAtiva === "SELECT" || tagAtiva === "TEXTAREA";

  if (e.key === "F1") {
    e.preventDefault();
    btnNovaConta.click();
  } else if (e.key === "F2") {
    e.preventDefault();
    document.getElementById("btn-backup").click();
  } else if (e.key === "F5") {
    e.preventDefault();
    document.getElementById("btn-atualizar").click();
  } else if (e.key === "Escape") {
    e.preventDefault();
    document.getElementById("btn-voltar").click();
  } else if (!ehInput) {
    if (e.key === "1") alternarAba("vendas");
    else if (e.key === "2") alternarAba("fluxo");
    else if (e.key === "3") alternarAba("pagar");
    else if (e.key === "4") alternarAba("receber");
    else if (e.key === "5") alternarAba("resultado");
  }
});

// Carga inicial
carregarPainelChokko("hoje");

const btnTopo = document.getElementById("btn-topo-voltar"); if (btnTopo) btnTopo.addEventListener("click", () => window.location.href = "main-menu.html");
