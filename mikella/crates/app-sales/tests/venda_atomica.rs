//! Testes de integração do caminho crítico: venda atômica + caixa.
//!
//! Usam SQLite real (em memória) — as triggers, CHECKs e FKs participam do
//! teste, então um erro de schema aparece aqui e não em produção.

use chrono::{DateTime, Utc};

use mikella_app_cash::{abrir_caixa, fechar_caixa, AbrirCaixaCmd, CaixaRepos, FecharCaixaCmd};
use mikella_app_sales::{finalizar_venda, FinalizarVendaCmd, ItemCmd, PoliticaEstoque, VendaRepos};
use mikella_domain_cash::ports::LivroCaixaRepository;
use mikella_domain_inventory::ports::MovimentacaoEstoqueRepository;
use mikella_domain_sales::ports::VendaRepository;
use mikella_domain_sales::venda::{FormaPagamento, Pagamento};
use mikella_infra_sqlite::hash::hash_entrada;
use mikella_infra_sqlite::{
    Blake3HashChain, Database, LivroRepo, LoteRepo, MovimentacaoRepo, ProdutoRepo, SessaoRepo,
    VendaRepo,
};
use mikella_shared_kernel::clock::Clock;
use mikella_shared_kernel::error::{DomainError, DomainResult};
use mikella_shared_kernel::ids::{DispositivoId, LoteId, ProdutoId, SessaoId, UsuarioId};
use mikella_shared_kernel::money::Money;

const U_DONO: &str = "00000000-0000-4000-8000-000000000001";
const U_OPERADOR: &str = "00000000-0000-4000-8000-000000000002";
const D_CAIXA: &str = "00000000-0000-4000-8000-0000000000d1";
const P_CAFE: &str = "00000000-0000-4000-8000-0000000000a1";
const P_LEITE: &str = "00000000-0000-4000-8000-0000000000a2";

struct RelogioFixo(DateTime<Utc>);

impl Clock for RelogioFixo {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

fn usuario(id: &str) -> UsuarioId {
    id.parse().unwrap()
}

fn dispositivo(id: &str) -> DispositivoId {
    id.parse().unwrap()
}

fn produto(id: &str) -> ProdutoId {
    id.parse().unwrap()
}

fn semear(db: &Database) {
    db.conn()
        .execute_batch(&format!(
            r#"
            INSERT INTO usuarios (id, nome, username, senha_hash, papel) VALUES
                ('{U_DONO}', 'Dono', 'dono', 'argon2-fake', 'DONO'),
                ('{U_OPERADOR}', 'Operador', 'operador', 'argon2-fake', 'OPERADOR_CAIXA');

            INSERT INTO dispositivos (id, nome, usuario_so, fingerprint, papel)
                VALUES ('{D_CAIXA}', 'PC-CAIXA', 'operador', 'fingerprint-fake', 'CAIXA');

            INSERT INTO produtos (id, nome, preco_centavos, custo_centavos, perecivel) VALUES
                ('{P_CAFE}', 'Café 500g', 1890, 900, 0),
                ('{P_LEITE}', 'Leite 1L', 550, 300, 0);

            INSERT INTO movimentacoes_estoque
                (id, produto_id, tipo, quantidade, custo_unitario_centavos, usuario_id) VALUES
                ('10000000-0000-4000-8000-000000000001', '{P_CAFE}', 'ENTRADA_COMPRA', 10, 900, '{U_DONO}'),
                ('10000000-0000-4000-8000-000000000002', '{P_LEITE}', 'ENTRADA_COMPRA', 5, 300, '{U_DONO}');
            "#
        ))
        .unwrap();
}

fn abrir_caixa_de_teste(db: &mut Database, inicial: i64) -> SessaoId {
    let sessao_id = SessaoId::new();
    let agora = Utc::now();

    let tx = db.transacao().unwrap();
    {
        let sessoes = SessaoRepo::novo(&tx);
        let livro = LivroRepo::novo(&tx);
        let hash = Blake3HashChain;
        let clock = RelogioFixo(agora);

        let repos = CaixaRepos {
            sessoes: &sessoes,
            livro: &livro,
            hash: &hash,
            clock: &clock,
        };

        abrir_caixa(
            &repos,
            AbrirCaixaCmd {
                sessao_id,
                numero_caixa: 1,
                dispositivo_id: dispositivo(D_CAIXA),
                operador_id: usuario(U_OPERADOR),
                saldo_inicial: Money::from_centavos(inicial),
                fechamento_cego: false,
            },
        )
        .unwrap();
    }
    tx.commit().unwrap();
    sessao_id
}

/// Repositório de estoque que falha de propósito em um produto — usado para
/// provar que a transação inteira volta atrás quando algo dá errado no meio.
struct EstoqueComFalha<'a> {
    real: &'a dyn MovimentacaoEstoqueRepository,
    produto_com_falha: Option<ProdutoId>,
}

impl MovimentacaoEstoqueRepository for EstoqueComFalha<'_> {
    fn registrar(
        &self,
        mov: &mikella_domain_inventory::movimentacao::MovimentacaoEstoque,
    ) -> DomainResult<()> {
        if Some(mov.produto_id) == self.produto_com_falha {
            return Err(DomainError::Infraestrutura(
                "falha simulada ao gravar movimentação".into(),
            ));
        }
        self.real.registrar(mov)
    }

    fn listar_por_produto(
        &self,
        produto_id: ProdutoId,
        params: mikella_shared_kernel::pagination::PageParams,
    ) -> DomainResult<
        mikella_shared_kernel::pagination::Page<
            mikella_domain_inventory::movimentacao::MovimentacaoEstoque,
        >,
    > {
        self.real.listar_por_produto(produto_id, params)
    }

    fn buscar_por_id(
        &self,
        id: mikella_shared_kernel::ids::MovimentacaoEstoqueId,
    ) -> DomainResult<Option<mikella_domain_inventory::movimentacao::MovimentacaoEstoque>> {
        self.real.buscar_por_id(id)
    }
}

fn executar_venda(
    db: &mut Database,
    cmd: FinalizarVendaCmd,
    falhar_no_produto: Option<ProdutoId>,
) -> DomainResult<mikella_domain_sales::venda::Venda> {
    let tx = db.transacao().unwrap();
    let resultado = {
        let produtos = ProdutoRepo::novo(&tx);
        let lotes = LoteRepo::novo(&tx);
        let vendas = VendaRepo::novo(&tx);
        let sessoes = SessaoRepo::novo(&tx);
        let livro = LivroRepo::novo(&tx);
        let estoque_real = MovimentacaoRepo::novo(&tx);
        let estoque = EstoqueComFalha {
            real: &estoque_real,
            produto_com_falha: falhar_no_produto,
        };
        let hash = Blake3HashChain;
        let clock = RelogioFixo(Utc::now());

        let repos = VendaRepos {
            produtos: &produtos,
            lotes: &lotes,
            vendas: &vendas,
            sessoes: &sessoes,
            livro: &livro,
            estoque: &estoque,
            hash: &hash,
            clock: &clock,
        };

        finalizar_venda(&repos, cmd)
    };

    match resultado {
        Ok(venda) => {
            tx.commit().unwrap();
            Ok(venda)
        }
        Err(erro) => {
            drop(tx); // rollback
            Err(erro)
        }
    }
}

fn cmd_venda(
    venda: &str,
    sessao: SessaoId,
    itens: Vec<(ProdutoId, u32)>,
    pagamentos: Vec<Pagamento>,
) -> FinalizarVendaCmd {
    FinalizarVendaCmd {
        venda_id: venda.parse().unwrap(),
        sessao_id: sessao,
        dispositivo_id: dispositivo(D_CAIXA),
        operador_id: usuario(U_OPERADOR),
        vendedor_id: None,
        comanda_id: None,
        itens: itens
            .into_iter()
            .map(|(produto_id, quantidade)| ItemCmd {
                produto_id,
                lote_id: None,
                quantidade,
            })
            .collect(),
        desconto: Money::ZERO,
        pagamentos,
        politica_estoque: PoliticaEstoque::Bloquear,
    }
}

fn contar(db: &Database, sql: &str) -> i64 {
    db.conn().query_row(sql, [], |row| row.get(0)).unwrap()
}

fn saldo_sessao(db: &Database, sessao: SessaoId) -> i64 {
    db.conn()
        .query_row(
            "SELECT saldo_esperado_centavos FROM sessoes_caixa WHERE id = ?1",
            [sessao.to_string()],
            |row| row.get(0),
        )
        .unwrap()
}

fn estoque(db: &Database, produto: &str) -> i64 {
    db.conn()
        .query_row(
            "SELECT saldo_atual FROM vw_estoque_atual WHERE produto_id = ?1",
            [produto],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn venda_em_dinheiro_grava_tudo_na_mesma_transacao() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);
    let sessao = abrir_caixa_de_teste(&mut db, 10_000);

    let venda = executar_venda(
        &mut db,
        cmd_venda(
            "20000000-0000-4000-8000-000000000001",
            sessao,
            vec![(produto(P_CAFE), 1), (produto(P_LEITE), 2)],
            vec![
                Pagamento::dinheiro(Money::from_centavos(2_990), Money::from_centavos(5_000))
                    .unwrap(),
            ],
        ),
        None,
    )
    .unwrap();

    assert_eq!(venda.total.centavos(), 2_990);
    assert_eq!(venda.troco.centavos(), 2_010);
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM vendas"), 1);
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM venda_itens"), 2);
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM venda_pagamentos"), 1);
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM movimentacoes_estoque"), 4);

    assert_eq!(estoque(&db, P_CAFE), 9);
    assert_eq!(estoque(&db, P_LEITE), 3);

    // 100,00 inicial + 29,90 líquido (50,00 recebidos - 20,10 de troco)
    assert_eq!(saldo_sessao(&db, sessao), 12_990);

    // Livro: ABERTURA + VENDA_DINHEIRO, cadeia íntegra.
    let tx = db.transacao().unwrap();
    let livro = LivroRepo::novo(&tx).carregar_livro(sessao).unwrap();
    assert_eq!(livro.len(), 2);
    livro
        .verificar_integridade(hash_entrada)
        .expect("cadeia de hashes do livro deve fechar");
}

#[test]
fn reenvio_da_mesma_venda_e_idempotente() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);
    let sessao = abrir_caixa_de_teste(&mut db, 10_000);

    let montar_cmd = || {
        cmd_venda(
            "20000000-0000-4000-8000-000000000002",
            sessao,
            vec![(produto(P_CAFE), 1)],
            vec![
                Pagamento::dinheiro(Money::from_centavos(1_890), Money::from_centavos(2_000))
                    .unwrap(),
            ],
        )
    };

    let primeira = executar_venda(&mut db, montar_cmd(), None).unwrap();
    let segunda = executar_venda(&mut db, montar_cmd(), None).unwrap();

    assert_eq!(primeira.id, segunda.id);
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM vendas"), 1);
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM venda_itens"), 1);
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM movimentacoes_estoque"), 3);
}

#[test]
fn falha_no_meio_da_venda_faz_rollback_de_tudo() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);
    let sessao = abrir_caixa_de_teste(&mut db, 10_000);

    let resultado = executar_venda(
        &mut db,
        cmd_venda(
            "20000000-0000-4000-8000-000000000003",
            sessao,
            vec![(produto(P_CAFE), 1), (produto(P_LEITE), 1)],
            vec![
                Pagamento::dinheiro(Money::from_centavos(2_440), Money::from_centavos(3_000))
                    .unwrap(),
            ],
        ),
        Some(produto(P_LEITE)),
    );

    assert!(resultado.is_err());
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM vendas"), 0);
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM venda_itens"), 0);
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM venda_pagamentos"), 0);
    // Só as duas entradas de compra do seed — nenhuma baixa de venda.
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM movimentacoes_estoque"), 2);
    assert_eq!(estoque(&db, P_CAFE), 10);
    assert_eq!(estoque(&db, P_LEITE), 5);
    // Só a abertura no livro.
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM livro_caixa"), 1);
    assert_eq!(saldo_sessao(&db, sessao), 10_000);
}

#[test]
fn pagamento_misto_gera_um_lancamento_por_forma() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);
    let sessao = abrir_caixa_de_teste(&mut db, 10_000);

    executar_venda(
        &mut db,
        cmd_venda(
            "20000000-0000-4000-8000-000000000004",
            sessao,
            vec![(produto(P_CAFE), 1), (produto(P_LEITE), 2)],
            vec![
                Pagamento::dinheiro(Money::from_centavos(1_000), Money::from_centavos(1_000))
                    .unwrap(),
                Pagamento::eletronico(FormaPagamento::Pix, Money::from_centavos(1_990)).unwrap(),
            ],
        ),
        None,
    )
    .unwrap();

    assert_eq!(
        contar(
            &db,
            "SELECT COUNT(*) FROM livro_caixa WHERE tipo = 'VENDA_DINHEIRO'"
        ),
        1
    );
    assert_eq!(
        contar(
            &db,
            "SELECT COUNT(*) FROM livro_caixa WHERE tipo = 'VENDA_PIX'"
        ),
        1
    );
    // Dinheiro não infla com o Pix.
    assert_eq!(saldo_sessao(&db, sessao), 11_000);
}

#[test]
fn estoque_insuficiente_bloqueia_antes_de_gravar() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);
    let sessao = abrir_caixa_de_teste(&mut db, 10_000);

    let resultado = executar_venda(
        &mut db,
        cmd_venda(
            "20000000-0000-4000-8000-000000000005",
            sessao,
            vec![(produto(P_LEITE), 99)],
            vec![
                Pagamento::dinheiro(Money::from_centavos(54_450), Money::from_centavos(54_450))
                    .unwrap(),
            ],
        ),
        None,
    );

    assert!(resultado.is_err());
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM vendas"), 0);
    assert_eq!(estoque(&db, P_LEITE), 5);
}

#[test]
fn fechamento_com_falta_persiste_diferenca_e_bloqueia_novas_vendas() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);
    let sessao = abrir_caixa_de_teste(&mut db, 10_000);

    executar_venda(
        &mut db,
        cmd_venda(
            "20000000-0000-4000-8000-000000000006",
            sessao,
            vec![(produto(P_CAFE), 1)],
            vec![
                Pagamento::dinheiro(Money::from_centavos(1_890), Money::from_centavos(2_000))
                    .unwrap(),
            ],
        ),
        None,
    )
    .unwrap();

    // Esperado: 100,00 + 18,90 = 118,90. Contado: 108,90 → falta 10,00.
    let tx = db.transacao().unwrap();
    let diferenca = {
        let sessoes = SessaoRepo::novo(&tx);
        let livro = LivroRepo::novo(&tx);
        let hash = Blake3HashChain;
        let clock = RelogioFixo(Utc::now());
        let repos = CaixaRepos {
            sessoes: &sessoes,
            livro: &livro,
            hash: &hash,
            clock: &clock,
        };
        fechar_caixa(
            &repos,
            FecharCaixaCmd {
                sessao_id: sessao,
                usuario_id: usuario(U_DONO),
                saldo_contado: Money::from_centavos(10_890),
            },
        )
        .unwrap()
    };
    tx.commit().unwrap();

    assert_eq!(diferenca.esperado.centavos(), 11_890);
    assert_eq!(diferenca.diferenca.centavos(), -1_000);
    assert!(diferenca.e_falta());

    let (estado, diff): (String, i64) = db
        .conn()
        .query_row(
            "SELECT estado, diferenca_centavos FROM sessoes_caixa WHERE id = ?1",
            [sessao.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(estado, "FECHADA");
    assert_eq!(diff, -1_000);

    let resultado = executar_venda(
        &mut db,
        cmd_venda(
            "20000000-0000-4000-8000-000000000007",
            sessao,
            vec![(produto(P_CAFE), 1)],
            vec![
                Pagamento::dinheiro(Money::from_centavos(1_890), Money::from_centavos(1_890))
                    .unwrap(),
            ],
        ),
        None,
    );
    assert!(resultado.is_err());
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM vendas"), 1);
}

#[test]
fn venda_em_outro_dispositivo_e_recusada() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);
    let sessao = abrir_caixa_de_teste(&mut db, 10_000);

    let mut cmd = cmd_venda(
        "20000000-0000-4000-8000-000000000008",
        sessao,
        vec![(produto(P_CAFE), 1)],
        vec![
            Pagamento::dinheiro(Money::from_centavos(1_890), Money::from_centavos(1_890)).unwrap(),
        ],
    );
    cmd.dispositivo_id = DispositivoId::new();

    let resultado = executar_venda(&mut db, cmd, None);
    assert!(matches!(resultado, Err(DomainError::PermissaoNegada(_))));
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM vendas"), 0);
}

#[test]
fn total_de_pagamentos_errado_e_recusado_pelo_dominio() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);
    let sessao = abrir_caixa_de_teste(&mut db, 10_000);

    let resultado = executar_venda(
        &mut db,
        cmd_venda(
            "20000000-0000-4000-8000-000000000009",
            sessao,
            vec![(produto(P_CAFE), 1)],
            vec![Pagamento::eletronico(FormaPagamento::Pix, Money::from_centavos(1_000)).unwrap()],
        ),
        None,
    );

    assert!(resultado.is_err());
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM vendas"), 0);
}

#[test]
fn produto_perecivel_sem_lote_e_recusado() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);
    db.conn()
        .execute("UPDATE produtos SET perecivel = 1 WHERE id = ?1", [P_CAFE])
        .unwrap();
    let sessao = abrir_caixa_de_teste(&mut db, 10_000);

    let resultado = executar_venda(
        &mut db,
        cmd_venda(
            "20000000-0000-4000-8000-00000000000a",
            sessao,
            vec![(produto(P_CAFE), 1)],
            vec![
                Pagamento::dinheiro(Money::from_centavos(1_890), Money::from_centavos(1_890))
                    .unwrap(),
            ],
        ),
        None,
    );

    assert!(resultado.is_err());
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM vendas"), 0);
}

#[test]
fn venda_nao_encontrada_depois_de_commit_continua_legivel() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);
    let sessao = abrir_caixa_de_teste(&mut db, 10_000);

    let venda = executar_venda(
        &mut db,
        cmd_venda(
            "20000000-0000-4000-8000-00000000000b",
            sessao,
            vec![(produto(P_CAFE), 2)],
            vec![
                Pagamento::dinheiro(Money::from_centavos(3_780), Money::from_centavos(4_000))
                    .unwrap(),
            ],
        ),
        None,
    )
    .unwrap();

    let tx = db.transacao().unwrap();
    let carregada = VendaRepo::novo(&tx)
        .buscar_por_id(venda.id)
        .unwrap()
        .expect("venda deve existir");
    assert_eq!(carregada.total.centavos(), 3_780);
    assert_eq!(carregada.itens.len(), 1);
    assert_eq!(carregada.itens[0].quantidade, 2);
    assert_eq!(carregada.pagamentos.len(), 1);
}

#[test]
fn abrir_duas_vezes_o_mesmo_caixa_e_idempotente() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);
    let sessao = SessaoId::new();

    for _ in 0..2 {
        let tx = db.transacao().unwrap();
        {
            let sessoes = SessaoRepo::novo(&tx);
            let livro = LivroRepo::novo(&tx);
            let hash = Blake3HashChain;
            let clock = RelogioFixo(Utc::now());
            let repos = CaixaRepos {
                sessoes: &sessoes,
                livro: &livro,
                hash: &hash,
                clock: &clock,
            };
            abrir_caixa(
                &repos,
                AbrirCaixaCmd {
                    sessao_id: sessao,
                    numero_caixa: 1,
                    dispositivo_id: dispositivo(D_CAIXA),
                    operador_id: usuario(U_OPERADOR),
                    saldo_inicial: Money::from_centavos(5_000),
                    fechamento_cego: false,
                },
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }

    assert_eq!(contar(&db, "SELECT COUNT(*) FROM sessoes_caixa"), 1);
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM livro_caixa"), 1);
    assert_eq!(saldo_sessao(&db, sessao), 5_000);
}

#[test]
fn venda_com_lote_respeita_saldo_do_lote() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);

    let lote = "30000000-0000-4000-8000-000000000001";
    db.conn()
        .execute_batch(&format!(
            r#"
            UPDATE produtos SET perecivel = 1 WHERE id = '{P_CAFE}';
            INSERT INTO lotes (id, produto_id, validade, custo_centavos)
                VALUES ('{lote}', '{P_CAFE}', date('now','localtime','+30 days'), 900);
            INSERT INTO movimentacoes_estoque
                (id, produto_id, lote_id, tipo, quantidade, custo_unitario_centavos, usuario_id)
                VALUES ('10000000-0000-4000-8000-000000000003',
                        '{P_CAFE}', '{lote}', 'ENTRADA_COMPRA', 3, 900, '{U_DONO}');
            "#
        ))
        .unwrap();

    let sessao = abrir_caixa_de_teste(&mut db, 10_000);
    let lote_id: LoteId = lote.parse().unwrap();

    let mut cmd = cmd_venda(
        "20000000-0000-4000-8000-00000000000c",
        sessao,
        vec![(produto(P_CAFE), 2)],
        vec![
            Pagamento::dinheiro(Money::from_centavos(3_780), Money::from_centavos(3_780)).unwrap(),
        ],
    );
    cmd.itens[0].lote_id = Some(lote_id);

    executar_venda(&mut db, cmd, None).unwrap();

    let saldo_lote: i64 = db
        .conn()
        .query_row(
            "SELECT saldo_atual FROM vw_estoque_lote WHERE lote_id = ?1",
            [lote],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(saldo_lote, 1);
    // 10 do seed + 3 do lote - 2 vendidos.
    assert_eq!(estoque(&db, P_CAFE), 11);

    // Vender mais 2 do mesmo lote (só resta 1) deve ser recusado.
    let mut cmd = cmd_venda(
        "20000000-0000-4000-8000-00000000000d",
        sessao,
        vec![(produto(P_CAFE), 2)],
        vec![
            Pagamento::dinheiro(Money::from_centavos(3_780), Money::from_centavos(3_780)).unwrap(),
        ],
    );
    cmd.itens[0].lote_id = Some(lote_id);

    let resultado = executar_venda(&mut db, cmd, None);
    assert!(resultado.is_err());
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM vendas"), 1);
}

#[test]
fn segunda_sessao_no_mesmo_caixa_e_recusada() {
    let mut db = Database::em_memoria().unwrap();
    semear(&db);
    abrir_caixa_de_teste(&mut db, 10_000);

    let tx = db.transacao().unwrap();
    let resultado = {
        let sessoes = SessaoRepo::novo(&tx);
        let livro = LivroRepo::novo(&tx);
        let hash = Blake3HashChain;
        let clock = RelogioFixo(Utc::now());
        let repos = CaixaRepos {
            sessoes: &sessoes,
            livro: &livro,
            hash: &hash,
            clock: &clock,
        };
        abrir_caixa(
            &repos,
            AbrirCaixaCmd {
                sessao_id: SessaoId::new(),
                numero_caixa: 1,
                dispositivo_id: dispositivo(D_CAIXA),
                operador_id: usuario(U_OPERADOR),
                saldo_inicial: Money::from_centavos(0),
                fechamento_cego: false,
            },
        )
    };
    drop(tx);

    assert!(matches!(resultado, Err(DomainError::EstadoInvalido(_))));
    assert_eq!(contar(&db, "SELECT COUNT(*) FROM sessoes_caixa"), 1);
}
