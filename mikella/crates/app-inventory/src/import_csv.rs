//! Importação de produtos via CSV (PROD-010).
//!
//! ## Formato esperado
//!
//! Colunas (case-insensitive, ordem não importa):
//!
//! | Coluna | Obrigatório | Formato |
//! |---|---|---|
//! | `nome` | sim | Texto |
//! | `preco` | sim | Decimal pt-BR (`25,90`) ou US (`25.90`) |
//! | `codigo_barras` | não | Texto (apenas dígitos) |
//! | `sku` | não | Texto |
//! | `unidade` | não | `UN`, `KG`, `L`, `CX`, `PCT` |
//! | `custo` | não | Decimal — default `0,00` |
//! | `estoque_minimo` | não | Inteiro — default `0` |
//!
//! Regras:
//! - O arquivo deve ter header.
//! - Linhas em branco são ignoradas.
//! - Cada linha é validada independentemente; falhas não abortam o lote.
//! - Duplicatas de SKU/código de barras dentro do CSV são detectadas.
//! - Preços aceitam `.` ou `,` como separador decimal.

use mikella_domain_inventory::produto::Unidade;
use mikella_shared_kernel::ids::ProdutoId;
use mikella_shared_kernel::money::Money;

use super::produto::{CriarProdutoCmd, ProdutoRepos};
use super::criar_produto;

/// Uma linha CSV mapeada para `CriarProdutoCmd`.
#[derive(Debug, serde::Deserialize)]
struct CsvRow {
    nome: String,
    preco: String, // parse manual para aceitar , ou .
    #[serde(default)]
    codigo_barras: Option<String>,
    #[serde(default)]
    sku: Option<String>,
    #[serde(default)]
    unidade: Option<String>,
    #[serde(default)]
    custo: Option<String>,
    #[serde(default)]
    estoque_minimo: Option<String>,
}

/// Resultado de uma linha importada.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum ResultadoLinha {
    Sucesso { indice: usize, nome: String, produto_id: String },
    Erro { indice: usize, nome: Option<String>, erro: String },
}

/// Resultado do lote completo.
#[derive(Debug, Default)]
pub struct ResultadoImportacao {
    pub total: usize,
    pub sucessos: usize,
    pub erros: usize,
    pub detalhes: Vec<ResultadoLinha>,
}

/// Limite de linhas por importação — protege o PC fraco (RNF de hardware).
pub const MAX_LINHAS_CSV: usize = 5_000;

/// Limite de detalhes devolvidos; o restante vira contagem agregada.
pub const MAX_DETALHES_CSV: usize = 200;

/// Faz o parse de um valor decimal que pode usar `.` ou `,` como separador.
///
/// Exemplos:
/// - `"25.90"` -> 2590 centavos
/// - `"25,90"` -> 2590 centavos
/// - `"  100  "` -> 10000 centavos
fn parse_decimal_para_centavos(valor: &str) -> Result<i64, String> {
    let limpo = valor.trim().replace(' ', "");

    // Determina qual é o separador decimal (último . ou ,)
    let (reais_str, centavos_str) = if limpo.contains(',') && limpo.contains('.') {
        if limpo.rfind(',').unwrap_or(0) > limpo.rfind('.').unwrap_or(0) {
            // vírgula é última → decimal é vírgula; remove pontos (milhar)
            let sem_milhar = limpo.replace('.', "");
            let parts: Vec<&str> = sem_milhar.split(',').collect();
            (parts[0].to_string(), parts.get(1).unwrap_or(&"0").to_string())
        } else {
            // ponto é última → decimal é ponto; remove vírgulas (milhar)
            let sem_milhar = limpo.replace(',', "");
            let parts: Vec<&str> = sem_milhar.split('.').collect();
            (parts[0].to_string(), parts.get(1).unwrap_or(&"0").to_string())
        }
    } else if limpo.contains(',') {
        let parts: Vec<&str> = limpo.split(',').collect();
        (parts[0].to_string(), parts.get(1).unwrap_or(&"0").to_string())
    } else if limpo.contains('.') {
        let parts: Vec<&str> = limpo.split('.').collect();
        (parts[0].to_string(), parts.get(1).unwrap_or(&"0").to_string())
    } else {
        (limpo, "0".to_string())
    };

    let reais: i64 = reais_str
        .parse()
        .map_err(|_| format!("valor inválido: {}", valor))?;
    if reais < 0 {
        return Err(format!("valor não pode ser negativo: {}", valor));
    }
    let centavos_parte: u8 = if centavos_str.is_empty() {
        0
    } else if centavos_str.len() == 1 {
        centavos_str.parse::<u8>()
            .map_err(|_| format!("centavos inválidos: {}", valor))? * 10
    } else if centavos_str.len() == 2 {
        centavos_str
            .parse()
            .map_err(|_| format!("centavos inválidos: {}", valor))?
    } else {
        return Err(format!(
            "valor com mais de duas casas decimais: {}",
            valor
        ));
    };

    let total = Money::from_reais(reais, centavos_parte)
        .map_err(|e| format!("valor inválido: {}", e))?;
    Ok(total.centavos())
}

/// Converte uma `CsvRow` em `CriarProdutoCmd`.
fn row_para_cmd(row: CsvRow, indice: usize) -> Result<CriarProdutoCmd, String> {
    let nome = row.nome.trim();
    if nome.is_empty() {
        return Err(format!("linha {}: nome vazio", indice));
    }
    if nome.len() > 200 {
        return Err(format!("linha {}: nome excede 200 caracteres", indice));
    }

    // Parse preço
    let preco_centavos = parse_decimal_para_centavos(&row.preco)?;
    let preco = Money::from_centavos(preco_centavos);

    // Parse custo (opcional)
    let custo = match row.custo.as_deref() {
        Some(c) if !c.trim().is_empty() => {
            let cents = parse_decimal_para_centavos(c)?;
            Money::from_centavos(cents)
        }
        _ => Money::ZERO,
    };

    // Parse unidade
    let unidade = match row.unidade.as_deref() {
        Some(u) if !u.trim().is_empty() => Unidade::from_db(u.trim())
            .map_err(|e| format!("linha {}: unidade inválida: {}", indice, e))?,
        _ => Unidade::Un,
    };

    // Parse estoque mínimo
    let estoque_minimo = match row.estoque_minimo.as_deref() {
        Some(e) if !e.trim().is_empty() => {
            let n: i32 = e
                .trim()
                .parse()
                .map_err(|_| format!("linha {}: estoque_minimo inválido", indice))?;
            if n < 0 {
                return Err(format!(
                    "linha {}: estoque_minimo não pode ser negativo",
                    indice
                ));
            }
            n
        }
        _ => 0,
    };

    let codigo_barras = row.codigo_barras.and_then(|s| {
        let lim = s.trim().to_string();
        if lim.is_empty() {
            None
        } else {
            Some(lim)
        }
    });
    if let Some(cb) = &codigo_barras {
        if !cb.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!(
                "linha {}: código de barras deve conter apenas dígitos",
                indice
            ));
        }
        if cb.len() > 64 {
            return Err(format!("linha {}: código de barras muito longo", indice));
        }
    }

    let sku = row.sku.and_then(|s| {
        let lim = s.trim().to_string();
        if lim.is_empty() {
            None
        } else {
            Some(lim)
        }
    });
    if let Some(s) = &sku {
        if s.len() > 64 {
            return Err(format!("linha {}: SKU muito longo", indice));
        }
    }

    Ok(CriarProdutoCmd {
        produto_id: ProdutoId::new(),
        nome: nome.to_string(),
        preco,
        codigo_barras,
        sku,
        categoria_id: None,
        unidade,
        custo,
        estoque_minimo,
        perecivel: false,
        alerta_validade_dias: 60,
    })
}

/// Importa produtos a partir de conteúdo CSV.
///
/// Regras:
/// - O CSV deve ter header compatível com as colunas definidas em `CsvRow`.
/// - Cada linha é processada independentemente.
/// - Duplicatas dentro do CSV são detectadas e reportadas como erro.
/// - SKU/código de barras duplicados no banco também são reportados por linha.
///
/// # Exemplo CSV
/// ```text
/// nome,preco,codigo_barras,sku,unidade,custo,estoque_minimo
/// Arroz 5kg,25,90,7891234567895,ARROZ-5KG,UN,15,00,10
/// Feijao,5.50,7891234567896,FEIJAO,KG,3,20,5
/// ```
pub fn importar_produtos_csv(
    repos: &ProdutoRepos<'_>,
    csv_text: &str,
) -> Result<ResultadoImportacao, String> {
    // O CSV usa virgula como separador decimal em pt-BR; permita campos extra
    // (por exemplo, "25,90" como um único campo em vez de dois).
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(csv_text.as_bytes());

    // Valida header
    let headers = reader.headers()
        .map_err(|e| format!("erro ler header CSV: {}", e))?
        .clone();

    // Verifica se temos pelo menos 'nome' e 'preco'
    let header_lower: Vec<String> = headers.iter()
        .map(|h| h.trim().to_lowercase())
        .collect();

    if !header_lower.contains(&"nome".to_string()) {
        return Err("coluna 'nome' é obrigatória".to_string());
    }
    if !header_lower.contains(&"preco".to_string()) {
        return Err("coluna 'preco' é obrigatória".to_string());
    }
    for (i, h) in header_lower.iter().enumerate() {
        if header_lower[..i].contains(h) {
            return Err(format!("coluna duplicada no header: '{h}'"));
        }
    }

    // `StringRecord::deserialize(None)` mapeia por posição, não por header.
    // Isso fazia `sku` cair em `codigo_barras` quando a ordem diferia do
    // struct. Passamos os headers normalizados explicitamente.
    let headers_normalizados = csv::StringRecord::from(header_lower.clone());

    let mut resultado = ResultadoImportacao::default();
    let mut skus_vistos: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut codigos_vistos: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (i, record_result) in reader.into_records().enumerate() {
        if resultado.total >= MAX_LINHAS_CSV {
            return Err(format!(
                "CSV excede o limite de {MAX_LINHAS_CSV} linhas"
            ));
        }

        let indice = i + 2; // +2: linha 1 = header, índice começa em 0
        resultado.total += 1;

        let record = match record_result {
            Ok(r) => r,
            Err(e) => {
                resultado.erros += 1;
                if resultado.detalhes.len() < MAX_DETALHES_CSV {
                    resultado.detalhes.push(ResultadoLinha::Erro {
                        indice,
                        nome: None,
                        erro: format!("erro de leitura: {}", e),
                    });
                }
                continue;
            }
        };

        let row: CsvRow = match record.deserialize(Some(&headers_normalizados)) {
            Ok(r) => r,
            Err(e) => {
                let nome = record.get(0).map(|s| s.trim().to_string());
                resultado.erros += 1;
                if resultado.detalhes.len() < MAX_DETALHES_CSV {
                    resultado.detalhes.push(ResultadoLinha::Erro {
                        indice,
                        nome,
                        erro: format!("erro de parse: {}", e),
                    });
                }
                continue;
            }
        };

        let nome = row.nome.trim().to_string();

        // Converte para cmd antes de marcar duplicatas: uma linha inválida não
        // pode "reservar" um SKU e fazer a próxima linha válida falhar.
        let cmd = match row_para_cmd(row, indice) {
            Ok(c) => c,
            Err(e) => {
                resultado.erros += 1;
                if resultado.detalhes.len() < MAX_DETALHES_CSV {
                    resultado.detalhes.push(ResultadoLinha::Erro {
                        indice,
                        nome: Some(nome),
                        erro: e,
                    });
                }
                continue;
            }
        };

        if let Some(sku) = cmd.sku.as_ref() {
            if !skus_vistos.insert(sku.clone()) {
                resultado.erros += 1;
                if resultado.detalhes.len() < MAX_DETALHES_CSV {
                    resultado.detalhes.push(ResultadoLinha::Erro {
                        indice,
                        nome: Some(nome.clone()),
                        erro: format!("SKU '{}' duplicado dentro do CSV", sku),
                    });
                }
                continue;
            }
        }

        if let Some(cb) = cmd.codigo_barras.as_ref() {
            if !codigos_vistos.insert(cb.clone()) {
                resultado.erros += 1;
                if resultado.detalhes.len() < MAX_DETALHES_CSV {
                    resultado.detalhes.push(ResultadoLinha::Erro {
                        indice,
                        nome: Some(nome.clone()),
                        erro: format!("código de barras '{}' duplicado dentro do CSV", cb),
                    });
                }
                continue;
            }
        }

        match criar_produto(repos, cmd) {
            Ok(produto) => {
                resultado.sucessos += 1;
                if resultado.detalhes.len() < MAX_DETALHES_CSV {
                    resultado.detalhes.push(ResultadoLinha::Sucesso {
                        indice,
                        nome: produto.nome.clone(),
                        produto_id: produto.id.to_string(),
                    });
                }
            }
            Err(e) => {
                resultado.erros += 1;
                if resultado.detalhes.len() < MAX_DETALHES_CSV {
                    resultado.detalhes.push(ResultadoLinha::Erro {
                        indice,
                        nome: Some(nome),
                        erro: format!("duplicado no banco ou erro de validação: {}", e),
                    });
                }
            }
        }
    }

    Ok(resultado)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mikella_domain_inventory::ports::ProdutoRepository;
    use mikella_domain_inventory::produto::Produto;

    /// Mock de repositório para testes.
    struct MockProdutoRepo {
        produtos: std::sync::Mutex<Vec<Produto>>,
    }

    impl MockProdutoRepo {
        fn new() -> Self {
            Self { produtos: std::sync::Mutex::new(Vec::new()) }
        }
    }

    impl ProdutoRepository for MockProdutoRepo {
        fn salvar(&self, produto: &Produto) -> mikella_shared_kernel::error::DomainResult<()> {
            self.produtos.lock().unwrap().push(produto.clone());
            Ok(())
        }
        fn atualizar(&self, produto: &Produto) -> mikella_shared_kernel::error::DomainResult<()> {
            let mut vec = self.produtos.lock().unwrap();
            if let Some(pos) = vec.iter().position(|p| p.id == produto.id) {
                vec[pos] = produto.clone();
            } else {
                vec.push(produto.clone());
            }
            Ok(())
        }
        fn buscar_por_id(&self, id: ProdutoId) -> mikella_shared_kernel::error::DomainResult<Option<Produto>> {
            Ok(self.produtos.lock().unwrap().iter().find(|p| p.id == id).cloned())
        }
        fn buscar_por_codigo_barras(&self, cb: &str) -> mikella_shared_kernel::error::DomainResult<Option<Produto>> {
            Ok(self.produtos.lock().unwrap().iter()
                .find(|p| p.codigo_barras.as_deref() == Some(cb))
                .cloned())
        }
        fn buscar_por_sku(&self, sku: &str) -> mikella_shared_kernel::error::DomainResult<Option<Produto>> {
            Ok(self.produtos.lock().unwrap().iter()
                .find(|p| p.sku.as_deref() == Some(sku))
                .cloned())
        }
        fn pesquisar(&self, _query: &str, _params: mikella_shared_kernel::pagination::PageParams) -> mikella_shared_kernel::error::DomainResult<mikella_shared_kernel::pagination::Page<Produto>> {
            Ok(mikella_shared_kernel::pagination::Page::empty())
        }
        fn listar_ativos(&self, _params: mikella_shared_kernel::pagination::PageParams) -> mikella_shared_kernel::error::DomainResult<mikella_shared_kernel::pagination::Page<Produto>> {
            Ok(mikella_shared_kernel::pagination::Page::empty())
        }
        fn estoque_atual(&self, _id: ProdutoId) -> mikella_shared_kernel::error::DomainResult<i32> {
            Ok(0)
        }
    }

    fn setup() -> ProdutoRepos<'static> {
        let repo = Box::leak(Box::new(MockProdutoRepo::new()));
        ProdutoRepos { produtos: repo }
    }

    #[test]
    fn parse_decimal_ponto() {
        assert_eq!(parse_decimal_para_centavos("25.90").unwrap(), 2590);
    }

    #[test]
    fn parse_decimal_virgula() {
        assert_eq!(parse_decimal_para_centavos("25,90").unwrap(), 2590);
    }

    #[test]
    fn parse_decimal_inteiro() {
        assert_eq!(parse_decimal_para_centavos("100").unwrap(), 10000);
    }

    #[test]
    fn parse_decimal_com_espaco() {
        assert_eq!(parse_decimal_para_centavos("  25,90  ").unwrap(), 2590);
    }

    #[test]
    fn parse_decimal_milhar_e_decimal() {
        assert_eq!(parse_decimal_para_centavos("1.000,00").unwrap(), 100000);
        assert_eq!(parse_decimal_para_centavos("1,000.00").unwrap(), 100000);
    }

    #[test]
    fn import_csv_sucesso() {
        let repos = setup();

        let csv = "nome,preco,codigo_barras,sku,unidade,custo,estoque_minimo\n\
            Arroz 5kg,25.90,7891234567895,ARROZ-5KG,UN,15.00,10\n\
            Feijao,5.50,7891234567896,FEIJAO,KG,3.20,20\n";

        let result = importar_produtos_csv(&repos, csv);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.total, 2);
        assert_eq!(r.sucessos, 2);
        assert_eq!(r.erros, 0);
    }

    #[test]
    fn import_csv_linha_erro_nome_vazio() {
        let repos = setup();

        let csv = "nome,preco\n\
            ,25.90\n";

        let result = importar_produtos_csv(&repos, csv);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.total, 1);
        assert_eq!(r.sucessos, 0);
        assert_eq!(r.erros, 1);
    }

    #[test]
    fn import_csv_skus_duplicados() {
        let repos = setup();

        let csv = "nome,preco,sku\nProd A,10.00,SKU-001\nProd B,20.00,SKU-001\n";

        let result = importar_produtos_csv(&repos, csv);
        println!("RESULT: {:?}", result);
        assert!(result.is_ok());
        let r = result.unwrap();
        for d in &r.detalhes {
            println!("  DETALHE: {:?}", d);
        }
        assert_eq!(r.total, 2);
        assert_eq!(r.sucessos, 1);
        assert_eq!(r.erros, 1);
    }

    #[test]
    fn import_csv_sem_header_nome() {
        let repos = setup();

        let csv = "produto,preco\nProduto A,10.00\n";

        let result = importar_produtos_csv(&repos, csv);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("nome"));
    }

    #[test]
    fn import_csv_precisa_decimal_virgula() {
        let repos = setup();

        let csv = "nome,preco\nProduto A,25,90\n";

        let result = importar_produtos_csv(&repos, csv);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.sucessos, 1);
    }

    #[test]
    fn import_csv_codigo_barras_duplicado() {
        let repos = setup();

        let csv = "nome,preco,codigo_barras\n\
            Prod A,10.00,7890000000001\n\
            Prod B,20.00,7890000000001\n";

        let result = importar_produtos_csv(&repos, csv);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.total, 2);
        assert_eq!(r.sucessos, 1);
        assert_eq!(r.erros, 1);
    }

    #[test]
    fn import_csv_unidade_opcional_default_un() {
        let repos = setup();

        let csv = "nome,preco,unidade\nProduto A,10.00,\n";

        let result = importar_produtos_csv(&repos, csv);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.sucessos, 1);
    }
}
