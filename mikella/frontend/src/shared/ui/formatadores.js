/**
 * Formatação e leitura de valores monetários, documentos, telefones e CEP.
 * O domínio do Mikella trafega dinheiro sempre em inteiros (centavos)
 * e documentos/telefones sempre em dígitos numéricos;
 * a conversão para texto formatado acontece apenas na apresentação.
 */

const formatadorMoeda = new Intl.NumberFormat("pt-BR", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

export function formatarCentavos(centavos) {
  const valor = Number.isFinite(centavos) ? centavos : 0;
  const sinal = valor < 0 ? "-" : "";
  return `${sinal}R$ ${formatadorMoeda.format(Math.abs(valor) / 100)}`;
}

export function parsearCentavos(texto) {
  const digitos = String(texto ?? "").replace(/\D/g, "");
  return parseInt(digitos, 10) || 0;
}

/**
 * Formata CPF (11 dígitos) no padrão 000.000.000-00
 * ou CNPJ (14 dígitos) no padrão 00.000.000/0000-00.
 */
export function formatarCPF(doc) {
  if (!doc) return "-";
  const d = String(doc).replace(/\D/g, "");
  if (d.length === 11) {
    return d.replace(/(\d{3})(\d{3})(\d{3})(\d{2})/, "$1.$2.$3-$4");
  }
  if (d.length === 14) {
    return d.replace(/(\d{2})(\d{3})(\d{3})(\d{4})(\d{2})/, "$1.$2.$3/$4-$5");
  }
  return doc;
}

/**
 * Formata telefone brasileiro:
 * 11 dígitos (celular): (00) 00000-0000
 * 10 dígitos (fixo): (00) 0000-0000
 */
export function formatarTelefone(tel) {
  if (!tel) return "-";
  const d = String(tel).replace(/\D/g, "");
  if (d.length === 11) {
    return d.replace(/(\d{2})(\d{5})(\d{4})/, "($1) $2-$3");
  }
  if (d.length === 10) {
    return d.replace(/(\d{2})(\d{4})(\d{4})/, "($1) $2-$3");
  }
  return tel;
}

/**
 * Formata CEP (8 dígitos) no padrão 00000-000.
 */
export function formatarCEP(cep) {
  if (!cep) return "";
  const d = String(cep).replace(/\D/g, "");
  if (d.length === 8) {
    return d.replace(/(\d{5})(\d{3})/, "$1-$2");
  }
  return cep;
}

/**
 * Aplica máscara dinâmica de CPF ao input enquanto o usuário digita.
 * Limita a 11 dígitos (14 caracteres formatados).
 */
export function aplicarMascaraCPF(input) {
  if (!input) return;
  input.maxLength = 14;
  input.addEventListener("input", () => {
    let v = input.value.replace(/\D/g, "").slice(0, 11);
    if (v.length > 9) {
      v = v.replace(/(\d{3})(\d{3})(\d{3})(\d{1,2})/, "$1.$2.$3-$4");
    } else if (v.length > 6) {
      v = v.replace(/(\d{3})(\d{3})(\d{1,3})/, "$1.$2.$3");
    } else if (v.length > 3) {
      v = v.replace(/(\d{3})(\d{1,3})/, "$1.$2");
    }
    input.value = v;
  });
}

/**
 * Aplica máscara dinâmica de Telefone ao input enquanto o usuário digita.
 * Trata tanto fixo (10 dígitos) quanto celular (11 dígitos).
 * Limita a 11 dígitos (15 caracteres formatados).
 */
export function aplicarMascaraTelefone(input) {
  if (!input) return;
  input.maxLength = 15;
  input.addEventListener("input", () => {
    let v = input.value.replace(/\D/g, "").slice(0, 11);
    if (v.length > 10) {
      v = v.replace(/(\d{2})(\d{5})(\d{4})/, "($1) $2-$3");
    } else if (v.length > 6) {
      v = v.replace(/(\d{2})(\d{4})(\d{1,4})/, "($1) $2-$3");
    } else if (v.length > 2) {
      v = v.replace(/(\d{2})(\d{1,5})/, "($1) $2");
    } else if (v.length > 0) {
      v = `(${v}`;
    }
    input.value = v;
  });
}

/**
 * Aplica máscara dinâmica de CEP ao input enquanto o usuário digita.
 * Limita a 8 dígitos (9 caracteres formatados: 00000-000).
 */
export function aplicarMascaraCEP(input, onCompleto) {
  if (!input) return;
  input.maxLength = 9;
  input.addEventListener("input", () => {
    let v = input.value.replace(/\D/g, "").slice(0, 8);
    const completo = v.length === 8;
    if (v.length > 5) {
      v = v.replace(/(\d{5})(\d{1,3})/, "$1-$2");
    }
    input.value = v;
    if (completo && typeof onCompleto === "function") {
      onCompleto(v.replace(/\D/g, ""));
    }
  });
}

/**
 * Valida CPF brasileiro com verificação de dígitos verificadores (módulo 11)
 * e rejeição estrita de sequências repetidas (FRM-024, CLI-001).
 */
export function validarCPF(cpf) {
  if (!cpf) return false;
  const d = String(cpf).replace(/\D/g, "");
  if (d.length !== 11) return false;
  // Rejeita sequências repetidas: 000.000.000-00, 111.111.111-11, etc.
  if (/^(\d)\1{10}$/.test(d)) return false;

  let soma = 0;
  for (let i = 0; i < 9; i++) {
    soma += parseInt(d.charAt(i), 10) * (10 - i);
  }
  let resto = (soma * 10) % 11;
  if (resto === 10 || resto === 11) resto = 0;
  if (resto !== parseInt(d.charAt(9), 10)) return false;

  soma = 0;
  for (let i = 0; i < 10; i++) {
    soma += parseInt(d.charAt(i), 10) * (11 - i);
  }
  resto = (soma * 10) % 11;
  if (resto === 10 || resto === 11) resto = 0;
  return resto === parseInt(d.charAt(10), 10);
}

/**
 * Valida telefone brasileiro: opcional (se vazio retorna true).
 * Se preenchido, exige DDD válido (11 a 99) e 10 ou 11 dígitos numéricos.
 */
export function validarTelefone(tel) {
  if (!tel || String(tel).trim() === "") return true;
  const d = String(tel).replace(/\D/g, "");
  if (d.length !== 10 && d.length !== 11) return false;
  const ddd = parseInt(d.slice(0, 2), 10);
  if (ddd < 11 || ddd > 99) return false;
  return true;
}
