//! Testes dos repositórios de produtos contra SQLite real: FTS5, atualização
//! do índice e paginação por keyset.

use mikella_domain_inventory::ports::ProdutoRepository;
use mikella_domain_inventory::produto::Produto;
use mikella_infra_sqlite::{Database, ProdutoRepo};
use mikella_shared_kernel::ids::ProdutoId;
use mikella_shared_kernel::money::Money;
use mikella_shared_kernel::pagination::PageParams;

fn produto(nome: &str, preco: i64) -> Produto {
    Produto::novo(
        ProdutoId::new(),
        nome.to_string(),
        Money::from_centavos(preco),
    )
    .unwrap()
}

fn params(cursor: Option<String>, limit: u32) -> PageParams {
    PageParams { cursor, limit }
}

#[test]
fn pesquisa_fts_encontra_sem_acento_e_pagina() {
    let db = Database::em_memoria().unwrap();
    let repo = ProdutoRepo::novo(db.conn());

    repo.salvar(&produto("Café Torrado", 1_890)).unwrap();
    repo.salvar(&produto("Café Moído", 1_590)).unwrap();
    repo.salvar(&produto("Leite Integral", 550)).unwrap();

    let primeira = repo.pesquisar("cafe", params(None, 1)).unwrap();
    assert_eq!(primeira.items.len(), 1);
    assert!(primeira.next_cursor.is_some());

    let segunda = repo
        .pesquisar("cafe", params(primeira.next_cursor.clone(), 1))
        .unwrap();
    assert_eq!(segunda.items.len(), 1);
    assert_ne!(
        primeira.items[0].id, segunda.items[0].id,
        "a segunda página não pode repetir o item da primeira"
    );

    let terceira = repo
        .pesquisar("cafe", params(segunda.next_cursor, 1))
        .unwrap();
    assert!(terceira.items.is_empty());
    assert!(terceira.next_cursor.is_none());
}

#[test]
fn busca_com_pontuacao_nao_vira_sintaxe_fts() {
    let db = Database::em_memoria().unwrap();
    let repo = ProdutoRepo::novo(db.conn());

    repo.salvar(&produto("Café Torrado", 1_890)).unwrap();

    // Aspas, operadores e comentário não podem quebrar a consulta nem vazar
    // como sintaxe para o FTS5.
    // Vira a frase "cafe OR 1 1" — não acha nada, mas não pode dar erro de sintaxe.
    repo.pesquisar("cafe\" OR 1=1 --", params(None, 10))
        .expect("operadores FTS colados não podem quebrar a consulta");

    for consulta in ["cafe*", "((cafe))", "café!"] {
        let pagina = repo
            .pesquisar(consulta, params(None, 10))
            .unwrap_or_else(|erro| panic!("consulta {consulta:?} falhou: {erro}"));
        assert!(
            !pagina.items.is_empty(),
            "consulta {consulta:?} deveria achar Café"
        );
    }

    // Sem nenhuma palavra, a busca vira "listar ativos" — não é erro.
    let sem_palavras = repo.pesquisar("\"!@#$%¨&*()", params(None, 10)).unwrap();
    assert_eq!(sem_palavras.items.len(), 1);
}

#[test]
fn listar_ativos_pagina_por_nome_e_id() {
    let db = Database::em_memoria().unwrap();
    let repo = ProdutoRepo::novo(db.conn());

    repo.salvar(&produto("Café", 1_000)).unwrap();
    repo.salvar(&produto("Leite", 500)).unwrap();
    repo.salvar(&produto("Arroz", 2_000)).unwrap();

    let primeira = repo.listar_ativos(params(None, 2)).unwrap();
    assert_eq!(primeira.items.len(), 2);
    assert_eq!(primeira.items[0].nome, "Arroz");
    assert_eq!(primeira.items[1].nome, "Café");
    assert!(primeira.next_cursor.is_some());

    let segunda = repo.listar_ativos(params(primeira.next_cursor, 2)).unwrap();
    assert_eq!(segunda.items.len(), 1);
    assert_eq!(segunda.items[0].nome, "Leite");
    assert!(segunda.next_cursor.is_none());
}

#[test]
fn atualizar_produto_reindexa_fts() {
    let db = Database::em_memoria().unwrap();
    let repo = ProdutoRepo::novo(db.conn());

    let mut leite = produto("Leite Integral", 550);
    repo.salvar(&leite).unwrap();

    leite.nome = "Iogurte Natural".into();
    repo.atualizar(&leite).unwrap();

    let encontrados = repo.pesquisar("iogurte", params(None, 10)).unwrap();
    assert_eq!(encontrados.items.len(), 1);
    assert_eq!(encontrados.items[0].nome, "Iogurte Natural");

    let antigos = repo.pesquisar("leite", params(None, 10)).unwrap();
    assert!(antigos.items.is_empty());
}

#[test]
fn arquivado_sai_da_lista_e_da_busca_mas_continua_por_id() {
    let db = Database::em_memoria().unwrap();
    let repo = ProdutoRepo::novo(db.conn());

    let mut cafe = produto("Café", 1_000);
    repo.salvar(&cafe).unwrap();
    cafe.arquivar(chrono::Utc::now()).unwrap();
    repo.atualizar(&cafe).unwrap();

    assert!(repo
        .listar_ativos(params(None, 10))
        .unwrap()
        .items
        .is_empty());
    assert!(repo
        .pesquisar("cafe", params(None, 10))
        .unwrap()
        .items
        .is_empty());
    assert!(repo.buscar_por_id(cafe.id).unwrap().is_some());
}

#[test]
fn codigo_de_barras_e_unico_e_recuperavel() {
    let db = Database::em_memoria().unwrap();
    let repo = ProdutoRepo::novo(db.conn());

    let mut cafe = produto("Café", 1_000);
    cafe.codigo_barras = Some("7891234567890".into());
    repo.salvar(&cafe).unwrap();

    let encontrado = repo
        .buscar_por_codigo_barras("7891234567890")
        .unwrap()
        .expect("produto deve ser encontrado pelo código de barras");
    assert_eq!(encontrado.id, cafe.id);

    let mut duplicado = produto("Outro", 500);
    duplicado.codigo_barras = Some("7891234567890".into());
    assert!(
        repo.salvar(&duplicado).is_err(),
        "código de barras duplicado deve falhar"
    );
}

#[test]
fn sku_e_unico_e_recuperavel() {
    let db = Database::em_memoria().unwrap();
    let repo = ProdutoRepo::novo(db.conn());

    let mut cafe = produto("Café", 1_000);
    cafe.sku = Some("CAFE-001".into());
    repo.salvar(&cafe).unwrap();

    let encontrado = repo
        .buscar_por_sku("CAFE-001")
        .unwrap()
        .expect("produto deve ser encontrado pelo SKU");
    assert_eq!(encontrado.id, cafe.id);

    // SKU nulo não encontra nada.
    assert!(repo.buscar_por_sku("INEXISTENTE").unwrap().is_none());

    let mut duplicado = produto("Outro", 500);
    duplicado.sku = Some("CAFE-001".into());
    assert!(
        repo.salvar(&duplicado).is_err(),
        "SKU duplicado deve falhar"
    );
}
