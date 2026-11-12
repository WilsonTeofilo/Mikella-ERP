import {
  garantirSessao,
  requisitarJSON,
  requisitarTexto,
} from "../../shared/services/api.js";
import { mostrarToast } from "../../shared/ui/feedback.js";
import {
  formatarCentavos,
  parsearCentavos,
} from "../../shared/ui/formatadores.js";
import { inicializarControlesJanela } from "../../shared/ui/window-controls.js";

inicializarControlesJanela();
garantirSessao();

let PRODUTOS = [];

const avisar = mostrarToast;

async function carregarProdutos() {
  const { ok, data } = await requisitarJSON("/v1/produtos");
  if (!ok) {
    avisar(data.erro || "Falha ao carregar lista de produtos.");
    return;
  }
  PRODUTOS = data.produtos || [];
  renderTabela(PRODUTOS);
}

function renderTabela(lista) {
  const tbody = document.getElementById("tabela-produtos-corpo");
  tbody.replaceChildren();
  if (lista.length === 0) {
    const tr = document.createElement("tr");
    const td = document.createElement("td");
    td.colSpan = 6;
    td.style.textAlign = "center";
    td.style.padding = "var(--espaco-6)";
    td.textContent =
      "Nenhum produto cadastrado. Pressione F1 para cadastrar ou F2 para importar CSV.";
    tr.appendChild(td);
    tbody.appendChild(tr);
    return;
  }

  lista.forEach((p) => {
    const tr = document.createElement("tr");
    const codigo = p.codigo_barras || p.sku || "-";

    const tdNome = document.createElement("td");
    const nomeForte = document.createElement("strong");
    nomeForte.textContent = p.nome;
    tdNome.appendChild(nomeForte);

    const tdCodigo = document.createElement("td");
    tdCodigo.textContent = codigo;

    const tdUnidade = document.createElement("td");
    tdUnidade.textContent = p.unidade || "UN";

    const tdPreco = document.createElement("td");
    tdPreco.className = "direita";
    tdPreco.textContent = formatarCentavos(p.preco_centavos);

    const tdEstoque = document.createElement("td");
    tdEstoque.className = "direita";
    tdEstoque.textContent =
      p.estoque_atual !== undefined ? String(p.estoque_atual) : "-";

    const tdAcoes = document.createElement("td");
    tdAcoes.className = "direita";
    const btnArquivar = document.createElement("button");
    btnArquivar.type = "button";
    btnArquivar.className = "botao botao-acao-tabela";
    btnArquivar.textContent = "Arquivar";
    btnArquivar.addEventListener("click", () => arquivarProduto(p.id));
    tdAcoes.appendChild(btnArquivar);

    tr.append(tdNome, tdCodigo, tdUnidade, tdPreco, tdEstoque, tdAcoes);
    tbody.appendChild(tr);
  });
}

async function arquivarProduto(id) {
  if (!confirm("Tem certeza que deseja arquivar este produto?")) return;
  const { ok, data } = await requisitarJSON(`/v1/produtos/${id}/arquivar`, {
    method: "POST",
  });
  if (ok) {
    avisar("Produto arquivado.");
    carregarProdutos();
  } else {
    avisar(data.erro || "Falha ao arquivar produto.");
  }
}

// Filtro de busca rápido
const inputFiltro = document.getElementById("filtro-busca");
inputFiltro.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    e.preventDefault();
    window.location.href = "main-menu.html";
  }
});
inputFiltro.addEventListener("input", () => {
  const termo = inputFiltro.value.trim().toLowerCase();
  if (!termo) {
    renderTabela(PRODUTOS);
    return;
  }
  const filtrados = PRODUTOS.filter(
    (p) =>
      p.nome.toLowerCase().includes(termo) ||
      (p.codigo_barras && p.codigo_barras.toLowerCase().includes(termo)) ||
      (p.sku && p.sku.toLowerCase().includes(termo)),
  );
  renderTabela(filtrados);
});

// Modal de Novo Produto
const modalProd = document.getElementById("modal-produto");
const btnNovo = document.getElementById("btn-novo");
const btnCancelaModal = document.getElementById("btn-cancelar-modal");
const formProd = document.getElementById("form-produto");
const lblMargem = document.getElementById("prod-margem-preview");

function atualizarMargem() {
  if (!lblMargem) return;
  const custo = parsearCentavos(document.getElementById("prod-custo").value);
  const preco = parsearCentavos(document.getElementById("prod-preco").value);

  if (preco <= 0) {
    lblMargem.textContent = "Margem: --";
    lblMargem.style.color = "var(--azul-600)";
    return;
  }

  const lucro = preco - custo;
  const percMargem = ((lucro / preco) * 100).toFixed(1);
  const lucroFmt = formatarCentavos(lucro);

  if (lucro >= 0) {
    lblMargem.textContent = `Margem: +${percMargem}% (Lucro: ${lucroFmt})`;
    lblMargem.style.color = "var(--verde)";
  } else {
    lblMargem.textContent = `Margem: ${percMargem}% (Prejuízo: ${lucroFmt})`;
    lblMargem.style.color = "var(--vermelho)";
  }
}

function fecharModalProduto() {
  const nome = document.getElementById("prod-nome").value.trim();
  const barras = document.getElementById("prod-barras").value.trim();
  const sku = document.getElementById("prod-sku").value.trim();
  const preco = document.getElementById("prod-preco").value.trim();
  const dirty = Boolean(nome || barras || sku || preco);
  if (dirty && !confirm("Descartar alterações?")) {
    return;
  }
  modalProd.style.display = "none";
  formProd.reset();
  if (lblMargem) lblMargem.textContent = "Margem: --";
  inputFiltro.focus();
}

btnNovo.onclick = () => {
  formProd.reset();
  if (lblMargem) lblMargem.textContent = "Margem: --";
  modalProd.style.display = "flex";
  document.getElementById("prod-barras").focus();
};

btnCancelaModal.onclick = fecharModalProduto;

// Ao bipar código de barras com leitor
const prodBarrasInput = document.getElementById("prod-barras");
prodBarrasInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    const codigo = prodBarrasInput.value.trim().toLowerCase();
    if (codigo) {
      const existe = PRODUTOS.find(
        (p) =>
          (p.codigo_barras && p.codigo_barras.toLowerCase() === codigo) ||
          (p.sku && p.sku.toLowerCase() === codigo),
      );
      if (existe) {
        avisar(`Atenção: Código já existente no produto "${existe.nome}".`);
      }
    }
    document.getElementById("prod-nome").focus();
  }
});

// Máscara monetária nos campos de preço e custo
["prod-preco", "prod-custo"].forEach((id) => {
  const el = document.getElementById(id);
  el.addEventListener("input", () => {
    const v = el.value.replace(/\D/g, "");
    if (v.length > 0) {
      el.value = (parseInt(v, 10) / 100).toFixed(2).replace(".", ",");
    }
    atualizarMargem();
  });
});

formProd.onsubmit = async (e) => {
  e.preventDefault();
  const nome = document.getElementById("prod-nome").value.trim();
  if (!nome) {
    avisar("Nome do produto é obrigatório.");
    return;
  }
  const precoCentavos = parsearCentavos(
    document.getElementById("prod-preco").value,
  );
  if (precoCentavos <= 0) {
    avisar("Preço de venda deve ser maior que zero.");
    return;
  }

  const payload = {
    produto_id: crypto.randomUUID(),
    nome: nome,
    codigo_barras: document.getElementById("prod-barras").value.trim() || null,
    sku: document.getElementById("prod-sku").value.trim() || null,
    categoria_id: null,
    unidade: document.getElementById("prod-unidade").value || "UN",
    preco_centavos: precoCentavos,
    custo_centavos: parsearCentavos(
      document.getElementById("prod-custo").value,
    ),
    quantidade_inicial:
      parseInt(document.getElementById("prod-qtd-inicial").value, 10) || 0,
    estoque_minimo:
      parseInt(document.getElementById("prod-estoque-min").value, 10) || 0,
    perecivel: false,
    alerta_validade_dias: 60,
  };

  const { ok, data } = await requisitarJSON("/v1/produtos", {
    method: "POST",
    body: JSON.stringify(payload),
  });

  if (ok) {
    avisar("Produto cadastrado com sucesso!");
    modalProd.style.display = "none";
    formProd.reset();
    carregarProdutos();
    inputFiltro.focus();
  } else {
    avisar(data.erro || "Falha ao cadastrar produto.");
  }
};

// Importação de CSV
const btnCsv = document.getElementById("btn-csv");
const inputCsv = document.getElementById("input-csv-file");

btnCsv.onclick = () => inputCsv.click();
inputCsv.onchange = async () => {
  const file = inputCsv.files[0];
  if (!file) return;

  const reader = new FileReader();
  reader.onload = async (e) => {
    const csvText = e.target.result;
    avisar("Processando importação...");
    const { ok, data } = await requisitarTexto("/v1/import/csv", {
      body: csvText,
      contentType: "text/csv",
    });
    if (ok) {
      avisar(
        `Importação concluída: ${data.produtos_importados || 0} produtos importados.`,
      );
      carregarProdutos();
    } else {
      avisar(data.erro || "Falha ao importar CSV.");
    }
    inputCsv.value = "";
  };
  reader.readAsText(file);
};

// Voltar
function voltarAoMenu() {
  window.location.href = "main-menu.html";
}
document.getElementById("btn-voltar").addEventListener("click", voltarAoMenu);

// Teclas de atalho
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    e.preventDefault();
    if (modalProd.style.display === "flex") {
      fecharModalProduto();
    } else {
      voltarAoMenu();
    }
    return;
  }

  if (modalProd.style.display === "flex") {
    return;
  }

  if (e.key === "F1") {
    e.preventDefault();
    btnNovo.click();
  } else if (e.key === "F2") {
    e.preventDefault();
    btnCsv.click();
  }
});

carregarProdutos();

const btnTopo = document.getElementById("btn-topo-voltar"); if (btnTopo) btnTopo.addEventListener("click", () => window.location.href = "main-menu.html");
