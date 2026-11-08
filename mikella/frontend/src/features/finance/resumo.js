import { garantirSessao, requisitarJSON } from "../../shared/services/api.js";
import { formatarCentavos } from "../../shared/ui/formatadores.js";
import { mostrarToast } from "../../shared/ui/feedback.js";
import { inicializarControlesJanela } from "../../shared/ui/window-controls.js";

inicializarControlesJanela();
garantirSessao();

let periodoAtual = "hoje";

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
const elGraficoSvg = document.getElementById("grafico-svg-container");

const elBtnIrFinanceiro = document.getElementById("btn-ir-financeiro");
const elBtnIrCaixa = document.getElementById("btn-ir-caixa");
const elBtnVoltar = document.getElementById("btn-voltar");

function bindEventos() {
  elBtnHoje?.addEventListener("click", () => trocarPeriodo("hoje"));
  elBtnSemana?.addEventListener("click", () => trocarPeriodo("semana"));
  elBtnMes?.addEventListener("click", () => trocarPeriodo("mes"));

  elBtnIrFinanceiro?.addEventListener("click", () => {
    window.location.href = "financeiro.html";
  });
  elBtnIrCaixa?.addEventListener("click", () => {
    window.location.href = "pos-menu.html";
  });
  elBtnVoltar?.addEventListener("click", () => {
    window.location.href = "main-menu.html";
  });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      window.location.href = "main-menu.html";
    } else if (e.key === "F1") {
      e.preventDefault();
      window.location.href = "financeiro.html";
    } else if (e.key === "F2") {
      e.preventDefault();
      window.location.href = "pos-menu.html";
    }
  });
}

function trocarPeriodo(periodo) {
  periodoAtual = periodo;
  [elBtnHoje, elBtnSemana, elBtnMes].forEach((btn) => btn?.classList.remove("ativo"));
  if (periodo === "hoje") elBtnHoje?.classList.add("ativo");
  if (periodo === "semana") elBtnSemana?.classList.add("ativo");
  if (periodo === "mes") elBtnMes?.classList.add("ativo");
  carregarResumo();
}

async function carregarResumo() {
  try {
    const { ok, data } = await requisitarJSON(`/v1/finance/resumo?periodo=${periodoAtual}`);
    if (!ok || !data) {
      mostrarToast("Não foi possível carregar o resumo executivo.", "aviso");
      return;
    }
    renderizarResumo(data);
  } catch (err) {
    mostrarToast("Erro de comunicação ao carregar resumo.", "perigo");
  }

  // Carrega o histórico para o gráfico SVG
  carregarGrafico();
}

function renderizarResumo(d) {
  // 1. Dominante: Faturamento e margem
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
      elPontoEquilibrioTexto.textContent = "Cadastre suas despesas fixas para acompanhar o ponto de equilíbrio";
      elBarraProgresso.style.width = "0%";
    }
  }

  // 2. Faixa Dinheiro e Contas
  if (elAPagar7d) elAPagar7d.textContent = formatarCentavos(d.a_pagar_7d_centavos);
  if (elAPagarVencidas) {
    elAPagarVencidas.textContent = `${d.num_contas_pagar_vencidas} vencidas (${formatarCentavos(d.a_pagar_vencidas_centavos)})`;
    elAPagarVencidas.style.color = d.num_contas_pagar_vencidas > 0 ? "var(--vermelho)" : "var(--cinza-500)";
  }
  if (elAReceber7d) elAReceber7d.textContent = formatarCentavos(d.a_receber_7d_centavos);
  if (elCrediarioVencido) elCrediarioVencido.textContent = formatarCentavos(d.crediario_vencido_centavos);
  if (elSaldoCaixa) elSaldoCaixa.textContent = formatarCentavos(d.saldo_caixa_dinheiro_centavos);
  if (elSaldoProjetado) {
    elSaldoProjetado.textContent = formatarCentavos(d.saldo_projetado_7d_centavos);
    elSaldoProjetado.style.color = d.saldo_projetado_7d_centavos >= 0 ? "#047857" : "var(--vermelho)";
  }

  // 3. Coluna de Atenção (Máximo 3 selos)
  renderizarAtencao(d.atencao_itens || []);

  // 4. Fita do dia
  renderizarFita(d.fita_do_dia || []);
}

function renderizarAtencao(itens) {
  if (!elColunaAtencao) return;
  elColunaAtencao.innerHTML = "";

  if (itens.length === 0) {
    const elVazio = document.createElement("div");
    elVazio.className = "card-atencao";
    elVazio.innerHTML = `
      <div class="selo-atencao-cabecalho">
        <span class="selo-tag backup">TUDO EM DIA</span>
      </div>
      <div class="card-atencao-titulo">Sem pendências críticas</div>
      <div class="card-atencao-desc">Validades, caixa e crediário sem alertas no momento.</div>
    `;
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

    card.innerHTML = `
      <div class="selo-atencao-cabecalho">
        <span class="selo-tag ${tagClasse}">${it.titulo}</span>
        ${it.valor_risco_centavos > 0 ? `<strong style="font-size:0.85rem;">${formatarCentavos(it.valor_risco_centavos)}</strong>` : ""}
      </div>
      <div class="card-atencao-titulo">${it.subtitulo}</div>
      <div class="card-atencao-desc">Ação preventiva recomendada</div>
      <a href="${it.link_destino}" class="card-atencao-btn">${it.acao_rotulo}</a>
    `;
    elColunaAtencao.appendChild(card);
  });
}

function renderizarFita(movimentos) {
  if (!elTabelaFitaCorpo) return;
  elTabelaFitaCorpo.innerHTML = "";

  if (movimentos.length === 0) {
    elTabelaFitaCorpo.innerHTML = `<tr><td colspan="4" style="text-align:center; padding:var(--espaco-3); color:var(--cinza-500);">Nenhuma movimentação registrada hoje ainda.</td></tr>`;
    return;
  }

  movimentos.forEach((m) => {
    const tr = document.createElement("tr");
    // BUG CORRIGIDO: substring(11,16) extraía a hora UTC — 3h a mais em São Paulo.
    // Usar new Date() converte corretamente para o fuso local do sistema operacional.
    const hora = m.created_at
      ? new Date(m.created_at).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })
      : "--:--";
    const sinal = m.tipo === "VENDA" ? "+" : "";
    tr.innerHTML = `
      <td style="font-weight:bold; font-size:0.85rem;">${hora}</td>
      <td><strong>${m.descricao}</strong></td>
      <td style="color:var(--cinza-500); font-size:0.85rem;">${m.detalhe}</td>
      <td class="direita" style="font-weight:900; font-variant-numeric: tabular-nums lining-nums;">${sinal}${formatarCentavos(m.valor_centavos)}</td>
    `;
    elTabelaFitaCorpo.appendChild(tr);
  });
}

async function carregarGrafico() {
  if (!elGraficoSvg) return;
  try {
    const { ok, data } = await requisitarJSON("/v1/finance/fluxo-caixa");
    if (!ok || !data || !data.dias) return;

    const dias = data.dias.slice(0, 14).reverse();
    if (dias.length === 0) {
      elGraficoSvg.innerHTML = `<div style="padding:var(--espaco-3); color:var(--cinza-500); text-align:center;">Sem dados suficientes para o gráfico no período.</div>`;
      return;
    }

    // Geração de SVG Puro (VIZ-010 / VIZ-020) sem bibliotecas externas
    const largura = 600;
    const altura = 160;
    const margem = { top: 20, right: 20, bottom: 30, left: 50 };
    const maxValor = Math.max(...dias.map((d) => d.entradas_centavos), 10000);

    const barraLargura = Math.max(16, (largura - margem.left - margem.right) / dias.length - 8);

    let svgBarras = "";
    let svgRotulosX = "";

    dias.forEach((d, i) => {
      const x = margem.left + i * (barraLargura + 8);
      const h = (d.entradas_centavos / maxValor) * (altura - margem.top - margem.bottom);
      const y = altura - margem.bottom - h;
      const diaTxt = d.data.substring(8, 10);

      // Série principal em azul cobalto #0040D8 com contorno preto 1.5px (VIZ-010)
      svgBarras += `
        <rect x="${x}" y="${y}" width="${barraLargura}" height="${h}" fill="#0040D8" stroke="#000000" stroke-width="1.5">
          <title>${d.data}: ${formatarCentavos(d.entradas_centavos)}</title>
        </rect>
      `;
      svgRotulosX += `
        <text x="${x + barraLargura / 2}" y="${altura - 10}" font-size="11" font-weight="bold" text-anchor="middle" fill="#000000">${diaTxt}</text>
      `;
    });

    elGraficoSvg.innerHTML = `
      <svg viewBox="0 0 ${largura} ${altura}" style="width:100%; height:auto; display:block;">
        <!-- Eixo Base 2px preto -->
        <line x1="${margem.left}" y1="${altura - margem.bottom}" x2="${largura - margem.right}" y2="${altura - margem.bottom}" stroke="#000000" stroke-width="2" />
        <!-- Barras da Série -->
        ${svgBarras}
        <!-- Rótulos do Eixo X -->
        ${svgRotulosX}
      </svg>
    `;
  } catch (_) {}
}

bindEventos();
carregarResumo();
