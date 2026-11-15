/**
 * Visão geral do financeiro (antigo Resumo do Dono).
 *
 * Consolida o que o dono precisa ver de imediato — faturamento, margem, contas
 * e riscos — dentro da tela de Financeiro, evitando duas telas com a mesma
 * função. Os dados vêm de `/v1/finance/resumo` e `/v1/finance/fluxo-caixa`;
 * nenhum cálculo de preço ou margem é refeito aqui (o Rust é a autoridade).
 */

import { requisitarJSON } from "../../shared/services/api.js";
import { formatarCentavos } from "../../shared/ui/formatadores.js";
import { mostrarToast } from "../../shared/ui/feedback.js";
import {
  estadoVazio,
  graficoBarras,
  graficoBarrasComparadas,
  graficoBarrasHorizontais,
  graficoRosca,
} from "../../shared/ui/grafico.js";

let periodoAtual = "hoje";
let inicializado = false;

const elBtnHoje = document.getElementById("btn-periodo-hoje");
const elBtnSemana = document.getElementById("btn-periodo-semana");
const elBtnMes = document.getElementById("btn-periodo-mes");

const elFaturamento = document.getElementById("kpi-faturamento");
const elComparativo = document.getElementById("kpi-comparativo");
const elMargemBruta = document.getElementById("kpi-margem-bruta");
const elNumVendas = document.getElementById("kpi-num-vendas");
const elTicketMedio = document.getElementById("kpi-ticket-medio");
const elPontoEquilibrioTexto = document.getElementById("ponto-equilibrio-texto");
const elBarraProgresso = document.getElementById("barra-progresso-equilibrio");

const elAPagar7d = document.getElementById("kpi-a-pagar-7d");
const elAPagarVencidas = document.getElementById("kpi-a-pagar-vencidas");
const elAReceber7d = document.getElementById("kpi-a-receber-7d");
const elCrediarioVencido = document.getElementById("kpi-crediario-vencido");
const elSaldoCaixa = document.getElementById("kpi-saldo-caixa");
const elSaldoProjetado = document.getElementById("kpi-saldo-projetado");

const elColunaAtencao = document.getElementById("coluna-atencao-container");
const elTabelaFitaCorpo = document.getElementById("tabela-fita-corpo");

const elGraficoFluxo = document.getElementById("grafico-fluxo");
const elGraficoPagamentos = document.getElementById("grafico-pagamentos");
const elGraficoHoras = document.getElementById("grafico-horas");
const elGraficoProdutos = document.getElementById("grafico-produtos");

// Rótulos de apresentação do mix. A forma canônica vem do Rust; o fallback
// cobre valores novos sem quebrar o gráfico.
const ROTULOS_FORMA = {
  DINHEIRO: "Dinheiro",
  DEBITO: "Débito",
  CREDITO: "Crédito",
  PIX: "Pix",
  VOUCHER: "Voucher",
  CREDIARIO: "Crediário",
  OUTRO: "Outro",
};

function rotuloForma(forma) {
  const chave = String(forma ?? "").toUpperCase();
  return ROTULOS_FORMA[chave] || chave || "Outro";
}

function rotuloDia(dataIso) {
  const texto = String(dataIso ?? "");
  return texto.length >= 10 ? `${texto.slice(8, 10)}/${texto.slice(5, 7)}` : texto;
}

// Destinos internos conhecidos. Qualquer outro valor (javascript:, data:,
// URL absoluta ou página inexistente) cai no menu principal.
const PAGINAS_INTERNAS = new Set([
  "financeiro.html",
  "pos-menu.html",
  "produtos.html",
  "main-menu.html",
]);

function sanitizarLinkInterno(destino) {
  const bruto = String(destino ?? "").trim().replace(/^\.\//, "");
  const [pagina] = bruto.split("?");
  return PAGINAS_INTERNAS.has(pagina) ? bruto : "main-menu.html";
}

function trocarPeriodo(periodo) {
  periodoAtual = periodo;
  [elBtnHoje, elBtnSemana, elBtnMes].forEach((btn) => btn?.classList.remove("ativo"));
  if (periodo === "hoje") elBtnHoje?.classList.add("ativo");
  if (periodo === "semana") elBtnSemana?.classList.add("ativo");
  if (periodo === "mes") elBtnMes?.classList.add("ativo");
  carregarVisaoGeral();
}

function renderizarAtencao(itens) {
  if (!elColunaAtencao) return;
  elColunaAtencao.replaceChildren();

  if (itens.length === 0) {
    const elVazio = document.createElement("div");
    elVazio.className = "card-atencao";

    const cabecalho = document.createElement("div");
    cabecalho.className = "selo-atencao-cabecalho";
    const selo = document.createElement("span");
    selo.className = "selo-tag backup";
    selo.textContent = "TUDO EM DIA";
    cabecalho.appendChild(selo);

    const titulo = document.createElement("div");
    titulo.className = "card-atencao-titulo";
    titulo.textContent = "Sem pendências críticas";

    const descricao = document.createElement("div");
    descricao.className = "card-atencao-desc";
    descricao.textContent = "Validades, caixa e crediário sem alertas no momento.";

    elVazio.append(cabecalho, titulo, descricao);
    elColunaAtencao.appendChild(elVazio);
    return;
  }

  itens.forEach((it) => {
    const card = document.createElement("div");
    card.className = "card-atencao";

    let tagClasse = "validade";
    if (it.tipo === "QUEBRA_CAIXA") tagClasse = "quebra";
    if (it.tipo === "DEVEDOR") tagClasse = "devedor";
    if (it.tipo === "BACKUP") tagClasse = "backup";

    const cabecalho = document.createElement("div");
    cabecalho.className = "selo-atencao-cabecalho";

    const selo = document.createElement("span");
    selo.className = `selo-tag ${tagClasse}`;
    selo.textContent = String(it.titulo ?? "");
    cabecalho.appendChild(selo);

    const risco = Number(it.valor_risco_centavos ?? 0);
    if (Number.isFinite(risco) && risco > 0) {
      const valor = document.createElement("strong");
      valor.className = "selo-valor-risco";
      valor.textContent = formatarCentavos(risco);
      cabecalho.appendChild(valor);
    }

    const titulo = document.createElement("div");
    titulo.className = "card-atencao-titulo";
    titulo.textContent = String(it.subtitulo ?? "");

    const descricao = document.createElement("div");
    descricao.className = "card-atencao-desc";
    descricao.textContent = "Ação preventiva recomendada";

    const acao = document.createElement("a");
    acao.className = "card-atencao-btn";
    acao.href = sanitizarLinkInterno(it.link_destino);
    acao.textContent = String(it.acao_rotulo ?? "Abrir");

    card.append(cabecalho, titulo, descricao, acao);
    elColunaAtencao.appendChild(card);
  });
}

function renderizarFita(movimentos) {
  if (!elTabelaFitaCorpo) return;
  elTabelaFitaCorpo.replaceChildren();

  if (movimentos.length === 0) {
    const tr = document.createElement("tr");
    const td = document.createElement("td");
    td.colSpan = 4;
    td.className = "fita-vazia";
    td.textContent = "Nenhuma movimentação registrada hoje ainda.";
    tr.appendChild(td);
    elTabelaFitaCorpo.appendChild(tr);
    return;
  }

  movimentos.forEach((m) => {
    const tr = document.createElement("tr");
    // new Date() converte o UTC do banco para o fuso local do sistema.
    const hora = m.created_at
      ? new Date(m.created_at).toLocaleTimeString("pt-BR", {
          hour: "2-digit",
          minute: "2-digit",
        })
      : "--:--";
    const sinal = m.tipo === "VENDA" ? "+" : "";

    const tdHora = document.createElement("td");
    tdHora.className = "fita-hora";
    tdHora.textContent = hora;

    const tdDescricao = document.createElement("td");
    const forte = document.createElement("strong");
    forte.textContent = String(m.descricao ?? "");
    tdDescricao.appendChild(forte);

    const tdDetalhe = document.createElement("td");
    tdDetalhe.className = "fita-detalhe";
    tdDetalhe.textContent = String(m.detalhe ?? "");

    const tdValor = document.createElement("td");
    tdValor.className = "direita fita-valor";
    tdValor.textContent = `${sinal}${formatarCentavos(m.valor_centavos)}`;

    tr.append(tdHora, tdDescricao, tdDetalhe, tdValor);
    elTabelaFitaCorpo.appendChild(tr);
  });
}

function renderizarResumo(d) {
  if (elFaturamento) elFaturamento.textContent = formatarCentavos(d.faturamento_centavos);
  if (elMargemBruta) elMargemBruta.textContent = `${d.margem_bruta_pct.toFixed(1)}%`;
  if (elNumVendas) elNumVendas.textContent = d.num_vendas;
  if (elTicketMedio) elTicketMedio.textContent = formatarCentavos(d.ticket_medio_centavos);

  if (elComparativo) {
    if (d.comparativo_faturamento_pct !== null && d.comparativo_faturamento_pct !== undefined) {
      const pct = d.comparativo_faturamento_pct;
      const sinal = pct >= 0 ? "+" : "";
      elComparativo.textContent = `${sinal}${pct.toFixed(1)}% vs anterior`;
      elComparativo.className = `comparativo-tag ${pct >= 0 ? "positivo" : "negativo"}`;
      elComparativo.style.display = "inline-block";
    } else {
      elComparativo.style.display = "none";
    }
  }

  // Ponto de equilíbrio
  if (elPontoEquilibrioTexto && elBarraProgresso) {
    if (d.ponto_equilibrio_centavos > 0) {
      const cob = Math.min(d.cobertura_ponto_equilibrio_pct, 100);
      const faltam = Math.max(0, d.ponto_equilibrio_centavos - d.faturamento_centavos);
      elPontoEquilibrioTexto.textContent = `${d.cobertura_ponto_equilibrio_pct.toFixed(0)}% coberto • Faltam ${formatarCentavos(faltam)} para o equilíbrio`;
      elBarraProgresso.style.width = `${cob}%`;
    } else {
      elPontoEquilibrioTexto.textContent =
        "Cadastre suas despesas fixas para acompanhar o ponto de equilíbrio";
      elBarraProgresso.style.width = "0%";
    }
  }

  // Faixa dinheiro e contas
  if (elAPagar7d) elAPagar7d.textContent = formatarCentavos(d.a_pagar_7d_centavos);
  if (elAPagarVencidas) {
    elAPagarVencidas.textContent = `${d.num_contas_pagar_vencidas} vencidas (${formatarCentavos(d.a_pagar_vencidas_centavos)})`;
    elAPagarVencidas.classList.toggle("valor-negativo", d.num_contas_pagar_vencidas > 0);
  }
  if (elAReceber7d) elAReceber7d.textContent = formatarCentavos(d.a_receber_7d_centavos);
  if (elCrediarioVencido) elCrediarioVencido.textContent = formatarCentavos(d.crediario_vencido_centavos);
  if (elSaldoCaixa) elSaldoCaixa.textContent = formatarCentavos(d.saldo_caixa_dinheiro_centavos);
  if (elSaldoProjetado) {
    elSaldoProjetado.textContent = formatarCentavos(d.saldo_projetado_7d_centavos);
    elSaldoProjetado.classList.toggle("valor-positivo", d.saldo_projetado_7d_centavos >= 0);
    elSaldoProjetado.classList.toggle("valor-negativo", d.saldo_projetado_7d_centavos < 0);
  }

  renderizarAtencao(d.atencao_itens || []);
  renderizarFita(d.fita_do_dia || []);
}

async function carregarFluxoCaixa() {
  if (!elGraficoFluxo) return;
  try {
    const { ok, data } = await requisitarJSON("/v1/finance/fluxo-caixa");
    if (!ok || !data || !Array.isArray(data.dias)) {
      estadoVazio(elGraficoFluxo, "Não foi possível carregar o fluxo de caixa.");
      return;
    }

    const itens = data.dias
      .slice(0, 30)
      .reverse()
      .map((d) => ({
        rotulo: rotuloDia(d.data),
        a: Number(d.entradas_centavos) || 0,
        b: Number(d.saidas_centavos) || 0,
      }));

    if (itens.length === 0 || itens.every((i) => i.a <= 0 && i.b <= 0)) {
      estadoVazio(elGraficoFluxo, "Sem movimentação de caixa nos últimos 30 dias.");
      return;
    }

    graficoBarrasComparadas(elGraficoFluxo, {
      itens,
      descricao: "Entradas e saídas de caixa por dia nos últimos 30 dias",
      rotuloA: "Entradas",
      rotuloB: "Saídas",
    });
  } catch {
    estadoVazio(elGraficoFluxo, "Falha ao carregar o fluxo de caixa.");
  }
}

async function carregarAnalytics() {
  let dados = null;
  try {
    const { ok, data } = await requisitarJSON(
      `/v1/analytics/vendas?periodo=${encodeURIComponent(periodoAtual)}`,
    );
    if (ok && data) dados = data;
  } catch {
    dados = null;
  }

  if (!dados) {
    estadoVazio(elGraficoPagamentos, "Não foi possível carregar as formas de pagamento.");
    estadoVazio(elGraficoHoras, "Não foi possível carregar as vendas por hora.");
    estadoVazio(elGraficoProdutos, "Não foi possível carregar os produtos.");
    return;
  }

  // Mix de pagamento: a forma já chega separada (CREDIARIO não se esconde em OUTRO).
  const fatias = (dados.formas_pagamento || []).map((f) => ({
    rotulo: rotuloForma(f.forma),
    valor: Number(f.valor_centavos) || 0,
  }));
  graficoRosca(elGraficoPagamentos, {
    fatias,
    descricao: "Participação de cada forma de pagamento no faturamento do período",
    tituloCentro: "Faturamento",
  });

  // Vendas por hora: 24 posições com zero para mostrar o horário de pico real.
  const horas = (dados.serie_hora || []).map((h) => ({
    rotulo: `${String(h.hora).padStart(2, "0")}h`,
    valor: Number(h.faturamento_centavos) || 0,
  }));
  if (horas.length === 0 || horas.every((h) => h.valor <= 0)) {
    estadoVazio(elGraficoHoras, "Sem vendas no período selecionado.");
  } else {
    graficoBarras(elGraficoHoras, {
      itens: horas,
      descricao: "Faturamento por hora do dia no período selecionado",
    });
  }

  // Ranking de produtos com a margem que o Rust calculou (custo congelado).
  const produtos = (dados.top_produtos || []).slice(0, 6).map((p) => ({
    rotulo: p.nome,
    valor: Number(p.faturamento_centavos) || 0,
    detalhe: `${p.quantidade} un · ${Math.round(Number(p.margem_pct) || 0)}% margem`,
  }));
  if (produtos.length === 0) {
    estadoVazio(elGraficoProdutos, "Sem produtos vendidos no período selecionado.");
  } else {
    graficoBarrasHorizontais(elGraficoProdutos, {
      itens: produtos,
      descricao: "Produtos com maior faturamento e a margem de cada um",
    });
  }
}

function carregarGraficos() {
  carregarFluxoCaixa();
  carregarAnalytics();
}

export async function carregarVisaoGeral() {
  try {
    const { ok, data } = await requisitarJSON(`/v1/finance/resumo?periodo=${periodoAtual}`);
    if (!ok || !data) {
      mostrarToast("Não foi possível carregar a visão geral.");
      return;
    }
    renderizarResumo(data);
  } catch {
    mostrarToast("Erro de comunicação ao carregar a visão geral.");
  }

  carregarGraficos();
}

/**
 * Liga o seletor de período e carrega os dados. Idempotente: a aba pode ser
 * reaberta sem duplicar listeners.
 */
export function inicializarVisaoGeral() {
  if (inicializado) {
    carregarVisaoGeral();
    return;
  }
  inicializado = true;

  elBtnHoje?.addEventListener("click", () => trocarPeriodo("hoje"));
  elBtnSemana?.addEventListener("click", () => trocarPeriodo("semana"));
  elBtnMes?.addEventListener("click", () => trocarPeriodo("mes"));

  carregarVisaoGeral();
}
