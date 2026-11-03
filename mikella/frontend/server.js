// =============================================================================
// Servidor estático de preview visual — NÃO PARTE DO PRODUTO FINAL.
//
// Este servidor existe APENAS para desenvolvimento local de assets HTML/CSS.
// Ele NÃO faz parte do produto final e NÃO será distribuído com o Mikella ERP.
//
// O produto final usa Tauri (desktop-shell) para servir os assets localmente
// dentro do binário — sem dependência de Node.js ou HTTP no cliente.
//
// Para iniciar o preview:
//   cd mikella/frontend
//   node server.js
//   # abra http://127.0.0.1:3000
//
// Após validação visual, os arquivos HTML/CSS são migrados para o desktop-shell
// (Tauri) e este servidor pode ser removido.
// =============================================================================
const http = require('http');
const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, 'src');
const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.woff2': 'font/woff2',
  '.svg': 'image/svg+xml',
};

const server = http.createServer((req, res) => {
  let urlPath = decodeURIComponent(req.url.split('?')[0]);
  if (urlPath === '/') urlPath = '/app/landing.html';

  let filePath = path.join(ROOT, urlPath);
  if (!fs.existsSync(filePath) || fs.statSync(filePath).isDirectory()) {
    filePath = path.join(filePath, 'index.html');
  }

  if (!fs.existsSync(filePath)) {
    res.writeHead(404);
    res.end('Not found');
    return;
  }

  const ext = path.extname(filePath).toLowerCase();
  const mime = MIME[ext] || 'application/octet-stream';

  fs.readFile(filePath, (err, data) => {
    if (err) {
      res.writeHead(500);
      res.end('Error');
      return;
    }
    res.writeHead(200, { 'Content-Type': mime });
    res.end(data);
  });
});

server.listen(3000, '127.0.0.1', () => {
  console.log('Servidor rodando em http://127.0.0.1:3000');
});
