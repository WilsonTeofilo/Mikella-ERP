# Mikella-ERP

Sistema de Gestão Comercial e Ponto de Venda (PDV) de Alta Velocidade e Local-First para Pequeno Varejo Brasileiro.

## 🚀 Visão Geral
- **Tecnologia:** Desktop nativo (Tauri 2 + Rust + HTML5/CSS3 Puro + SQLite).
- **Filosofia:** Local-First. O sistema funciona 100% desconectado, com autoridade e validação de regras de negócio estritamente no domínio em Rust.
- **Piso de Hardware:** Suporte a máquinas de baixo consumo (Intel Core i3/i5 5ª geração ou equivalente, 6 GB DDR3-1333, HDD 5400rpm).
- **Sistemas Operacionais Suportados:** Windows 10/11 e Linux (Ubuntu/Lubuntu/Mint/Zorin). Windows 7 é terminantemente proibido.

## 🛡️ Princípios Invioláveis de Arquitetura
1. **Dinheiro estritamente em Centavos Inteiros:** `Money(i64)`, aritmética com checagem de overflow (`checked_add`, `checked_sub`), zero `float`/`f64`.
2. **Dados Imutáveis e Livro-Caixa:** Tabelas de movimentação e livro caixa são estritamente `INSERT ONLY`. Triggers SQL bloqueiam `UPDATE` e `DELETE`. Cadeia de hashes BLAKE3 em cada movimentação de caixa.
3. **Autoridade no Domínio:** A interface (frontend) é apenas apresentação; nunca calcula totais ou decide permissões.
4. **Clean Architecture:** Camadas de domínio (`domain-*`) sem dependências de infraestrutura, banco de dados ou framework web.
5. **Privacidade e Segurança:** Sem vazamento de dados sensíveis ou senhas em logs (`tracing`). Senhas hasheadas com Argon2id + salt via CSPRNG (`OsRng`). Códigos 2FA keyados com BLAKE3.

## 📁 Estrutura do Repositório
```
├── BASE/                   # Especificações técnicas, regras de negócio e UX
├── PLANO E FASES/          # Roadmap, plano mestre e fases de desenvolvimento
├── mikella/                # Workspace Cargo do sistema
│   ├── crates/
│   │   ├── shared-kernel/  # Tipos compartilhados (Money, IDs, Documentos, Clock)
│   │   ├── domain-*/       # Domínios puros (Cash, Sales, Inventory, Finance, People)
│   │   ├── app-*/          # Casos de uso e orquestração de aplicação
│   │   ├── infra-sqlite/   # SQLite, migrações com BLAKE3, triggers e repositórios
│   │   ├── infra-hardware/ # ESC/POS para bobinas 58mm/80mm e impressoras térmicas
│   │   ├── store-server/   # Servidor HTTP local (Axum, 127.0.0.1:47823)
│   │   └── desktop-shell/  # Shell desktop nativo (Tauri 2 + WebView2)
│   ├── frontend/           # Interface do balcão e retaguarda em HTML5/CSS3 puro
│   └── tools/              # Utilitários (seed de dados, benchmarks)
├── .env.example            # Exemplo de configuração de variáveis de ambiente
└── .gitignore              # Proteção contra vazamento de bancos, binários e segredos
```

## 🛠️ Como Iniciar em Desenvolvimento

### Pré-requisitos
- Rust (toolchain estável 1.80+)
- Visual Studio C++ Build Tools (Windows) ou WebKitGTK 4.1 (Linux)

### Executando os Testes
```bash
cd mikella
cargo test --workspace
```

### Configurando o Ambiente
Copie o arquivo de exemplo de ambiente:
```bash
cp .env.example .env
```
