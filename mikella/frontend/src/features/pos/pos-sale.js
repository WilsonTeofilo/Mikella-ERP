import { garantirSessao, requisitarJSON } from "../../shared/services/api.js";
import { mostrarToast } from "../../shared/ui/feedback.js";
import {
  formatarCentavos,
  parsearCentavos,
  formatarCPF,
  formatarTelefone,
  normalizarBusca,
  somenteDigitos,
  formatarCampoMoeda,
} from "../../shared/ui/formatadores.js";
import {
  obterIconePagamento,
  montarGradePagamentos,
} from "../../shared/ui/payment-icons.js";
import { inicializarControlesJanela } from "../../shared/ui/window-controls.js";
import { inicializarConsultaPreco } from "./consulta-preco.js";

inicializarControlesJanela();
garantirSessao();

const avisar = mostrarToast;
const consultaPreco = inicializarConsultaPreco();

const urlParams = new URLSearchParams(window.location.search);
const modoVenda = urlParams.get("modo") || "balcao";
const comandaNum = urlParams.get("comanda");

let PRODUTOS_CACHE = [];
let CLIENTES_CACHE = [];
let clienteSelecionado = null; // { id, nome, cpf, telefone }
let formaPreSelecionada = "DINHEIRO";
let ultimaVendaIdConcluida = null;
// Limites da loja (migração 0012). Enquanto a API não responde, valem os
// defaults do servidor; a venda é revalidada no backend de qualquer forma.
let configPagamento = {
  max_parcelas_cartao: 12,
  prazo_crediario_dias: 30,
};

const VENDA = {
  // Chave de idempotência do rascunho (ADR-0002). Só muda quando o carrinho
  // muda; um retry após falha ambígua reenvia exatamente o mesmo venda_id.
  venda_id: crypto.randomUUID(),
  itens: [],
  pagamentos: [], // { forma, valor_centavos, recebido_centavos }
  total_centavos: 0,
  subtotal_centavos: 0,
  desconto_centavos: 0,
};

// Inicialização
async function inicializar() {
  // Ajusta modo e comanda no topo
  if (modoVenda === "comanda" && comandaNum) {
    document.getElementById("badge-modo").textContent = "VENDA COMANDA";
    const bCom = document.getElementById("badge-comanda");
    bCom.style.display = "inline-block";
    bCom.textContent = `COMANDA Nº ${comandaNum}`;
  } else {
    document.getElementById("badge-modo").textContent = "VENDA BALCÃO";
  }

  // Carrega catálogo de produtos
  const { ok, data } = await requisitarJSON("/v1/produtos");
  if (ok && data.produtos) {
    PRODUTOS_CACHE = data.produtos;
  }

  // Verifica status real da sessão do caixa (UI-004)
  const badgeSessao = document.getElementById("badge-sessao-status");
  const resCaixa = await requisitarJSON("/v1/caixa/status");
  if (resCaixa.ok && resCaixa.data && resCaixa.data.aberta) {
    localStorage.setItem("mikella_sessao_id", resCaixa.data.sessao_id);
    if (badgeSessao) {
      badgeSessao.style.background = "var(--verde)";
      badgeSessao.style.color = "var(--preto)";
      badgeSessao.textContent = `CAIXA 0${resCaixa.data.numero_caixa || 1} ABERTO`;
    }
  } else {
    localStorage.removeItem("mikella_sessao_id");
    if (badgeSessao) {
      badgeSessao.style.background = "var(--vermelho)";
      badgeSessao.style.color = "var(--branco)";
      badgeSessao.textContent = "CAIXA FECHADO";
    }
    avisar("Atenção: Caixa fechado! Abra o caixa no Menu PDV para realizar vendas.");
  }

  // Carrega clientes
  const resCli = await requisitarJSON("/v1/clientes");
  if (resCli.ok && resCli.data.clientes) {
    CLIENTES_CACHE = resCli.data.clientes;
  }

  // Limites de parcelamento e prazo do crediário definidos pela loja
  const resConfig = await requisitarJSON("/v1/config/pagamento");
  if (resConfig.ok && resConfig.data) {
    configPagamento = { ...configPagamento, ...resConfig.data };
    aplicarConfigPagamento();
  }

  // Foco no campo de produto
  document.getElementById("produto-input").focus();
}

function aplicarConfigPagamento() {
  const inputParcelas = document.getElementById("pg-input-parcelas");
  if (inputParcelas) {
    inputParcelas.max = String(configPagamento.max_parcelas_cartao);
    const atual = parseInt(inputParcelas.value, 10) || 1;
    if (atual > configPagamento.max_parcelas_cartao) {
      inputParcelas.value = String(configPagamento.max_parcelas_cartao);
    }
  }

  const prazo = document.getElementById("pg-crediario-prazo");
  if (prazo) {
    prazo.textContent = `Prazo da loja: ${configPagamento.prazo_crediario_dias} dias para quitação.`;
  }
}

// Bipagem / Adição de Produto com Janela de Sugestões Conforme Esboço do Paint (BUS-001 a BUS-016)
const produtoInput = document.getElementById("produto-input");
const sugestoesBox = document.getElementById("sugestoes-produtos");
let indiceSugestaoSelecionada = 0;
let sugestoesAtuais = [];
let debounceBuscaTimer = null;

function fecharSugestoesProdutos() {
  if (sugestoesBox) {
    sugestoesBox.style.display = "none";
    sugestoesBox.replaceChildren();
  }
  indiceSugestaoSelecionada = -1;
  sugestoesAtuais = [];
}

function atualizarDestaqueSugestao() {
  if (!sugestoesBox) return;
  const itens = sugestoesBox.querySelectorAll(".item-sugestao");
  itens.forEach((el, idx) => {
    el.classList.toggle("selecionado", idx === indiceSugestaoSelecionada);
    if (idx === indiceSugestaoSelecionada) {
      el.scrollIntoView({ block: "nearest" });
    }
  });
}

function exibirSugestoesProdutos(lista, termoOriginal) {
  if (!sugestoesBox) return;
  sugestoesAtuais = lista;
  sugestoesBox.replaceChildren();

  if (lista.length === 0) {
    const vazio = document.createElement("div");
    vazio.className = "item-sugestao-vazio";
    vazio.textContent = `Nenhum produto para "${termoOriginal}".`;
    sugestoesBox.appendChild(vazio);
    sugestoesBox.style.display = "block";
    indiceSugestaoSelecionada = -1;
    return;
  }

  // BUS-003: Primeira linha já vem destacada por padrão
  indiceSugestaoSelecionada = 0;

  lista.forEach((p, idx) => {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = `item-sugestao${idx === 0 ? " selecionado" : ""}`;

    // Coluna esquerda: Nome
    const divEsq = document.createElement("div");
    divEsq.style.display = "flex";
    divEsq.style.flexDirection = "column";

    const nomeDiv = document.createElement("div");
    nomeDiv.className = "item-sugestao-nome";
    nomeDiv.textContent = p.nome;

    divEsq.appendChild(nomeDiv);

    // Coluna centro: Código / SKU
    const cod = p.codigo_barras || p.sku || "—";
    const metaDiv = document.createElement("div");
    metaDiv.className = "item-sugestao-meta";
    metaDiv.textContent = `COD: ${cod}`;
    divEsq.appendChild(metaDiv);

    // Coluna direita: Estoque e Preço (conforme esboço do Paint)
    const divDir = document.createElement("div");
    divDir.className = "item-sugestao-direita";

    if (p.estoque_atual !== undefined) {
      const estDiv = document.createElement("div");
      estDiv.className = "item-sugestao-estoque";
      if (p.estoque_atual <= 0) {
        estDiv.style.color = "var(--vermelho)";
        estDiv.textContent = "Sem estoque";
      } else if (p.estoque_atual <= (p.estoque_minimo || 5)) {
        estDiv.style.color = "#d97706";
        estDiv.textContent = `Est: ${p.estoque_atual} (Baixo)`;
      } else {
        estDiv.textContent = `Est: ${p.estoque_atual}`;
      }
      divDir.appendChild(estDiv);
    }

    const precoDiv = document.createElement("div");
    precoDiv.className = "item-sugestao-preco";
    precoDiv.textContent = `PRE: ${formatarCentavos(p.preco_centavos)}`;
    divDir.appendChild(precoDiv);

    btn.append(divEsq, divDir);

    btn.addEventListener("click", () => {
      adicionarItemVenda(p);
      produtoInput.value = "";
      fecharSugestoesProdutos();
      produtoInput.focus();
    });

    sugestoesBox.appendChild(btn);
  });

  sugestoesBox.style.display = "block";
}

// BUS-010: Debounce de 120ms e filtro inteligente por palavras (BUS-011)
produtoInput.addEventListener("input", () => {
  clearTimeout(debounceBuscaTimer);
  debounceBuscaTimer = setTimeout(() => {
    const termoOriginal = produtoInput.value.trim();
    const termoNorm = normalizarBusca(termoOriginal);

    if (termoNorm.length < 2) {
      fecharSugestoesProdutos();
      return;
    }

    const apenasNumeros = /^[0-9]+$/.test(termoOriginal);

    let filtrados = [];
    if (apenasNumeros) {
      // BUS-011: Busca por código de barras ou SKU
      filtrados = PRODUTOS_CACHE.filter(
        (p) =>
          (p.codigo_barras && p.codigo_barras.includes(termoOriginal)) ||
          (p.sku && p.sku.toLowerCase().includes(termoNorm)),
      );
    } else {
      // BUS-011: Busca por qualquer palavra do nome (ex: "cola 2l" acha "Coca-Cola 2L")
      const palavras = termoNorm.split(/\s+/).filter((w) => w.length > 0);
      filtrados = PRODUTOS_CACHE.filter((p) => {
        const nomeNorm = normalizarBusca(p.nome);
        return palavras.every((w) => nomeNorm.includes(w));
      });
    }

    exibirSugestoesProdutos(filtrados.slice(0, 8), termoOriginal);
  }, 120);
});

produtoInput.addEventListener("keydown", (e) => {
  const listaAberta = sugestoesBox && sugestoesBox.style.display === "block";

  if (listaAberta && sugestoesAtuais.length > 0) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      indiceSugestaoSelecionada = Math.min(
        indiceSugestaoSelecionada + 1,
        sugestoesAtuais.length - 1,
      );
      atualizarDestaqueSugestao();
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      indiceSugestaoSelecionada = Math.max(indiceSugestaoSelecionada - 1, 0);
      atualizarDestaqueSugestao();
      return;
    }
    if (e.key === "Enter" && indiceSugestaoSelecionada >= 0) {
      e.preventDefault();
      adicionarItemVenda(sugestoesAtuais[indiceSugestaoSelecionada]);
      produtoInput.value = "";
      fecharSugestoesProdutos();
      return;
    }
  }

  if (e.key === "Enter") {
    e.preventDefault();
    fecharSugestoesProdutos();
    biparProduto();
  } else if (e.key === "Escape") {
    // BUS-014: Esc fecha a lista mantendo o texto; segundo Esc limpa o campo
    if (listaAberta) {
      e.preventDefault();
      fecharSugestoesProdutos();
    } else if (produtoInput.value.length > 0) {
      e.preventDefault();
      produtoInput.value = "";
    } else {
      e.preventDefault();
      tentarSairDoCaixa();
    }
  }
});

function biparProduto() {
  const termo = produtoInput.value.trim();
  if (!termo) return;

  const termoNorm = normalizarBusca(termo);

  // BUS-012: Código de barras exato adiciona direto
  const p =
    PRODUTOS_CACHE.find(
      (prod) =>
        (prod.codigo_barras && prod.codigo_barras === termo) ||
        (prod.sku && prod.sku.toLowerCase() === termoNorm) ||
        normalizarBusca(prod.nome) === termoNorm,
    ) ||
    PRODUTOS_CACHE.find((prod) => normalizarBusca(prod.nome).includes(termoNorm));

  if (p) {
    adicionarItemVenda(p);
    produtoInput.value = "";
  } else {
    avisar(`Produto não encontrado: ${termo}`);
  }
}

function adicionarItemVenda(prod) {
  const itemExistente = VENDA.itens.find((i) => i.id === prod.id);
  if (itemExistente) {
    itemExistente.quantidade += 1;
  } else {
    VENDA.itens.push({
      id: prod.id,
      nome: prod.nome,
      quantidade: 1,
      preco_centavos: prod.preco_centavos,
      desconto_centavos: 0,
    });
  }
  renderItens();
}

function renderItens() {
  const tbody = document.getElementById("itens-lista");
  tbody.replaceChildren();
  let subtotal = 0;
  let desc = 0;
  let qtdItens = 0;

  VENDA.itens.forEach((it, idx) => {
    const itemSubtotal =
      it.preco_centavos * it.quantidade - it.desconto_centavos;
    subtotal += it.preco_centavos * it.quantidade;
    desc += it.desconto_centavos;
    qtdItens += it.quantidade;

    const tr = document.createElement("tr");

    const tdNum = document.createElement("td");
    const numForte = document.createElement("strong");
    numForte.textContent = String(idx + 1);
    tdNum.appendChild(numForte);

    const tdNome = document.createElement("td");
    const nomeForte = document.createElement("strong");
    nomeForte.textContent = it.nome;
    tdNome.appendChild(nomeForte);

    const tdQtd = document.createElement("td");
    tdQtd.className = "direita";
    tdQtd.textContent = String(it.quantidade);

    const tdPreco = document.createElement("td");
    tdPreco.className = "direita";
    tdPreco.textContent = formatarCentavos(it.preco_centavos);

    const tdDesc = document.createElement("td");
    tdDesc.className = "direita";
    tdDesc.textContent = formatarCentavos(it.desconto_centavos);

    const tdSubtotal = document.createElement("td");
    tdSubtotal.className = "direita";
    const subtotalForte = document.createElement("strong");
    subtotalForte.textContent = formatarCentavos(itemSubtotal);
    tdSubtotal.appendChild(subtotalForte);

    tr.append(tdNum, tdNome, tdQtd, tdPreco, tdDesc, tdSubtotal);
    tbody.appendChild(tr);
  });

  VENDA.subtotal_centavos = subtotal;
  VENDA.desconto_centavos = desc;
  VENDA.total_centavos = Math.max(0, subtotal - desc);

  // Qualquer mutação do carrinho gera um novo rascunho: o venda_id anterior
  // não pode ser reutilizado para uma venda diferente.
  VENDA.venda_id = crypto.randomUUID();

  document.getElementById("total-valor").textContent = formatarCentavos(
    VENDA.total_centavos,
  );
  const textoDestino = comandaNum ? "comanda" : "venda";
  const textoQtd = qtdItens === 1 ? "1 item" : `${qtdItens} itens`;
  document.getElementById("total-meta").textContent =
    `${textoQtd} na ${textoDestino}`;
  document.getElementById("resumo-subtotal").textContent =
    formatarCentavos(subtotal);
  document.getElementById("resumo-desconto").textContent =
    formatarCentavos(desc);

  atualizarFitaLateral();

  // Auto scroll para a última linha
  const container = document.querySelector(".fita-itens-container");
  container.scrollTop = container.scrollHeight;
}

// F2: Buscar / Identificar Cliente
function abrirModalCliente() {
  abrirModal("modal-cliente-busca");
  const input = document.getElementById("input-busca-cli-termo");
  input.value = "";
  const box = document.getElementById("cli-resultados-lista");
  box.replaceChildren();
  box.style.display = "none";
  input.focus();
}

const inputCliTermo = document.getElementById("input-busca-cli-termo");
inputCliTermo.addEventListener("input", () => {
  const t = inputCliTermo.value.trim().toLowerCase();
  const box = document.getElementById("cli-resultados-lista");

  if (t.length < 2) {
    box.replaceChildren();
    box.style.display = "none";
    return;
  }

  const tDigitos = somenteDigitos(t);
  const filtrados = CLIENTES_CACHE.filter((c) => {
    const nomeBate = c.nome.toLowerCase().includes(t);
    const cpfBate =
      (c.cpf && c.cpf.includes(tDigitos)) ||
      (c.cpf && formatarCPF(c.cpf).toLowerCase().includes(t));
    const telBate =
      (c.telefone && c.telefone.includes(tDigitos)) ||
      (c.telefone && formatarTelefone(c.telefone).toLowerCase().includes(t));
    return nomeBate || cpfBate || telBate;
  });

  box.style.display = "flex";
  renderResultadosClientes(filtrados);
});

function renderResultadosClientes(lista) {
  const box = document.getElementById("cli-resultados-lista");
  box.replaceChildren();

  if (lista.length === 0) {
    const vazio = document.createElement("div");
    vazio.style.padding = "8px";
    vazio.style.fontSize = "0.9rem";
    vazio.style.color = "var(--cinza-500)";
    vazio.textContent = "Nenhum cliente encontrado com estes dados.";
    box.appendChild(vazio);
    return;
  }

  lista.slice(0, 8).forEach((c) => {
    const item = document.createElement("button");
    item.className = "botao";
    item.style.textAlign = "left";
    item.style.justifyContent = "space-between";
    item.style.padding = "6px 12px";

    const esquerda = document.createElement("span");
    const nomeForte = document.createElement("strong");
    nomeForte.textContent = c.nome;
    const docInfo = c.cpf ? formatarCPF(c.cpf) : c.telefone ? formatarTelefone(c.telefone) : "Sem documento";
    esquerda.append(
      nomeForte,
      ` (${docInfo})`,
    );

    const direita = document.createElement("span");
    direita.style.fontSize = "0.8rem";
    const temDivida = c.saldo_devedor_centavos > 0;
    direita.style.color = temDivida ? "var(--vermelho)" : "var(--verde)";
    direita.textContent = temDivida
      ? `Débito: ${c.saldo_devedor_fmt}`
      : "Sem débito";

    item.append(esquerda, direita);
    item.onclick = () => vincularCliente(c);
    box.appendChild(item);
  });
}

function vincularCliente(c) {
  clienteSelecionado = c;
  const badge = document.getElementById("badge-cliente");
  const docExibicao = c.cpf ? formatarCPF(c.cpf) : "NÃO INFORMADO";
  badge.textContent = `CLIENTE: ${c.nome.toUpperCase()} | CPF: ${docExibicao}`;
  badge.style.background = "var(--amarelo)";
  fecharModais();
  avisar(`Cliente vinculado: ${c.nome}`);
  produtoInput.focus();
}

document.getElementById("btn-confirmar-cli").onclick = () => {
  const termo = inputCliTermo.value.trim();
  if (!termo) {
    avisar("Digite o CPF, celular ou nome do cliente.");
    return;
  }
  // Se não clicou em um cliente existente, vincula com o CPF digitado
  vincularCliente({
    id: null,
    nome: "Consumidor Identificado",
    cpf: termo,
    telefone: termo,
  });
};

document.getElementById("btn-consumidor-final").onclick = () => {
  clienteSelecionado = null;
  document.getElementById("badge-cliente").textContent =
    "CONSUMIDOR FINAL (SEM CPF)";
  document.getElementById("badge-cliente").style.background =
    "var(--amarelo, #FFD21F)";
  fecharModais();
  produtoInput.focus();
};

document.getElementById("btn-cancelar-cli-modal").onclick = () => {
  fecharModais();
  produtoInput.focus();
};

let formaAtualPagamento = "DINHEIRO";

function calcularSaldoRestante() {
  const somaPagos = VENDA.pagamentos.reduce(
    (acc, p) => acc + p.valor_centavos,
    0,
  );
  return Math.max(0, VENDA.total_centavos - somaPagos);
}

function calcularParcelasTexto(valorCentavos, numParcelas) {
  if (numParcelas <= 1) {
    return `1x de ${formatarCentavos(valorCentavos)} (à vista)`;
  }
  const baseCentavos = Math.floor(valorCentavos / numParcelas);
  const resto = valorCentavos % numParcelas;
  if (resto === 0) {
    return `${numParcelas}x de ${formatarCentavos(baseCentavos)} sem juros`;
  }
  const primeiraParcela = baseCentavos + resto;
  return `${numParcelas}x (1x de ${formatarCentavos(primeiraParcela)} + ${numParcelas - 1}x de ${formatarCentavos(baseCentavos)}) • Total: ${formatarCentavos(valorCentavos)}`;
}

function atualizarFitaLateral() {
  const painel = document.getElementById("painel-pagamentos-lateral");
  const lista = document.getElementById("fita-pagamentos-lista");
  const lblFalta = document.getElementById("fita-falta-valor");
  if (!painel || !lista) return;

  if (VENDA.pagamentos.length === 0) {
    painel.style.display = "none";
    lista.replaceChildren();
    return;
  }

  painel.style.display = "block";
  lista.replaceChildren();

  VENDA.pagamentos.forEach((p, idx) => {
    const item = document.createElement("div");
    item.style.display = "flex";
    item.style.justifyContent = "space-between";
    item.style.alignItems = "center";
    item.style.fontSize = "0.84rem";

    const desc = document.createElement("span");
    const parcTexto = p.parcelas && p.parcelas > 1 ? ` (${p.parcelas}x)` : "";
    desc.textContent = `${p.forma}${parcTexto}: ${formatarCentavos(p.valor_centavos)}`;

    const btnX = document.createElement("button");
    btnX.type = "button";
    btnX.className = "botao danger pg-botao-remover";
    btnX.textContent = "Remover";
    btnX.title = "Remover este pagamento";
    btnX.onclick = () => removerPagamentoLinha(idx);

    item.append(desc, btnX);
    lista.appendChild(item);
  });

  const restante = calcularSaldoRestante();
  if (restante === 0) {
    lblFalta.textContent = "Quitado";
    lblFalta.style.color = "var(--verde)";
  } else {
    lblFalta.textContent = formatarCentavos(restante);
    lblFalta.style.color = "var(--vermelho)";
  }
}

function removerPagamentoLinha(idx) {
  const p = VENDA.pagamentos[idx];
  if (!p) return;
  if (
    !confirm(
      `Remover o pagamento de ${formatarCentavos(p.valor_centavos)} em ${p.forma}?`,
    )
  ) {
    return;
  }
  VENDA.pagamentos.splice(idx, 1);
  atualizarFitaLateral();
  if (
    document.getElementById("modal-pagamento-final").style.display === "flex"
  ) {
    abrirTelaPagamento(formaAtualPagamento);
  }
  avisar("Pagamento removido. Saldo restante atualizado.");
}

// F3: Selecionar Método de Pagamento — grade de ícones SVG
function selecionarFormaPre(forma) {
  forma = forma.toUpperCase();
  formaAtualPagamento = forma;
  formaPreSelecionada = forma;
  const lbl = document.getElementById("lbl-metodo-pre");
  const info = document.getElementById("info-metodo-pre");
  if (lbl) lbl.textContent = forma;
  if (info) info.style.display = "block";

  fecharModais();
  if (VENDA.total_centavos > 0) {
    abrirTelaPagamento(forma);
  } else {
    avisar(`Forma de pagamento pré-selecionada: ${forma}`);
    produtoInput.focus();
  }
}

function abrirModalMetodoPre(mensagemCustomizada) {
  const somaPagos = VENDA.pagamentos.reduce(
    (acc, p) => acc + p.valor_centavos,
    0,
  );
  const restante = Math.max(0, VENDA.total_centavos - somaPagos);

  document.getElementById("f3-total-valor").textContent = formatarCentavos(
    VENDA.total_centavos,
  );
  const elFalta = document.getElementById("f3-falta-valor");
  if (restante === 0 && VENDA.total_centavos > 0) {
    elFalta.textContent = "Quitado";
    elFalta.style.color = "var(--verde)";
  } else {
    elFalta.textContent = formatarCentavos(restante);
    elFalta.style.color = "var(--vermelho)";
  }

  const pInstrucao = document.getElementById("f3-instrucao");
  if (mensagemCustomizada) {
    pInstrucao.textContent = mensagemCustomizada;
    pInstrucao.style.color = "var(--azul-600)";
  } else {
    pInstrucao.textContent =
      "Selecione no teclado [F1 a F6] ou clique na forma desejada:";
    pInstrucao.style.color = "var(--preto)";
  }

  abrirModal("modal-metodos-pre");
}

// Monta grade de ícones no modal de seleção F3
montarGradePagamentos(
  document.getElementById("grade-metodos-pre"),
  selecionarFormaPre,
);

document.getElementById("btn-fecha-metodo-pre").onclick = () => {
  fecharModais();
  produtoInput.focus();
};

// F4: Cancelar Item Específico
function abrirModalCancelarItem() {
  if (VENDA.itens.length === 0) {
    avisar("Nenhum item na venda para cancelar.");
    return;
  }
  document.getElementById("canc-item-num").value =
    VENDA.itens.length.toString();
  document.getElementById("canc-item-qtd").value = "1";
  abrirModal("modal-cancelar-item");
  document.getElementById("canc-item-num").focus();
}

document.getElementById("form-cancelar-item").onsubmit = (e) => {
  e.preventDefault();
  const num = parseInt(document.getElementById("canc-item-num").value, 10);
  const qtdCanc =
    parseInt(document.getElementById("canc-item-qtd").value, 10) || 1;

  const idx = num - 1;
  if (idx < 0 || idx >= VENDA.itens.length) {
    avisar("Número do item inválido.");
    return;
  }

  const item = VENDA.itens[idx];
  if (confirm(`Confirma o cancelamento de ${qtdCanc} un de "${item.nome}"?`)) {
    if (qtdCanc >= item.quantidade) {
      VENDA.itens.splice(idx, 1);
    } else {
      item.quantidade -= qtdCanc;
    }
    renderItens();
    fecharModais();
    avisar(`Item #${num} cancelado com sucesso.`);
    produtoInput.focus();
  }
};

// F5: Cancelar Compra Inteira (Autorização)
function abrirModalCancelarCompra() {
  if (VENDA.itens.length === 0) {
    avisar("Nenhuma venda em andamento.");
    return;
  }
  document.getElementById("form-canc-compra-auth").reset();
  abrirModal("modal-cancelar-compra");
  document.getElementById("canc-auth-user").focus();
}

document.getElementById("form-canc-compra-auth").onsubmit = async (e) => {
  e.preventDefault();
  const user = document.getElementById("canc-auth-user").value.trim();
  const senha = document.getElementById("canc-auth-senha").value;
  const sessao_id =
    localStorage.getItem("mikella_sessao_id") ||
    "00000000-0000-4000-8000-0000000000c1";

  avisar("Validando autorização de cancelamento...");
  const { ok, data } = await requisitarJSON(
    "/v1/vendas/autorizar-cancelamento",
    {
      method: "POST",
      body: JSON.stringify({
        username: user,
        senha: senha,
        sessao_id: sessao_id,
      }),
    },
  );

  if (ok && data.autorizado) {
    VENDA.itens = [];
    VENDA.pagamentos = [];
    atualizarFitaLateral();
    renderItens();
    fecharModais();
    avisar("Compra cancelada com sucesso!");
    setTimeout(() => {
      window.location.href = "pos-menu.html";
    }, 800);
  } else {
    avisar(data.erro || "Falha na autorização de cancelamento.");
  }
};

// F1: Tela de Pagamento Direta e Descomplicada
function abrirTelaPagamento(forma) {
  if (VENDA.total_centavos <= 0) {
    avisar("Adicione produtos antes de ir para o pagamento.");
    return;
  }

  forma = (forma || formaAtualPagamento || "DINHEIRO").toUpperCase();
  formaAtualPagamento = forma;

  const somaPagos = VENDA.pagamentos.reduce(
    (acc, p) => acc + p.valor_centavos,
    0,
  );
  const restante = Math.max(0, VENDA.total_centavos - somaPagos);

  document.getElementById("pg-total-devido").textContent = formatarCentavos(
    VENDA.total_centavos,
  );

  const boxJaPago = document.getElementById("box-ja-pago-wrapper");
  if (somaPagos > 0) {
    boxJaPago.style.display = "block";
    document.getElementById("pg-ja-pago-valor").textContent =
      formatarCentavos(somaPagos);
  } else {
    boxJaPago.style.display = "none";
  }

  const elRestante = document.getElementById("pg-restante-valor");
  if (restante === 0) {
    elRestante.textContent = "Quitado";
    elRestante.style.color = "var(--verde)";
  } else {
    elRestante.textContent = formatarCentavos(restante);
    elRestante.style.color = "var(--preto)";
  }

  // Nome e Ícone da Forma
  document.getElementById("pg-forma-nome").textContent = forma;
  document.getElementById("pg-forma-icone").innerHTML =
    obterIconePagamento(forma);

  // Ocultar caixas de prazo
  document.getElementById("pg-prazo-debito").style.display = "none";
  document.getElementById("pg-prazo-credito").style.display = "none";
  document.getElementById("pg-prazo-pix").style.display = "none";
  document.getElementById("pg-prazo-crediario").style.display = "none";
  document.getElementById("pg-prazo-dinheiro").style.display = "none";
  document.getElementById("pg-troco-box").style.display = "none";

  const inputValor = document.getElementById("pg-input-valor");
  const lblValor = document.getElementById("pg-lbl-valor");
  const inputParcelas = document.getElementById("pg-input-parcelas");
  const lblParcelas = document.getElementById("pg-parcelas-preview");

  if (forma === "DINHEIRO") {
    lblValor.textContent = "Valor recebido (R$)";
    document.getElementById("pg-prazo-dinheiro").style.display = "block";
  } else if (forma === "DEBITO") {
    lblValor.textContent = "Valor a pagar (R$)";
    document.getElementById("pg-prazo-debito").style.display = "block";
  } else if (forma === "CREDITO") {
    lblValor.textContent = "Valor a pagar (R$)";
    document.getElementById("pg-prazo-credito").style.display = "block";
    inputParcelas.max = String(configPagamento.max_parcelas_cartao);
    inputParcelas.value = "1";
    lblParcelas.textContent = calcularParcelasTexto(restante, 1);
  } else if (forma === "PIX") {
    lblValor.textContent = "Valor a pagar (R$)";
    document.getElementById("pg-prazo-pix").style.display = "block";
  } else if (forma === "CREDIARIO") {
    lblValor.textContent = "Valor a pagar (R$)";
    document.getElementById("pg-prazo-crediario").style.display = "block";
    const infoCli = document.getElementById("pg-crediario-cliente-info");
    if (clienteSelecionado) {
      infoCli.textContent = `Cliente: ${clienteSelecionado.nome}`;
      infoCli.style.color = "var(--preto)";
    } else {
      infoCli.textContent =
        "Atenção: Venda a crediário exige cliente identificado! Pressione F2.";
      infoCli.style.color = "var(--vermelho, #C81E16)";
    }
  } else {
    lblValor.textContent = "Valor a pagar (R$)";
  }

  // Preenche valor com o saldo que falta pagar
  inputValor.value = formatarCampoMoeda(restante);

  // Renderiza pagamentos já lançados
  const boxFita = document.getElementById("pg-box-pagamentos-fita");
  const listaFita = document.getElementById("pg-lancamentos-lista");
  listaFita.replaceChildren();

  if (VENDA.pagamentos.length > 0) {
    boxFita.style.display = "block";
    VENDA.pagamentos.forEach((p, idx) => {
      const item = document.createElement("div");
      item.style.display = "flex";
      item.style.justifyContent = "space-between";
      item.style.alignItems = "center";
      item.style.background = "var(--branco)";
      item.style.border = "1px solid var(--linha)";
      item.style.padding = "4px 8px";
      item.style.fontSize = "0.85rem";

      const infoP = document.createElement("span");
      const parcTexto =
        p.parcelas && p.parcelas > 1 ? ` (${p.parcelas}x)` : "";
      infoP.textContent = `${p.forma}${parcTexto}: ${formatarCentavos(p.valor_centavos)}`;

      const btnRemover = document.createElement("button");
      btnRemover.type = "button";
      btnRemover.className = "botao danger pg-botao-remover-lista";
      btnRemover.textContent = "Remover pagamento";
      btnRemover.onclick = () => removerPagamentoLinha(idx);

      item.append(infoP, btnRemover);
      listaFita.appendChild(item);
    });
  } else {
    boxFita.style.display = "none";
  }

  const btnConcluir = document.getElementById("btn-concluir-pg");
  btnConcluir.textContent = "Finalizar venda (Enter)";

  abrirModal("modal-pagamento-final");

  setTimeout(() => {
    inputValor.focus();
    inputValor.select();
  }, 40);
}

function acionarPagarVenda() {
  if (VENDA.total_centavos <= 0) {
    avisar("Adicione produtos antes de ir para o pagamento.");
    return;
  }
  if (VENDA.pagamentos.length === 0 && !formaPreSelecionada) {
    abrirModalMetodoPre();
  } else {
    abrirTelaPagamento(
      formaAtualPagamento || formaPreSelecionada || "DINHEIRO",
    );
  }
}

const pgInputValor = document.getElementById("pg-input-valor");
const pgInputParcelas = document.getElementById("pg-input-parcelas");

function atualizarDinamicaPagamento() {
  const somaPagos = VENDA.pagamentos.reduce(
    (acc, p) => acc + p.valor_centavos,
    0,
  );
  const restante = Math.max(0, VENDA.total_centavos - somaPagos);
  const valorDigitado = parsearCentavos(pgInputValor.value);
  const btnConcluir = document.getElementById("btn-concluir-pg");

  // Se Dinheiro e valor digitado for maior que o restante -> exibe troco
  if (formaAtualPagamento === "DINHEIRO") {
    const boxTroco = document.getElementById("pg-troco-box");
    const valTroco = document.getElementById("pg-troco-valor");
    if (valorDigitado > restante && restante > 0) {
      valTroco.textContent = formatarCentavos(valorDigitado - restante);
      boxTroco.style.display = "block";
    } else {
      boxTroco.style.display = "none";
    }
  }

  // Se Crédito, recalcula preview das parcelas (limitado pela loja)
  if (formaAtualPagamento === "CREDITO") {
    let numParc = Math.max(1, parseInt(pgInputParcelas.value, 10) || 1);
    if (numParc > configPagamento.max_parcelas_cartao) {
      numParc = configPagamento.max_parcelas_cartao;
      pgInputParcelas.value = String(numParc);
    }
    const baseCalculo = valorDigitado > 0 ? valorDigitado : restante;
    document.getElementById("pg-parcelas-preview").textContent =
      calcularParcelasTexto(baseCalculo, numParc);
  }

  if (valorDigitado > 0 && valorDigitado < restante) {
    btnConcluir.textContent = "Confirmar pagamento parcial (Enter)";
  } else {
    btnConcluir.textContent = "Finalizar venda (Enter)";
  }
}

pgInputValor.addEventListener("input", atualizarDinamicaPagamento);
pgInputParcelas.addEventListener("input", atualizarDinamicaPagamento);

document.getElementById("btn-pg-mudar-forma").onclick = () => {
  abrirModalMetodoPre();
};

document.getElementById("btn-pg-voltar").onclick = () => {
  fecharModais();
  produtoInput.focus();
};

// Submissão do Formulário de Pagamento
document.getElementById("form-confirmar-pagamento").onsubmit = async (e) => {
  e.preventDefault();
  const valorDigitado = parsearCentavos(pgInputValor.value);
  if (valorDigitado <= 0) {
    avisar("Informe um valor válido maior que zero.");
    return;
  }

  const somaPagos = VENDA.pagamentos.reduce(
    (acc, p) => acc + p.valor_centavos,
    0,
  );
  const restante = Math.max(0, VENDA.total_centavos - somaPagos);

  if (formaAtualPagamento !== "DINHEIRO" && valorDigitado > restante) {
    avisar(
      `O valor passa do que falta pagar. Máximo permitido: ${formatarCentavos(restante)}.`,
    );
    return;
  }

  if (
    formaAtualPagamento === "CREDIARIO" &&
    (!clienteSelecionado || !clienteSelecionado.id)
  ) {
    avisar(
      "Venda a crediário exige cliente cadastrado. Pressione F2 e selecione um cliente da lista.",
    );
    return;
  }

  if (formaAtualPagamento === "DINHEIRO") {
    if (valorDigitado >= restante) {
      VENDA.pagamentos.push({
        forma: "DINHEIRO",
        valor_centavos: restante,
        recebido_centavos: valorDigitado,
        parcelas: 1,
      });
      atualizarFitaLateral();
      await finalizarVendaEfetiva(valorDigitado - restante);
    } else {
      VENDA.pagamentos.push({
        forma: "DINHEIRO",
        valor_centavos: valorDigitado,
        recebido_centavos: valorDigitado,
        parcelas: 1,
      });
      atualizarFitaLateral();
      const novoRestante = restante - valorDigitado;
      fecharModais();
      abrirModalMetodoPre(
        `Falta ${formatarCentavos(novoRestante)}. Escolha a próxima forma de pagamento:`,
      );
      avisar(
        `Registrado ${formatarCentavos(valorDigitado)} em Dinheiro. Falta ${formatarCentavos(novoRestante)}.`,
      );
    }
    return;
  }

  // Outras formas (Débito, Crédito, Pix, Voucher, Crediário)
  const numParc =
    formaAtualPagamento === "CREDITO"
      ? Math.max(1, parseInt(pgInputParcelas.value, 10) || 1)
      : 1;

  if (numParc > configPagamento.max_parcelas_cartao) {
    avisar(
      `Esta loja aceita no máximo ${configPagamento.max_parcelas_cartao} parcelas no cartão.`,
    );
    return;
  }

  if (valorDigitado < restante) {
    VENDA.pagamentos.push({
      forma: formaAtualPagamento,
      valor_centavos: valorDigitado,
      recebido_centavos: valorDigitado,
      parcelas: numParc,
    });
    atualizarFitaLateral();
    const novoRestante = restante - valorDigitado;
    fecharModais();
    abrirModalMetodoPre(
      `Falta ${formatarCentavos(novoRestante)}. Escolha a próxima forma de pagamento:`,
    );
    avisar(
      `Registrado ${formatarCentavos(valorDigitado)} em ${formaAtualPagamento}. Falta ${formatarCentavos(novoRestante)}.`,
    );
  } else {
    VENDA.pagamentos.push({
      forma: formaAtualPagamento,
      valor_centavos: valorDigitado,
      recebido_centavos: valorDigitado,
      parcelas: numParc,
    });
    atualizarFitaLateral();
    await finalizarVendaEfetiva(0);
  }
};

async function finalizarVendaEfetiva(trocoCentavos) {
  const sessao_id = localStorage.getItem("mikella_sessao_id");
  if (!sessao_id) {
    avisar("Caixa fechado! Abra o caixa antes de finalizar a venda.");
    return;
  }
  const venda_id = VENDA.venda_id;

  const payload = {
    venda_id,
    sessao_id,
    dispositivo_id: "00000000-0000-4000-8000-0000000000d1",
    itens: VENDA.itens.map((i) => ({
      produto_id: i.id,
      lote_id: null,
      quantidade: i.quantidade,
    })),
    pagamentos: VENDA.pagamentos.map((p) => ({
      forma: p.forma,
      valor_centavos: p.valor_centavos,
      recebido_centavos: p.recebido_centavos,
      parcelas: p.parcelas || 1,
    })),
    desconto_centavos: VENDA.desconto_centavos,
    cliente_id: clienteSelecionado?.id || null,
    comanda_id: comandaNum || null,
  };

  avisar("Finalizando venda na loja local...");
  const { ok, data } = await requisitarJSON("/v1/vendas", {
    method: "POST",
    body: JSON.stringify(payload),
  });

  if (ok) {
    ultimaVendaIdConcluida = data.venda_id || venda_id;
    // A venda gravada não pode ser reaproveitada por um novo rascunho.
    VENDA.venda_id = crypto.randomUUID();
    fecharModais();

    const trocoCalculado =
      trocoCentavos > 0
        ? trocoCentavos
        : parsearCentavos(data.troco || "0");
    if (trocoCalculado > 0) {
      document.getElementById("concluida-troco-valor").textContent =
        formatarCentavos(trocoCalculado);
      document.getElementById("box-troco-final").style.display = "block";
    } else {
      document.getElementById("box-troco-final").style.display = "none";
    }

    abrirModal("modal-concluida");
  } else {
    avisar(data.erro || "Falha ao gravar venda.");
  }
}

document.getElementById("btn-imprimir-cupom").onclick = async () => {
  if (!ultimaVendaIdConcluida) return;
  avisar("Enviando comprovante para a impressora...");
  await requisitarJSON(`/v1/vendas/${ultimaVendaIdConcluida}/imprimir`, {
    method: "POST",
  });
  avisar("Comprovante enviado!");
  setTimeout(() => {
    window.location.href = "pos-menu.html";
  }, 600);
};

document.getElementById("btn-retornar-pdv").onclick = () => {
  window.location.href = "pos-menu.html";
};

// Controle de Modais
function abrirModal(id) {
  fecharModais();
  document.getElementById(id).style.display = "flex";
}

function fecharModais() {
  document
    .querySelectorAll(".modal")
    .forEach((m) => (m.style.display = "none"));
}
window.fecharModais = fecharModais;

document.querySelectorAll('[data-acao="fechar-modais"]').forEach((botao) => {
  botao.addEventListener("click", fecharModais);
});

function tentarSairDoCaixa() {
  // Regra 1: Carrinho vazio e nada pago -> Sai direto sem confirmação
  if (VENDA.itens.length === 0 && VENDA.pagamentos.length === 0) {
    window.location.href = "pos-menu.html";
    return;
  }

  // Regra 2: Pagamento parcial já lançado -> Não pode sair sem estornar/cancelar
  if (VENDA.pagamentos.length > 0) {
    avisar("Há pagamentos parciais registrados nesta venda. Conclua a venda ou cancele os pagamentos antes de sair.");
    return;
  }

  // Regra 3: Com itens e nada pago -> Modal pergunta se deseja descartar
  const modalConfirmarSair = document.getElementById("modal-confirmar-sair");
  if (modalConfirmarSair) {
    modalConfirmarSair.style.display = "flex";
  } else {
    window.location.href = "pos-menu.html";
  }
}

// Teclas de atalho na tela do caixa (F1-F6, Esc, Enter)
window.addEventListener("keydown", (e) => {
  // O módulo de consulta captura o próprio Esc; enquanto ele estiver aberto,
  // F1-F6 não podem agir por baixo do modal.
  if (consultaPreco.estaAberta()) return;

  const modalConcluidaAberta =
    document.getElementById("modal-concluida").style.display === "flex";
  if (modalConcluidaAberta) {
    if (e.key === "Enter") {
      e.preventDefault();
      document.getElementById("btn-imprimir-cupom").click();
    } else if (e.key === "Escape") {
      e.preventDefault();
      window.location.href = "pos-menu.html";
    }
    return;
  }

  const modalMetodosPreAberto =
    document.getElementById("modal-metodos-pre").style.display === "flex";
  if (modalMetodosPreAberto) {
    if (e.key === "F1") {
      e.preventDefault();
      selecionarFormaPre("DINHEIRO");
      return;
    }
    if (e.key === "F2") {
      e.preventDefault();
      selecionarFormaPre("DEBITO");
      return;
    }
    if (e.key === "F3") {
      e.preventDefault();
      selecionarFormaPre("CREDITO");
      return;
    }
    if (e.key === "F4") {
      e.preventDefault();
      selecionarFormaPre("VOUCHER");
      return;
    }
    if (e.key === "F5") {
      e.preventDefault();
      selecionarFormaPre("PIX");
      return;
    }
    if (e.key === "F6") {
      e.preventDefault();
      selecionarFormaPre("CREDIARIO");
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      fecharModais();
      produtoInput.focus();
      return;
    }
  }

  const modalPagamentoAberto =
    document.getElementById("modal-pagamento-final").style.display === "flex";
  if (modalPagamentoAberto) {
    if (e.key === "F3") {
      e.preventDefault();
      abrirModalMetodoPre();
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      fecharModais();
      produtoInput.focus();
      return;
    }
  }

  const algumModalAberto = Array.from(document.querySelectorAll(".modal")).some(
    (m) => m.style.display === "flex",
  );
  if (e.key === "Escape") {
    e.preventDefault();
    if (algumModalAberto) {
      fecharModais();
      produtoInput.focus();
    } else {
      tentarSairDoCaixa();
    }
    return;
  }

  // Atalhos globais caso não esteja digitando em um input de texto dentro de modal
  if (
    document.activeElement &&
    document.activeElement.tagName === "INPUT" &&
    algumModalAberto
  ) {
    return;
  }

  if (e.key === "F1") {
    e.preventDefault();
    acionarPagarVenda();
  } else if (e.key === "F2") {
    e.preventDefault();
    abrirModalCliente();
  } else if (e.key === "F3") {
    e.preventDefault();
    abrirModalMetodoPre();
  } else if (e.key === "F4") {
    e.preventDefault();
    abrirModalCancelarItem();
  } else if (e.key === "F5") {
    e.preventDefault();
    abrirModalCancelarCompra();
  } else if (e.key === "F6") {
    e.preventDefault();
    consultaPreco.abrir(produtoInput.value);
  }
});

// Cliques diretos
document.getElementById("btn-f1-pagar").onclick = acionarPagarVenda;
document.getElementById("btn-atalho-f1").onclick = acionarPagarVenda;
document.getElementById("btn-atalho-f2").onclick = abrirModalCliente;
document.getElementById("btn-atalho-f3").onclick = abrirModalMetodoPre;
document.getElementById("btn-atalho-f4").onclick = abrirModalCancelarItem;
document.getElementById("btn-atalho-f5").onclick = abrirModalCancelarCompra;
document.getElementById("btn-atalho-f6").onclick = () =>
  consultaPreco.abrir(produtoInput.value);

const btnConfirmarDescarte = document.getElementById("btn-confirmar-descarte-sair");
if (btnConfirmarDescarte) {
  btnConfirmarDescarte.onclick = () => {
    window.location.href = "pos-menu.html";
  };
}

document.getElementById("btn-atalho-sair").onclick = tentarSairDoCaixa;

inicializar();
