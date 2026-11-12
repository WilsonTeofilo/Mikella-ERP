import {
  garantirSessao,
  obterUsuario,
  requisitarJSON,
} from "../../shared/services/api.js";
import { mostrarToast } from "../../shared/ui/feedback.js";
import {
  formatarCentavos,
  parsearCentavos,
  formatarCPF,
  formatarTelefone,
  formatarCEP,
  aplicarMascaraCPF,
  aplicarMascaraTelefone,
  aplicarMascaraCEP,
  validarCPF,
  validarTelefone,
} from "../../shared/ui/formatadores.js";
import { inicializarControlesJanela } from "../../shared/ui/window-controls.js";

inicializarControlesJanela();
garantirSessao();

let usuarioAtual = obterUsuario();
let sessaoAtual = null;

const avisar = mostrarToast;

// Inicialização da Tela
async function inicializar() {
  // Exibe operador e papel
  const nomeOperador = usuarioAtual.nome || usuarioAtual.username || "ADMIN";
  document.getElementById("user-display").textContent =
    `OPERADOR: ${nomeOperador}`;
  const papel = (usuarioAtual.papel || "DONO").toUpperCase();
  document.getElementById("user-papel-tag").textContent = papel;

  // Carrega status da sessão do caixa
  const { ok, data } = await requisitarJSON("/v1/caixa/status");
  if (ok && data.aberta) {
    sessaoAtual = data;
    localStorage.setItem("mikella_sessao_id", data.sessao_id);
    document.getElementById("hud-caixa-num").textContent =
      `CAIXA 0${data.numero_caixa || 1}`;
    const tagEstado = document.getElementById("hud-caixa-estado");
    tagEstado.className = "tag-sessao aberto";
    document.getElementById("hud-caixa-estado-texto").textContent = "ABERTO";
  } else {
    sessaoAtual = null;
    const tagEstado = document.getElementById("hud-caixa-estado");
    tagEstado.className = "tag-sessao fechado";
    document.getElementById("hud-caixa-estado-texto").textContent = "FECHADO";
  }
}

// Garante que o caixa esteja aberto antes de vender ou operar
async function garantirCaixaAberto() {
  // 1. Se já sabemos que está aberto na memória local, prossegue
  if (sessaoAtual && sessaoAtual.sessao_id) return true;

  // 2. Consulta o backend para checar se já existe sessão aberta no banco
  const statusRes = await requisitarJSON("/v1/caixa/status");
  if (statusRes.ok && statusRes.data && statusRes.data.aberta) {
    sessaoAtual = statusRes.data;
    localStorage.setItem("mikella_sessao_id", statusRes.data.sessao_id);
    await inicializar();
    return true;
  }

  // 3. Se estiver fechado, abre a sessão 01 com suporte irrestrito
  avisar("Abrindo sessão do Caixa 01...");
  const payload = {
    sessao_id: crypto.randomUUID(),
    numero_caixa: 1,
    dispositivo_id: "00000000-0000-4000-8000-0000000000d1",
    saldo_inicial_centavos: 0,
    fechamento_cego: false,
  };

  const res = await requisitarJSON("/v1/caixa/abrir", {
    method: "POST",
    body: JSON.stringify(payload),
  });

  if (res.ok && res.data && res.data.sessao_id) {
    sessaoAtual = {
      sessao_id: res.data.sessao_id,
      numero_caixa: res.data.numero_caixa || 1,
    };
    localStorage.setItem("mikella_sessao_id", res.data.sessao_id);
    await inicializar();
    avisar("Caixa 01 aberto com sucesso!");
    return true;
  }

  // 4. Se a abertura retornou conflito/idempotência, recupera a sessão ativa
  const retryStatus = await requisitarJSON("/v1/caixa/status");
  if (retryStatus.ok && retryStatus.data && retryStatus.data.aberta) {
    sessaoAtual = retryStatus.data;
    localStorage.setItem("mikella_sessao_id", retryStatus.data.sessao_id);
    await inicializar();
    return true;
  }

  console.error("[garantirCaixaAberto] Erro na resposta da API:", res);
  const msgErro =
    res.data && res.data.erro
      ? res.data.erro
      : "Falha ao abrir sessão de caixa.";
  avisar(msgErro);
  return false;
}

// Ações F1 - F8
async function acaoF1() {
  if (await garantirCaixaAberto()) {
    window.location.href = "pos.html?modo=balcao";
  }
}

async function acaoF2() {
  if (await garantirCaixaAberto()) {
    document.getElementById("input-num-comanda").value = "";
    abrirModal("modal-comanda");
    document.getElementById("input-num-comanda").focus();
  }
}

document.getElementById("form-comanda").onsubmit = (e) => {
  e.preventDefault();
  const num = document.getElementById("input-num-comanda").value.trim();
  if (!num) return;
  window.location.href = `pos.html?modo=comanda&comanda=${encodeURIComponent(num)}`;
};

// F3: Histórico de Vendas
let ultimaVendaIdComprovante = null;

function dataHoje() {
  const agora = new Date();
  const ano = agora.getFullYear();
  const mes = String(agora.getMonth() + 1).padStart(2, "0");
  const dia = String(agora.getDate()).padStart(2, "0");
  return `${ano}-${mes}-${dia}`;
}

function acaoF3() {
  const ini = document.getElementById("hist-data-ini");
  const fim = document.getElementById("hist-data-fim");
  // Padrão do caixa: compras do dia. O operador pode limpar para ver outro
  // período; a busca por COO ignora as datas.
  if (!ini.value) ini.value = dataHoje();
  if (!fim.value) fim.value = dataHoje();

  abrirModal("modal-historico");
  document.getElementById("hist-filtro-cod").focus();
  carregarHistoricoVendas();
}

async function carregarHistoricoVendas() {
  const cod = document.getElementById("hist-filtro-cod").value.trim();
  const ini = document.getElementById("hist-data-ini").value;
  const fim = document.getElementById("hist-data-fim").value;

  let qs = "?limite=100";
  if (cod) qs += `&codigo=${encodeURIComponent(cod)}`;
  if (ini) qs += `&data_inicio=${encodeURIComponent(ini)}`;
  if (fim) qs += `&data_fim=${encodeURIComponent(fim)}`;

  const { ok, data } = await requisitarJSON(`/v1/vendas${qs}`);
  const tbody = document.getElementById("hist-tabela-corpo");
  tbody.replaceChildren();

  if (!ok || !Array.isArray(data) || data.length === 0) {
    const tr = document.createElement("tr");
    const td = document.createElement("td");
    td.colSpan = 5;
    td.style.textAlign = "center";
    td.style.padding = "16px";
    td.textContent = ok
      ? "Nenhuma venda encontrada no período."
      : data.erro || "Falha ao consultar vendas.";
    tr.appendChild(td);
    tbody.appendChild(tr);
    return;
  }

  data.forEach((v) => {
    const tr = document.createElement("tr");

    const tdCod = document.createElement("td");
    const strong = document.createElement("strong");
    strong.textContent = `#${String(v.coo ?? "").padStart(6, "0")}`;
    const br = document.createElement("br");
    const pequeno = document.createElement("span");
    pequeno.style.fontSize = "0.75rem";
    pequeno.style.color = "var(--cinza-500)";
    pequeno.textContent = v.id;
    tdCod.append(strong, br, pequeno);

    const tdData = document.createElement("td");
    // BUG CORRIGIDO: v.created_at é UTC RFC-3339 (ex: "2026-11-07T23:45:12.123Z").
    // Usar .slice(0,16) mostrava a hora UTC, não a hora local — 3h a mais em São Paulo.
    // new Date() + toLocaleString converte corretamente para o fuso do sistema operacional.
    tdData.textContent = v.created_at
      ? new Date(v.created_at).toLocaleString("pt-BR", {
          day: "2-digit",
          month: "2-digit",
          year: "numeric",
          hour: "2-digit",
          minute: "2-digit",
        })
      : "-";

    const tdStatus = document.createElement("td");
    const status = document.createElement("span");
    status.style.fontWeight = "900";
    status.style.color = v.estado === "FINALIZADA" ? "green" : "red";
    status.textContent = v.estado === "CANCELADA" ? "ESTORNADA" : v.estado;
    tdStatus.appendChild(status);

    const tdTotal = document.createElement("td");
    tdTotal.style.textAlign = "right";
    tdTotal.style.fontWeight = "900";
    tdTotal.textContent = v.total;

    const tdAcoes = document.createElement("td");
    tdAcoes.style.textAlign = "center";

    const btnCupom = document.createElement("button");
    btnCupom.type = "button";
    btnCupom.className = "badge-tecla btn-acao-tabela";
    btnCupom.textContent = "Cupom";
    btnCupom.addEventListener("click", () => exibirPreviewCupom(v.id));
    tdAcoes.appendChild(btnCupom);

    if (v.estado === "FINALIZADA") {
      const btnEstorno = document.createElement("button");
      btnEstorno.type = "button";
      btnEstorno.className = "badge-tecla btn-perigo btn-acao-tabela btn-acao-tabela--espacada";
      btnEstorno.textContent = "Estornar";
      btnEstorno.addEventListener("click", () => abrirEstornoVenda(v));
      tdAcoes.appendChild(btnEstorno);
    }

    tr.append(tdCod, tdData, tdStatus, tdTotal, tdAcoes);
    tbody.appendChild(tr);
  });
}

document.getElementById("btn-hist-filtrar").onclick = carregarHistoricoVendas;

document.getElementById("hist-filtro-cod").addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    carregarHistoricoVendas();
  }
});

async function exibirPreviewCupom(vendaId) {
  ultimaVendaIdComprovante = vendaId;
  const { ok, data } = await requisitarJSON(
    `/v1/vendas/${vendaId}/comprovante`,
  );
  if (ok && data.texto_58mm) {
    document.getElementById("hist-preview-cupom").textContent = data.texto_58mm;
    document.getElementById("hist-preview-bloco").style.display = "block";
  } else {
    avisar("Falha ao gerar comprovante da venda.");
  }
}

document.getElementById("btn-reimprimir-cupom").onclick = async () => {
  if (!ultimaVendaIdComprovante) return;
  avisar("Enviando para impressora ESC/POS...");
  const { ok } = await requisitarJSON(
    `/v1/vendas/${ultimaVendaIdComprovante}/imprimir`,
    { method: "POST" },
  );
  if (ok) avisar("Cupom reimpresso com sucesso!");
  else avisar("Falha ao imprimir o cupom; verifique a impressora.");
};

// F3: Estorno de Venda (3 passos)
let vendaEstornoAtual = null;
let ultimoEstorno = null;

function usuarioEhGerencial() {
  const papel = (usuarioAtual.papel || "").toUpperCase();
  return papel === "DONO" || papel === "GERENTE";
}

function abrirEstornoVenda(venda) {
  if (!sessaoAtual || !sessaoAtual.sessao_id) {
    avisar(
      "Sessão fechada — o estorno só pode ser feito com o caixa aberto.",
    );
    return;
  }
  vendaEstornoAtual = venda;

  document.getElementById("est-coo").textContent =
    `#${String(venda.coo ?? "").padStart(6, "0")}`;
  document.getElementById("est-data").textContent = venda.created_at
    ? new Date(venda.created_at).toLocaleString("pt-BR", {
        day: "2-digit",
        month: "2-digit",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      })
    : "—";
  document.getElementById("est-operador").textContent = venda.operador || "—";
  document.getElementById("est-total").textContent = venda.total || "—";

  document.getElementById("est-motivo").value = "";
  document.getElementById("est-motivo-outro").value = "";
  document.getElementById("est-observacao").value = "";
  document.getElementById("est-aprovador-usuario").value = "";
  document.getElementById("est-aprovador-senha").value = "";
  document.getElementById("estorno-cupom").textContent = "";
  document.getElementById("estorno-resultado-texto").textContent = "";

  const gerencial = usuarioEhGerencial();
  document.getElementById("estorno-aprovacao-campos").style.display = gerencial
    ? "none"
    : "block";
  document.getElementById("estorno-autoaprovado").style.display = gerencial
    ? "block"
    : "none";

  document.getElementById("btn-estorno-continuar").disabled = false;
  document.getElementById("btn-estorno-confirmar").disabled = false;

  // O histórico fica aberto por baixo: ao fechar o estorno o operador volta
  // para a lista, sem precisar apertar F3 de novo.
  const modal = document.getElementById("modal-estorno");
  if (modal) modal.style.display = "flex";
  mostrarEtapaEstorno(1);
}

// Fecha apenas o estorno; o histórico (se estiver aberto) reaparece.
function fecharEstorno() {
  const modal = document.getElementById("modal-estorno");
  if (modal) modal.style.display = "none";
  const historico = document.getElementById("modal-historico");
  if (historico && historico.style.display === "flex") {
    document.getElementById("hist-filtro-cod").focus();
  }
}

document.getElementById("btn-estorno-cancelar").onclick = fecharEstorno;
document.getElementById("btn-estorno-concluir").onclick = fecharEstorno;

function mostrarEtapaEstorno(etapa) {
  for (let i = 1; i <= 3; i += 1) {
    const secao = document.getElementById(`estorno-passo-${i}`);
    if (secao) secao.style.display = i === etapa ? "block" : "none";
  }
  document.getElementById("estorno-resultado").style.display = "none";

  if (etapa === 1) {
    document.getElementById("btn-estorno-continuar").focus();
  } else if (etapa === 2) {
    document.getElementById("est-motivo").focus();
  } else if (etapa === 3) {
    if (usuarioEhGerencial()) {
      document.getElementById("btn-estorno-confirmar").focus();
    } else {
      document.getElementById("est-aprovador-usuario").focus();
    }
  }
}

function lerMotivoEstorno() {
  const selecionado = document.getElementById("est-motivo").value;
  const outro = document.getElementById("est-motivo-outro").value.trim();

  if (!selecionado) return null;
  if (selecionado === "Outro") return outro || null;
  return selecionado;
}

function observacaoEstorno() {
  return document.getElementById("est-observacao").value.trim();
}

document.getElementById("btn-estorno-continuar").onclick = () =>
  mostrarEtapaEstorno(2);

document.getElementById("btn-estorno-voltar-1").onclick = () =>
  mostrarEtapaEstorno(1);

document.getElementById("btn-estorno-continuar-2").onclick = () => {
  if (!lerMotivoEstorno()) {
    avisar("Escolha o motivo do estorno.");
    document.getElementById("est-motivo").focus();
    return;
  }
  mostrarEtapaEstorno(3);
};

document.getElementById("btn-estorno-voltar-2").onclick = () =>
  mostrarEtapaEstorno(2);

document.getElementById("btn-estorno-confirmar").onclick = async () => {
  if (!vendaEstornoAtual) return;

  const motivo = lerMotivoEstorno();
  if (!motivo) {
    avisar("Escolha o motivo do estorno.");
    mostrarEtapaEstorno(2);
    return;
  }

  const payload = {
    motivo,
    observacao: observacaoEstorno() || null,
  };

  if (!usuarioEhGerencial()) {
    const username = document
      .getElementById("est-aprovador-usuario")
      .value.trim();
    const senha = document.getElementById("est-aprovador-senha").value;
    if (!username || !senha) {
      avisar("Informe usuário e senha do gerente/dono.");
      document.getElementById("est-aprovador-usuario").focus();
      return;
    }
    payload.aprovador_username = username;
    payload.aprovador_senha = senha;
  }

  const btn = document.getElementById("btn-estorno-confirmar");
  btn.disabled = true;
  const vendaId = vendaEstornoAtual.id;
  const { ok, data } = await requisitarJSON(
    `/v1/vendas/${vendaId}/estornar`,
    { method: "POST", body: payload },
  );
  btn.disabled = false;

  if (!ok) {
    avisar(data.erro || "Falha ao estornar a venda.");
    return;
  }

  // Limpa a senha de autorização assim que a operação termina.
  document.getElementById("est-aprovador-senha").value = "";

  ultimoEstorno = { vendaId, coo: data.coo };
  const texto =
    document.getElementById("estorno-cupom");
  texto.textContent =
    data.texto_58mm ||
    "Estorno registrado. Use [Cupom] para consultar o comprovante.";

  const valor = formatarCentavos(data.valor_estornado_centavos || 0);
  document.getElementById("estorno-resultado-texto").textContent =
    `ESTORNO CONCLUÍDO — ${valor} devolvido (COO #${String(data.coo ?? "").padStart(6, "0")})`;

  for (let i = 1; i <= 3; i += 1) {
    document.getElementById(`estorno-passo-${i}`).style.display = "none";
  }
  document.getElementById("estorno-resultado").style.display = "block";

  const enviado = await requisitarJSON(
    `/v1/vendas/${vendaId}/estorno/imprimir`,
    { method: "POST" },
  );
  if (!enviado.ok) {
    avisar("Estorno registrado. Impressão indisponível — reimprima depois.");
  }

  await carregarHistoricoVendas();
};

document.getElementById("btn-estorno-reimprimir").onclick = async () => {
  if (!ultimoEstorno) return;
  const { ok } = await requisitarJSON(
    `/v1/vendas/${ultimoEstorno.vendaId}/estorno/imprimir`,
    { method: "POST" },
  );
  avisar(
    ok
      ? "Comprovante de estorno reenviado para a impressora."
      : "Falha ao reimprimir o comprovante de estorno.",
  );
};

// F4: Clientes & Crediário
function acaoF4() {
  // Reseta estado para modo busca/lista
  mostrarModoBuscaCliente();
  abrirModal("modal-clientes");
  carregarClientes();
}

function mostrarModoBuscaCliente() {
  document.getElementById("box-busca-cliente").style.display = "flex";
  document.getElementById("box-tabela-clientes").style.display = "block";
  document.getElementById("box-botoes-clientes-rodape").style.display = "flex";
  document.getElementById("box-form-cliente").style.display = "none";
  document.getElementById("modal-cli-titulo").textContent = "Clientes da loja";
}

function mostrarModoNovoCliente(titulo = "Cadastrar novo cliente") {
  // OMITIR BUSCA E TABELA (Exigência do usuário: SUMIR busca ao cadastrar)
  document.getElementById("box-busca-cliente").style.display = "none";
  document.getElementById("box-tabela-clientes").style.display = "none";
  document.getElementById("box-botoes-clientes-rodape").style.display = "none";
  document.getElementById("box-form-cliente").style.display = "block";
  document.getElementById("modal-cli-titulo").textContent = titulo;

  const btnAnon = document.getElementById("btn-anonimizar-cliente");
  const cliId = document.getElementById("cli-id").value;
  if (btnAnon) {
    btnAnon.style.display = cliId ? "inline-flex" : "none";
  }

  document.getElementById("cli-nome").focus();
}

document.getElementById("btn-novo-cliente-form").onclick = () => {
  document.getElementById("form-cliente-salvar").reset();
  document.getElementById("cli-id").value = "";
  document.getElementById("cli-lgpd-marketing").checked = false; // CLI-010 opt-in desmarcado
  mostrarModoNovoCliente("Cadastrar novo cliente");
};

const btnAnonimizarCli = document.getElementById("btn-anonimizar-cliente");
if (btnAnonimizarCli) {
  btnAnonimizarCli.onclick = async () => {
    const cliId = document.getElementById("cli-id").value;
    const nome = document.getElementById("cli-nome").value;
    if (!cliId) return;

    const confirma = window.confirm(
      `Anonimizar o titular "${nome}" nos termos do Art. 18 da LGPD?\n\n` +
      `Os dados pessoais (nome, CPF, telefone, endereço) serão substituídos por registros anônimos no banco de dados.\n` +
      `As notas e registros contábeis/fiscais dos últimos 5 anos serão mantidos preservados nos termos da Lei 9.430/96.\n\n` +
      `Esta ação é irreversível. Deseja prosseguir?`
    );
    if (!confirma) return;

    avisar("Processando anonimização do titular...");
    const { ok, data } = await requisitarJSON(`/v1/clientes/${cliId}`, {
      method: "DELETE",
    });
    if (ok) {
      avisar("Titular anonimizado com sucesso conforme a LGPD.");
      mostrarModoBuscaCliente();
      carregarClientes();
    } else {
      avisar(data.erro || "Falha ao anonimizar cliente.");
    }
  };
}

const btnTesteImp = document.getElementById("btn-teste-impressora");
if (btnTesteImp) {
  btnTesteImp.onclick = async () => {
    avisar("Enviando teste de impressão...");
    const { ok, data } = await requisitarJSON("/v1/sistema/teste-impressora", {
      method: "POST",
    });
    if (ok) {
      avisar("Comprovante de teste enviado para a impressora!");
    } else {
      avisar(data.erro || "Falha ao acionar impressora.");
    }
  };
}

const btnTopo = document.getElementById("btn-voltar-busca-topo"); if (btnTopo) btnTopo.onclick = mostrarModoBuscaCliente;
document.getElementById("btn-cancelar-cliente").onclick =
  mostrarModoBuscaCliente;

async function carregarClientes() {
  const termo = document.getElementById("cli-filtro-termo").value.trim();
  const { ok, data } = await requisitarJSON(
    `/v1/clientes?busca=${encodeURIComponent(termo)}`,
  );
  const tbody = document.getElementById("cli-tabela-corpo");
  tbody.replaceChildren();

  if (!ok || !data.clientes || data.clientes.length === 0) {
    tbody.appendChild(
      linhaVazia(
        5,
        ok
          ? 'Nenhum cliente cadastrado. Clique em "+ Novo Cliente" para cadastrar.'
          : data.erro || "Falha ao consultar clientes.",
      ),
    );
    return;
  }

  data.clientes.forEach((c) => {
    const tr = document.createElement("tr");

    const tdNome = document.createElement("td");
    const nomeForte = document.createElement("strong");
    nomeForte.textContent = c.nome;
    tdNome.appendChild(nomeForte);

    const tdCpf = document.createElement("td");
    tdCpf.textContent = formatarCPF(c.cpf);

    const tdTelefone = document.createElement("td");
    tdTelefone.textContent = formatarTelefone(c.telefone);

    const tdStatus = document.createElement("td");
    tdStatus.style.textAlign = "right";
    const status = document.createElement("span");
    const temDivida = c.saldo_devedor_centavos > 0;
    if (temDivida) {
      status.style.color = c.tem_vencido ? "var(--vermelho)" : "#D97706";
      status.style.fontWeight = "900";
      status.textContent =
        `${c.saldo_devedor_fmt} ${c.tem_vencido ? "(VENCIDO)" : "(A VENCER)"}`;
    } else {
      status.style.color = "green";
      status.style.fontWeight = "700";
      status.textContent = "Sem dívidas";
    }
    tdStatus.appendChild(status);

    const tdAcoes = document.createElement("td");
    tdAcoes.style.textAlign = "center";
    const btnEditar = document.createElement("button");
    btnEditar.type = "button";
    btnEditar.className = "badge-tecla btn-acao-tabela";
    btnEditar.textContent = "Editar";
    // O objeto fica na closure: nada de serializar dado de cliente em atributo.
    btnEditar.addEventListener("click", () => editarCliente(c));
    tdAcoes.appendChild(btnEditar);

    tr.append(tdNome, tdCpf, tdTelefone, tdStatus, tdAcoes);
    tbody.appendChild(tr);
  });
}

function normalizarTexto(txt) {
  return (txt || "")
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .trim();
}

function apenasDigitos(txt) {
  return (txt || "").replace(/\D/g, "");
}

function desmembrarEndereco(endStr) {
  const padrao = {
    cep: "",
    logradouro: "",
    numero: "",
    complemento: "",
    bairro: "",
    cidade: "",
    uf: "",
  };
  if (!endStr || typeof endStr !== "string") return padrao;

  const limpo = endStr.trim();
  // Caso 1: Salvo em formato JSON estruturado
  if (limpo.startsWith("{") && limpo.endsWith("}")) {
    try {
      const obj = JSON.parse(limpo);
      return {
        cep: obj.cep || "",
        logradouro: obj.logradouro || "",
        numero: obj.numero || "",
        complemento: obj.complemento || "",
        bairro: obj.bairro || "",
        cidade: obj.cidade || "",
        uf: (obj.uf || "").toUpperCase(),
      };
    } catch (_e) {
      // continua para o fallback
    }
  }

  // Caso 2: Formato texto legado concatenado (ex: "Rua X, Nº 7, casa, Bairro Centro, São Paulo/SP, CEP 01001-000")
  let textoRestante = limpo;

  // Extrai CEP se presente
  const matchCep = textoRestante.match(/CEP\s+([0-9]{5}-?[0-9]{3})/i);
  if (matchCep) {
    padrao.cep = formatarCEP(matchCep[1]);
    textoRestante = textoRestante.replace(matchCep[0], "").replace(/,\s*,/g, ",");
  }

  // Extrai Cidade/UF se presente (ex: "São Paulo/SP")
  const matchCidadeUf = textoRestante.match(/,\s*([^,]+)\/([A-Za-z]{2})/);
  if (matchCidadeUf) {
    padrao.cidade = matchCidadeUf[1].trim();
    padrao.uf = matchCidadeUf[2].trim().toUpperCase();
    textoRestante = textoRestante.replace(matchCidadeUf[0], "");
  }

  // Extrai Bairro se presente (ex: "Bairro Centro")
  const matchBairro = textoRestante.match(/,\s*Bairro\s+([^,]+)/i);
  if (matchBairro) {
    padrao.bairro = matchBairro[1].trim();
    textoRestante = textoRestante.replace(matchBairro[0], "");
  }

  // Extrai Número se presente (ex: "Nº 123" ou "Nº S/N")
  const matchNum = textoRestante.match(/,\s*N[ºo°]\s*([^,]+)/i);
  if (matchNum) {
    padrao.numero = matchNum[1].trim();
    textoRestante = textoRestante.replace(matchNum[0], "");
  }

  // Divide as partes restantes por vírgula
  const partes = textoRestante
    .split(",")
    .map((p) => p.trim())
    .filter((p) => p.length > 0);

  if (partes.length > 0) {
    padrao.logradouro = partes[0];
  }
  if (partes.length > 1) {
    padrao.complemento = partes.slice(1).join(", ");
  }

  return padrao;
}

function editarCliente(cli) {
  document.getElementById("cli-id").value = cli.id;
  document.getElementById("cli-nome").value = cli.nome;
  document.getElementById("cli-cpf").value = cli.cpf ? formatarCPF(cli.cpf) : "";
  document.getElementById("cli-tel").value = cli.telefone ? formatarTelefone(cli.telefone) : "";
  document.getElementById("cli-email").value = cli.email || "";

  const end = desmembrarEndereco(cli.endereco);
  document.getElementById("cli-cep").value = end.cep ? formatarCEP(end.cep) : "";
  document.getElementById("cli-logradouro").value = end.logradouro || "";
  document.getElementById("cli-numero").value = end.numero || "";
  document.getElementById("cli-complemento").value = end.complemento || "";
  document.getElementById("cli-bairro").value = end.bairro || "";
  document.getElementById("cli-cidade").value = end.cidade || "";
  document.getElementById("cli-uf").value = end.uf || "";

  mostrarModoNovoCliente(`Editar cliente: ${cli.nome}`);
}

// Cria uma linha de tabela com uma única célula de aviso.
function linhaVazia(colunas, mensagem) {
  const tr = document.createElement("tr");
  const td = document.createElement("td");
  td.colSpan = colunas;
  td.style.textAlign = "center";
  td.style.padding = "16px";
  td.textContent = mensagem;
  tr.appendChild(td);
  return tr;
}

document
  .getElementById("cli-filtro-termo")
  .addEventListener("input", carregarClientes);

document.getElementById("btn-sn-num").onclick = () => {
  const numInput = document.getElementById("cli-numero");
  numInput.value = "S/N";
  document.getElementById("cli-complemento").focus();
};

async function consultarViaCEP(cepParam) {
  const valorOriginal = typeof cepParam === "string" ? cepParam : document.getElementById("cli-cep").value;
  const cepLimpo = apenasDigitos(valorOriginal);
  if (cepLimpo.length !== 8) {
    if (!cepParam) {
      avisar("Digite um CEP válido com 8 dígitos.");
    }
    return;
  }
  avisar("Consultando CEP...");

  // 1. Tenta pelo backend local (bypassa CSP do Tauri com garantia)
  try {
    const { ok, data } = await requisitarJSON(`/v1/sistema/cep/${cepLimpo}`);
    if (ok && data) {
      if (data.logradouro) document.getElementById("cli-logradouro").value = data.logradouro;
      if (data.bairro) document.getElementById("cli-bairro").value = data.bairro;
      if (data.cidade) document.getElementById("cli-cidade").value = data.cidade;
      if (data.uf) document.getElementById("cli-uf").value = data.uf.toUpperCase();
      document.getElementById("cli-numero").value = "";
      document.getElementById("cli-complemento").value = "";
      document.getElementById("cli-numero").focus();
      avisar("Endereço preenchido pelo CEP! Preencha o número.");
      return;
    }
  } catch (_e) {
    // continua para o fallback
  }

  // 2. Fallback direto via fetch (para modo browser / dev)
  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 4000);
    const resp = await fetch(`https://viacep.com.br/ws/${cepLimpo}/json/`, {
      signal: controller.signal,
    });
    clearTimeout(timeout);
    if (!resp.ok) throw new Error("Falha no serviço ViaCEP");
    const dados = await resp.json();
    if (dados.erro) {
      avisar("CEP não encontrado.");
      return;
    }
    if (dados.logradouro) document.getElementById("cli-logradouro").value = dados.logradouro;
    if (dados.bairro) document.getElementById("cli-bairro").value = dados.bairro;
    if (dados.localidade) document.getElementById("cli-cidade").value = dados.localidade;
    if (dados.uf) document.getElementById("cli-uf").value = dados.uf.toUpperCase();
    document.getElementById("cli-numero").value = "";
    document.getElementById("cli-complemento").value = "";
    document.getElementById("cli-numero").focus();
    avisar("Endereço preenchido pelo CEP! Preencha o número.");
  } catch (_e) {
    avisar("Não foi possível consultar o CEP online. Preencha manualmente.");
  }
}

// Inicialização das máscaras nos inputs de cliente
aplicarMascaraCPF(document.getElementById("cli-cpf"));
aplicarMascaraTelefone(document.getElementById("cli-tel"));
aplicarMascaraCEP(document.getElementById("cli-cep"), (cep8) => {
  consultarViaCEP(cep8);
});

document.getElementById("cli-cep").addEventListener("blur", () => {
  const v = apenasDigitos(document.getElementById("cli-cep").value);
  if (v.length === 8 && !document.getElementById("cli-logradouro").value.trim()) {
    consultarViaCEP(v);
  }
});

document.getElementById("btn-buscar-cep").onclick = () => consultarViaCEP();
document.getElementById("cli-cep").addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    consultarViaCEP();
  }
});

document.getElementById("form-cliente-salvar").onsubmit = async (e) => {
  e.preventDefault();

  const nome = document.getElementById("cli-nome").value.trim().replace(/\s+/g, " ");
  if (!nome || nome.length < 3) {
    avisar("Nome do cliente deve conter no mínimo 3 caracteres.");
    document.getElementById("cli-nome").focus();
    return;
  }

  const cpfBruto = document.getElementById("cli-cpf").value.trim();
  const cpfDigitos = apenasDigitos(cpfBruto);
  if (!cpfDigitos) {
    avisar("CPF é obrigatório para cadastrar o cliente.");
    document.getElementById("cli-cpf").focus();
    return;
  }
  if (!validarCPF(cpfDigitos)) {
    avisar("CPF inválido. Confira os 11 dígitos e dígitos verificadores.");
    document.getElementById("cli-cpf").focus();
    return;
  }

  const telBruto = document.getElementById("cli-tel").value.trim();
  const telDigitos = apenasDigitos(telBruto);
  if (telDigitos && !validarTelefone(telDigitos)) {
    avisar("Telefone inválido. Informe DDD válido (11 a 99) e 8 ou 9 dígitos.");
    document.getElementById("cli-tel").focus();
    return;
  }

  const logradouro = document.getElementById("cli-logradouro").value.trim();
  const numero = document.getElementById("cli-numero").value.trim();
  const complemento = document.getElementById("cli-complemento").value.trim();
  const bairro = document.getElementById("cli-bairro").value.trim();
  const cidade = document.getElementById("cli-cidade").value.trim();
  const uf = document.getElementById("cli-uf").value.trim().toUpperCase();
  const cep = document.getElementById("cli-cep").value.trim();

  // Validação condicional estrita de endereço (CLI-002, AUD-02)
  const temAlgumEndereco = Boolean(
    logradouro || numero || complemento || bairro || cidade || uf || cep,
  );
  if (temAlgumEndereco) {
    if (!logradouro) {
      avisar("Logradouro/rua é obrigatório quando há endereço informado.");
      document.getElementById("cli-logradouro").focus();
      return;
    }
    if (/^[0-9\s.,\-_/]+$/.test(logradouro)) {
      avisar("O logradouro/rua deve conter o nome da rua, não apenas números.");
      document.getElementById("cli-logradouro").focus();
      return;
    }
    if (!numero) {
      avisar("Número é obrigatório quando há endereço (use S/N se não houver).");
      document.getElementById("cli-numero").focus();
      return;
    }
    if (!bairro) {
      avisar("Bairro é obrigatório quando há endereço informado.");
      document.getElementById("cli-bairro").focus();
      return;
    }
    if (/^[0-9\s.,\-_/]+$/.test(bairro)) {
      avisar("O bairro deve conter o nome do bairro, não apenas números.");
      document.getElementById("cli-bairro").focus();
      return;
    }
    if (!cidade) {
      avisar("Cidade é obrigatória quando há endereço informado.");
      document.getElementById("cli-cidade").focus();
      return;
    }
    if (!uf) {
      avisar("UF é obrigatória quando há endereço informado. Selecione o estado.");
      document.getElementById("cli-uf").focus();
      return;
    }
  }

  let enderecoFinal = null;
  if (temAlgumEndereco) {
    enderecoFinal = JSON.stringify({
      cep: cep ? formatarCEP(cep) : "",
      logradouro: logradouro,
      numero: numero,
      complemento: complemento,
      bairro: bairro,
      cidade: cidade,
      uf: uf,
    });
  }

  const payload = {
    id: document.getElementById("cli-id").value || null,
    nome: nome,
    cpf: cpfDigitos,
    telefone: telDigitos || null,
    email: document.getElementById("cli-email").value.trim() || null,
    endereco: enderecoFinal,
  };

  const btnSalvar = document.getElementById("btn-salvar-cli-form");
  if (btnSalvar) btnSalvar.disabled = true;

  try {
    const { ok, data } = await requisitarJSON("/v1/clientes", {
      method: "POST",
      body: JSON.stringify(payload),
    });

    if (ok) {
      avisar("Cliente salvo com sucesso!");
      mostrarModoBuscaCliente();
      carregarClientes();
    } else {
      avisar(data.erro || "Falha ao salvar cliente.");
    }
  } finally {
    if (btnSalvar) btnSalvar.disabled = false;
  }
};

// F5: Sangria de Caixa
function acaoF5() {
  if (!sessaoAtual || !sessaoAtual.sessao_id) {
    avisar("Abra o caixa antes de realizar sangria.");
    return;
  }
  document.getElementById("form-sangria").reset();
  const gerencial = usuarioEhGerencial();
  document.getElementById("sangria-aprovacao-campos").style.display = gerencial
    ? "none"
    : "block";
  abrirModal("modal-sangria");
  document.getElementById("sangria-valor").focus();
}

document.getElementById("sangria-valor").addEventListener("input", (e) => {
  const c = parsearCentavos(e.target.value);
  if (c > 0) e.target.value = (c / 100).toFixed(2).replace(".", ",");
});

document.getElementById("form-sangria").onsubmit = async (e) => {
  e.preventDefault();
  const centavos = parsearCentavos(
    document.getElementById("sangria-valor").value,
  );
  if (centavos <= 0) {
    avisar("Digite o valor da sangria.");
    return;
  }

  const payload = {
    sessao_id: sessaoAtual.sessao_id,
    valor_centavos: centavos,
    motivo: document.getElementById("sangria-motivo").value.trim(),
  };

  if (!usuarioEhGerencial()) {
    const username = document
      .getElementById("sangria-aprovador-usuario")
      .value.trim();
    const senha = document.getElementById("sangria-aprovador-senha").value;
    if (!username || !senha) {
      avisar("Informe usuário e senha do gerente/dono para autorizar a sangria.");
      document.getElementById("sangria-aprovador-usuario").focus();
      return;
    }
    payload.aprovador_username = username;
    payload.aprovador_senha = senha;
  }

  const res = await requisitarJSON("/v1/caixa/sangria", {
    method: "POST",
    body: JSON.stringify(payload),
  });

  document.getElementById("sangria-aprovador-senha").value = "";

  if (res.ok) {
    avisar(`Sangria realizada com sucesso: ${formatarCentavos(centavos)}`);
    fecharModais();
  } else {
    avisar(res.data.erro || "Falha ao registrar sangria.");
  }
};

// F6: Pagar Dívidas (Crediário)
let DEVEDORES_CACHE = [];
function acaoF6() {
  abrirModal("modal-devedores");
  const filtroInput = document.getElementById("dev-filtro");
  if (filtroInput) {
    filtroInput.value = "";
    setTimeout(() => filtroInput.focus(), 60);
  }
  carregarDevedores();
}

async function carregarDevedores() {
  const { ok, data } = await requisitarJSON("/v1/crediario/devedores");
  const tbody = document.getElementById("dev-tabela-corpo");
  tbody.replaceChildren();

  if (!ok || !data.devedores || data.devedores.length === 0) {
    tbody.appendChild(
      linhaVazia(
        5,
        ok
          ? "Nenhum cliente com débito. Tudo em dia."
          : data.erro || "Falha ao consultar o crediário.",
      ),
    );
    DEVEDORES_CACHE = [];
    return;
  }

  DEVEDORES_CACHE = data.devedores;
  renderDevedoresTabela(DEVEDORES_CACHE);
}

function abrirRecebimentoDevedor(item) {
  document.getElementById("dev-liq-conta-id").value = item.parcela_id;
  document.getElementById("dev-liq-parc-num").value = item.parcela_numero;
  document.getElementById("dev-liq-valor").value = item.valor_fmt;
  document.getElementById("dev-liq-titulo").textContent =
    `Receber Dívida de: ${item.cliente_nome}`;
  document.getElementById("dev-submodal-pagar").style.display = "block";
}

function renderDevedoresTabela(lista, termoBusca = "") {
  const tbody = document.getElementById("dev-tabela-corpo");
  tbody.replaceChildren();

  if (lista.length === 0) {
    const msg = termoBusca
      ? `Nenhum devedor encontrado para "${termoBusca}".`
      : "Nenhum cliente com débito. Tudo em dia.";
    tbody.appendChild(linhaVazia(5, msg));
    return;
  }

  lista.forEach((d) => {
    const tr = document.createElement("tr");

    const tdCliente = document.createElement("td");
    const nomeForte = document.createElement("strong");
    nomeForte.textContent = d.cliente_nome;
    const br = document.createElement("br");
    const documento = document.createElement("span");
    documento.style.fontSize = "0.75rem";
    documento.style.color = "var(--cinza-500)";
    documento.textContent = d.cliente_cpf || d.cliente_telefone || "-";
    tdCliente.append(nomeForte, br, documento);

    const tdDescricao = document.createElement("td");
    tdDescricao.textContent = `${d.descricao} (Parc. ${d.parcela_numero})`;

    const tdVencimento = document.createElement("td");
    tdVencimento.textContent = d.vencimento;
    if (d.vencido) {
      const atraso = document.createElement("span");
      atraso.style.color = "red";
      atraso.style.fontWeight = "900";
      atraso.textContent = ` (${d.dias_atraso}d atrás)`;
      tdVencimento.appendChild(atraso);
    }

    const tdValor = document.createElement("td");
    tdValor.style.textAlign = "right";
    tdValor.style.fontWeight = "900";
    tdValor.style.color = "var(--vermelho)";
    tdValor.textContent = d.valor_fmt;

    const tdAcoes = document.createElement("td");
    tdAcoes.style.textAlign = "center";
    const btnReceber = document.createElement("button");
    btnReceber.type = "button";
    btnReceber.className = "badge-tecla btn-acao-tabela btn-acao-tabela--lg";
    btnReceber.textContent = "Receber";
    btnReceber.addEventListener("click", () => abrirRecebimentoDevedor(d));
    tdAcoes.appendChild(btnReceber);

    tr.append(tdCliente, tdDescricao, tdVencimento, tdValor, tdAcoes);
    tbody.appendChild(tr);
  });
}

document.getElementById("dev-filtro").addEventListener("input", (e) => {
  const tOriginal = e.target.value.trim();
  const tNorm = normalizarTexto(tOriginal);
  const tDigitos = apenasDigitos(tOriginal);

  if (!tNorm && !tDigitos) {
    renderDevedoresTabela(DEVEDORES_CACHE);
    return;
  }

  const filtrados = DEVEDORES_CACHE.filter((d) => {
    const nomeNorm = normalizarTexto(d.cliente_nome);
    const cpfDigitos = apenasDigitos(d.cliente_cpf);
    const telDigitos = apenasDigitos(d.cliente_telefone);

    const bateNome = tNorm.length >= 2 && nomeNorm.includes(tNorm);
    const bateNomeInicio = tNorm.length > 0 && nomeNorm.startsWith(tNorm);
    const bateCpf = tDigitos.length >= 2 && cpfDigitos.includes(tDigitos);
    const bateTel = tDigitos.length >= 2 && telDigitos.includes(tDigitos);

    return bateNome || bateNomeInicio || bateCpf || bateTel;
  });

  renderDevedoresTabela(filtrados, tOriginal);
});

document.getElementById("form-dev-liquidar").onsubmit = async (e) => {
  e.preventDefault();
  const payload = {
    parcela_id: document.getElementById("dev-liq-conta-id").value,
    forma_pagamento: document.getElementById("dev-liq-forma").value,
    sessao_id: sessaoAtual ? sessaoAtual.sessao_id : null,
  };

  const { ok, data } = await requisitarJSON("/v1/crediario/liquidar", {
    method: "POST",
    body: JSON.stringify(payload),
  });

  if (ok) {
    avisar("Dívida quitada e recebida no caixa com sucesso!");
    document.getElementById("dev-submodal-pagar").style.display = "none";
    carregarDevedores();
  } else {
    avisar(data.erro || "Falha ao liquidar dívida.");
  }
};

// F7: Atualizar Caixa / Sync
async function acaoF7() {
  avisar("Sincronizando caixa e banco local...");
  await inicializar();
  avisar("Caixa sincronizado com sucesso!");
}

// F8: Fechar Caixa
function acaoF8() {
  if (!sessaoAtual || !sessaoAtual.sessao_id) {
    avisar("O caixa já está fechado.");
    return;
  }
  document.getElementById("fech-etapa-1").style.display = "block";
  document.getElementById("fech-etapa-2").style.display = "none";
  document.getElementById("fech-etapa-3").style.display = "none";
  abrirModal("modal-fechar-fluxo");
}

document.getElementById("btn-fech-sim-sangria").onclick = () => {
  document.getElementById("fech-etapa-1").style.display = "none";
  document.getElementById("fech-etapa-2").style.display = "block";
  document.getElementById("fech-user-id").value =
    `${usuarioAtual.nome || usuarioAtual.username} (${usuarioAtual.id || "ID-ATUAL"})`;
  document.getElementById("fech-contado").value = "";
  document.getElementById("fech-obs").value = "";
  document.getElementById("fech-contado").focus();
};

document.getElementById("btn-fech-nao-sangria").onclick = () => {
  fecharModais();
  acaoF5(); // Redireciona para sangria prévia
};

document.getElementById("fech-contado").addEventListener("input", (e) => {
  const c = parsearCentavos(e.target.value);
  if (c > 0) e.target.value = (c / 100).toFixed(2).replace(".", ",");
});

document.getElementById("form-fechamento-exec").onsubmit = async (e) => {
  e.preventDefault();
  const contadoCentavos = parsearCentavos(
    document.getElementById("fech-contado").value,
  );
  const obs = document.getElementById("fech-obs").value.trim();

  const payload = {
    sessao_id: sessaoAtual.sessao_id,
    saldo_contado_centavos: contadoCentavos,
    observacao: obs || null,
  };

  avisar("Gravando fechamento com integridade BLAKE3...");
  const res = await requisitarJSON("/v1/caixa/fechar", {
    method: "POST",
    body: JSON.stringify(payload),
  });

  if (res.ok) {
    // Busca comprovante térmico do fechamento
    const resComp = await requisitarJSON(
      `/v1/caixa/fechamento/${sessaoAtual.sessao_id}/comprovante`,
    );
    const cupomTexto =
      resComp.ok && resComp.data.texto_58mm
        ? resComp.data.texto_58mm
        : "Comprovante gerado com sucesso.";

    document.getElementById("fech-cupom-texto").textContent = cupomTexto;
    document.getElementById("fech-etapa-2").style.display = "none";
    document.getElementById("fech-etapa-3").style.display = "block";

    document.getElementById("btn-imprimir-fechamento").onclick = async () => {
      await requisitarJSON(
        `/v1/caixa/fechamento/${sessaoAtual.sessao_id}/imprimir`,
        { method: "POST" },
      );
      avisar("Bobina enviada para impressão!");
    };

    // Reset estado
    sessaoAtual = null;
    localStorage.removeItem("mikella_sessao_id");
    inicializar();
  } else {
    avisar(res.data.erro || "Falha ao fechar caixa.");
  }
};

// Gerenciador de Modais
function abrirModal(id) {
  fecharModais();
  const el = document.getElementById(id);
  if (el) el.style.display = "flex";
}

function fecharModais() {
  document
    .querySelectorAll(".modal-overlay")
    .forEach((m) => (m.style.display = "none"));
}
window.fecharModais = fecharModais;

const acoesDeInterface = {
  "fechar-modais": fecharModais,
  "fechar-cupom": () => {
    document.getElementById("hist-preview-bloco").style.display = "none";
  },
  "fechar-submodal-devedor": () => {
    document.getElementById("dev-submodal-pagar").style.display = "none";
  },
  "ir-menu-principal": () => {
    window.location.href = "main-menu.html";
  },
};

document.querySelectorAll("[data-acao]").forEach((botao) => {
  const acao = acoesDeInterface[botao.dataset.acao];
  if (acao) botao.addEventListener("click", acao);
  else
    console.warn(
      "[mikella] ação de interface desconhecida:",
      botao.dataset.acao,
    );
});

// Teclas de Atalho [F1 a F8] e [ESC]
window.addEventListener("keydown", (e) => {
  if (document.querySelector(".modal-overlay[style*='display: flex']")) {
    if (e.key === "Escape") {
      // O estorno fica empilhado sobre o histórico: Esc fecha só o topo.
      const estorno = document.getElementById("modal-estorno");
      if (estorno && estorno.style.display === "flex") {
        fecharEstorno();
      } else {
        fecharModais();
      }
      e.preventDefault();
    }
    return;
  }

  switch (e.key) {
    case "F1":
      e.preventDefault();
      acaoF1();
      break;
    case "F2":
      e.preventDefault();
      acaoF2();
      break;
    case "F3":
      e.preventDefault();
      acaoF3();
      break;
    case "F4":
      e.preventDefault();
      acaoF4();
      break;
    case "F5":
      e.preventDefault();
      acaoF5();
      break;
    case "F6":
      e.preventDefault();
      acaoF6();
      break;
    case "F7":
      e.preventDefault();
      acaoF7();
      break;
    case "F8":
      e.preventDefault();
      acaoF8();
      break;
    case "Escape":
      e.preventDefault();
      window.location.href = "main-menu.html";
      break;
  }
});

// Cliques nos Cards
document.getElementById("card-f1").onclick = acaoF1;
document.getElementById("card-f2").onclick = acaoF2;
document.getElementById("card-f3").onclick = acaoF3;
document.getElementById("card-f4").onclick = acaoF4;
document.getElementById("card-f5").onclick = acaoF5;
document.getElementById("card-f6").onclick = acaoF6;
document.getElementById("card-f7").onclick = acaoF7;
document.getElementById("card-f8").onclick = acaoF8;
document.getElementById("btn-esc-voltar").onclick = () =>
  (window.location.href = "main-menu.html");

// Boot
inicializar();
