//! Testes dos casos de uso de produto (criar, atualizar, arquivar).
//!
//! Executa contra SQLite em memória para validar regras de domínio:
//! - Unicidade de código de barras e SKU
//! - Arquivamento irreversível
//! - Proteção contra edição de produto arquivado

use mikella_app_inventory::produto::{
    arquivar_produto, atualizar_produto, criar_produto, AtualizarProdutoCmd, CriarProdutoCmd,
    ProdutoRepos,
};
use mikella_domain_inventory::ports::ProdutoRepository;
use mikella_domain_inventory::produto::Unidade;
use mikella_infra_sqlite::{Database, ProdutoRepo};
use mikella_shared_kernel::ids::ProdutoId;
use mikella_shared_kernel::money::Money;

#[test]
fn criar_produto_com_campo_unico() {
    let db = Database::em_memoria().expect("deve abrir DB em memória");
    let repo = ProdutoRepo::novo(db.conn());
    let repos = ProdutoRepos { produtos: &repo };

    let id = ProdutoId::new();
    let cmd = CriarProdutoCmd {
        produto_id: id,
        nome: "Café Torrado".into(),
        preco: Money::from_centavos(1890),
        codigo_barras: Some("7891234567890".into()),
        sku: Some("CAFE-TORRADO".into()),
        categoria_id: None,
        unidade: Unidade::Un,
        custo: Money::from_centavos(800),
        estoque_minimo: 10,
        perecivel: false,
        alerta_validade_dias: 60,
    };

    let produto = criar_produto(&repos, cmd).expect("criar produto deve suceder");
    assert_eq!(produto.nome, "Café Torrado");
    assert_eq!(produto.codigo_barras, Some("7891234567890".into()));
    assert_eq!(produto.sku, Some("CAFE-TORRADO".into()));
    assert_eq!(produto.unidade, Unidade::Un);
    assert!(!produto.arquivado);

    // Verifica que o DB persistiu.
    let encontrado = repo
        .buscar_por_codigo_barras("7891234567890")
        .expect("busca deve suceder")
        .expect("produto deve existir");
    assert_eq!(encontrado.id, id);
}

#[test]
fn criar_produto_com_codigo_barras_duplicado_falha() {
    let db = Database::em_memoria().expect("deve abrir DB em memória");
    let repo = ProdutoRepo::novo(db.conn());
    let repos = ProdutoRepos { produtos: &repo };

    let cmd1 = CriarProdutoCmd {
        produto_id: ProdutoId::new(),
        nome: "Produto A".into(),
        preco: Money::from_centavos(1000),
        codigo_barras: Some("111".into()),
        sku: None,
        categoria_id: None,
        unidade: Unidade::Un,
        custo: Money::ZERO,
        estoque_minimo: 0,
        perecivel: false,
        alerta_validade_dias: 60,
    };
    criar_produto(&repos, cmd1).expect("primeiro produto deve ser criado");

    let cmd2 = CriarProdutoCmd {
        produto_id: ProdutoId::new(),
        nome: "Produto B".into(),
        preco: Money::from_centavos(2000),
        codigo_barras: Some("111".into()),
        sku: None,
        categoria_id: None,
        unidade: Unidade::Un,
        custo: Money::ZERO,
        estoque_minimo: 0,
        perecivel: false,
        alerta_validade_dias: 60,
    };
    let result = criar_produto(&repos, cmd2);
    assert!(result.is_err(), "código de barras duplicado deve falhar");
}

#[test]
fn criar_produto_com_sku_duplicado_falha() {
    let db = Database::em_memoria().expect("deve abrir DB em memória");
    let repo = ProdutoRepo::novo(db.conn());
    let repos = ProdutoRepos { produtos: &repo };

    let cmd1 = CriarProdutoCmd {
        produto_id: ProdutoId::new(),
        nome: "Produto A".into(),
        preco: Money::from_centavos(1000),
        codigo_barras: None,
        sku: Some("SKU-001".into()),
        categoria_id: None,
        unidade: Unidade::Un,
        custo: Money::ZERO,
        estoque_minimo: 0,
        perecivel: false,
        alerta_validade_dias: 60,
    };
    criar_produto(&repos, cmd1).expect("primeiro produto deve ser criado");

    let cmd2 = CriarProdutoCmd {
        produto_id: ProdutoId::new(),
        nome: "Produto B".into(),
        preco: Money::from_centavos(2000),
        codigo_barras: None,
        sku: Some("SKU-001".into()),
        categoria_id: None,
        unidade: Unidade::Un,
        custo: Money::ZERO,
        estoque_minimo: 0,
        perecivel: false,
        alerta_validade_dias: 60,
    };
    let result = criar_produto(&repos, cmd2);
    assert!(result.is_err(), "SKU duplicado deve falhar");
}

#[test]
fn atualizar_produto_muda_nome_e_preserva_id() {
    let db = Database::em_memoria().expect("deve abrir DB em memória");
    let repo = ProdutoRepo::novo(db.conn());
    let repos = ProdutoRepos { produtos: &repo };

    let id = ProdutoId::new();
    let cmd = CriarProdutoCmd {
        produto_id: id,
        nome: "Café".into(),
        preco: Money::from_centavos(1500),
        codigo_barras: None,
        sku: None,
        categoria_id: None,
        unidade: Unidade::Un,
        custo: Money::ZERO,
        estoque_minimo: 5,
        perecivel: false,
        alerta_validade_dias: 60,
    };
    criar_produto(&repos, cmd).expect("criar produto");

    let cmd_update = AtualizarProdutoCmd {
        produto_id: id,
        codigo_barras: None,
        sku: None,
        nome: "Café Torrado Especial".into(),
        categoria_id: None,
        unidade: Unidade::Kg,
        preco: Money::from_centavos(1890),
        custo: Money::from_centavos(900),
        estoque_minimo: 10,
        perecivel: false,
        alerta_validade_dias: 60,
    };
    let atualizado = atualizar_produto(&repos, cmd_update).expect("atualizar deve suceder");
    assert_eq!(atualizado.nome, "Café Torrado Especial");
    assert_eq!(atualizado.unidade, Unidade::Kg);
    assert_eq!(atualizado.preco_centavos, Money::from_centavos(1890));
    assert_eq!(atualizado.custo_centavos, Money::from_centavos(900));
    assert_eq!(atualizado.estoque_minimo, 10);
}

#[test]
fn arquivar_produto_mexe_no_estado_e_nao_deleta() {
    let db = Database::em_memoria().expect("deve abrir DB em memória");
    let repo = ProdutoRepo::novo(db.conn());
    let repos = ProdutoRepos { produtos: &repo };

    let id = ProdutoId::new();
    let cmd = CriarProdutoCmd {
        produto_id: id,
        nome: "Produto Teste".into(),
        preco: Money::from_centavos(500),
        codigo_barras: None,
        sku: None,
        categoria_id: None,
        unidade: Unidade::Un,
        custo: Money::ZERO,
        estoque_minimo: 0,
        perecivel: false,
        alerta_validade_dias: 60,
    };
    criar_produto(&repos, cmd).expect("criar produto");

    // Arquivar
    arquivar_produto(&repos, id).expect("arquivar deve suceder");

    // Produto ainda existe por ID
    let produto = repo
        .buscar_por_id(id)
        .expect("busca por ID não deve falhar")
        .expect("produto deve existir após arquivamento");
    assert!(
        produto.arquivado,
        "produto deve estar marcado como arquivado"
    );
    assert!(
        produto.arquivado_em.is_some(),
        "deve ter timestamp de arquivamento"
    );

    // Não aparece na lista de ativos
    let ativos = repo
        .listar_ativos(Default::default())
        .expect("listagem deve suceder");
    assert!(
        ativos.items.is_empty(),
        "produto arquivado não deve aparecer na lista"
    );
}

#[test]
fn atualizar_produto_arquivado_falha() {
    let db = Database::em_memoria().expect("deve abrir DB em memória");
    let repo = ProdutoRepo::novo(db.conn());
    let repos = ProdutoRepos { produtos: &repo };

    let id = ProdutoId::new();
    let cmd = CriarProdutoCmd {
        produto_id: id,
        nome: "Produto Teste".into(),
        preco: Money::from_centavos(500),
        codigo_barras: None,
        sku: None,
        categoria_id: None,
        unidade: Unidade::Un,
        custo: Money::ZERO,
        estoque_minimo: 0,
        perecivel: false,
        alerta_validade_dias: 60,
    };
    criar_produto(&repos, cmd).expect("criar produto");
    arquivar_produto(&repos, id).expect("arquivar");

    let cmd_update = AtualizarProdutoCmd {
        produto_id: id,
        codigo_barras: None,
        sku: None,
        nome: "Novo Nome".into(),
        categoria_id: None,
        unidade: Unidade::Un,
        preco: Money::from_centavos(600),
        custo: Money::ZERO,
        estoque_minimo: 0,
        perecivel: false,
        alerta_validade_dias: 60,
    };
    let result = atualizar_produto(&repos, cmd_update);
    assert!(result.is_err(), "não pode atualizar produto arquivado");
}
