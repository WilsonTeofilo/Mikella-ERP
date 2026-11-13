// Migra atributos `style="..."` dos HTMLs para classes utilitárias estáveis.
//
// Motivo: a CSP do desktop precisa deixar `style-src 'self'` (SEC-009). Atributo
// `style` inline é bloqueado pelo WebView2 e não pode ser liberado por hash de
// forma segura. Cada declaração única vira uma classe `.u-<hash8>` gerada em
// shared/design/utilitarios.css.
//
// O script é idempotente: se não houver mais `style=` em HTML, não escreve nada.
//
// Uso: node scripts/migrar-estilos-inline.mjs

import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const raiz = join(dirname(fileURLToPath(import.meta.url)), "..", "frontend", "src");
const cssDestino = join(raiz, "shared", "design", "utilitarios.css");

function listarHtml(dir) {
  const arquivos = [];
  for (const entrada of readdirSync(dir)) {
    const caminho = join(dir, entrada);
    const info = statSync(caminho);
    if (info.isDirectory()) {
      arquivos.push(...listarHtml(caminho));
    } else if (caminho.endsWith(".html")) {
      arquivos.push(caminho);
    }
  }
  return arquivos;
}

function normalizar(css) {
  return css
    .split(";")
    .map((parte) => parte.trim())
    .filter(Boolean)
    .map((declaracao) => {
      const separador = declaracao.indexOf(":");
      if (separador === -1) return declaracao;
      const propriedade = declaracao.slice(0, separador).trim().toLowerCase();
      const valor = declaracao.slice(separador + 1).trim().replace(/\s+/g, " ");
      return `${propriedade}: ${valor}`;
    })
    .join("; ");
}

function classePara(normalizado) {
  return `u-${createHash("sha1").update(normalizado).digest("hex").slice(0, 8)}`;
}

const utilitarios = new Map();
let totalAtributos = 0;
const arquivosAlterados = [];

for (const arquivo of listarHtml(raiz)) {
  const original = readFileSync(arquivo, "utf8");
  if (!/\sstyle\s*=\s*"/i.test(original)) continue;

  const texto = original.replace(/<[a-zA-Z][^>]*>/gs, (tag) => {
    const comEstilo = tag.match(/\sstyle\s*=\s*"([^"]*)"/i);
    if (!comEstilo) return tag;

    const normalizado = normalizar(comEstilo[1]);
    if (!normalizado) return tag.replace(/\sstyle\s*=\s*"[^"]*"/i, "");

    const classe = classePara(normalizado);
    utilitarios.set(classe, normalizado);
    totalAtributos += 1;

    const semEstilo = tag.replace(/\sstyle\s*=\s*"[^"]*"/i, "");
    const comClasse = semEstilo.match(/\sclass\s*=\s*"([^"]*)"/i);
    if (comClasse) {
      const existentes = comClasse[1].trim();
      const novas = existentes.split(/\s+/).includes(classe)
        ? existentes
        : `${existentes} ${classe}`.trim();
      return semEstilo.replace(comClasse[0], ` class="${novas}"`);
    }
    return semEstilo.replace(/^<([a-zA-Z0-9-]+)/, `<$1 class="${classe}"`);
  });

  let final = texto;
  const jaTemLink = /utilitarios\.css/.test(texto);
  if (!jaTemLink) {
    const caminhoRelativo = relative(dirname(arquivo), cssDestino).split("\\").join("/");
    final = texto.replace(
      /<\/head>/i,
      `  <link rel="stylesheet" href="${caminhoRelativo}" />\n</head>`,
    );
  }

  writeFileSync(arquivo, final, "utf8");
  arquivosAlterados.push(relative(raiz, arquivo));
}

if (utilitarios.size === 0) {
  console.log("Nenhum style= inline encontrado; nada a fazer.");
  process.exit(0);
}

const linhas = [
  "/* Utilitários gerados por scripts/migrar-estilos-inline.mjs.",
  " * Cada classe substitui um atributo style= que a CSP bloquearia.",
  " * Não edite à mão: rode o script novamente se precisar regenerar. */",
  "",
  ...[...utilitarios.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([classe, css]) => `.${classe} { ${css}; }`),
  "",
];
writeFileSync(cssDestino, linhas.join("\n"), "utf8");

console.log(`style= migrados: ${totalAtributos}`);
console.log(`classes únicas:   ${utilitarios.size}`);
console.log(`arquivos HTML:    ${arquivosAlterados.length}`);
for (const arquivo of arquivosAlterados) console.log(`  - ${arquivo}`);
console.log(`CSS:              ${relative(raiz, cssDestino)}`);
