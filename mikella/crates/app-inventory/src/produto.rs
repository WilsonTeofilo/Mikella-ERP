//! Casos de uso de cadastro de produtos.
//!
//! Regras de negócio do domínio:
//! - Nome não pode ser vazio.
//! - Preço não pode ser negativo.
//! - código_barras e SKU são únicos por loja (validação no repositório/DB).
//! - Arquivamento é permanente e irreversível (D8): nunca deleta fisicamente.
//! - Arquivamento não devolve cota mensal.

use chrono::Utc;

use mikella_domain_inventory::ports::ProdutoRepository;
use mikella_domain_inventory::produto::{Produto, Unidade};
use mikella_shared_kernel::error::{DomainError, DomainResult};
use mikella_shared_kernel::ids::{CategoriaId, ProdutoId};
use mikella_shared_kernel::money::Money;

/// Portas necessárias para os casos de uso de produto.
pub struct ProdutoRepos<'a> {
    pub produtos: &'a dyn ProdutoRepository,
}

/// Comando para criação de produto.
#[derive(Debug)]
pub struct CriarProdutoCmd {
    pub produto_id: ProdutoId,
    pub nome: String,
    pub preco: Money,
    pub codigo_barras: Option<String>,
    pub sku: Option<String>,
    pub categoria_id: Option<CategoriaId>,
    pub unidade: Unidade,
    pub custo: Money,
    pub estoque_minimo: i32,
    pub perecivel: bool,
    pub alerta_validade_dias: u32,
}

/// Cria um novo produto validando unicidade de código_barras e SKU.
///
/// Idempotency parcial: se `produto_id` já existe, retorna erro. O cliente
/// deve gerar um UUID v4 antes do envio para suportar reenvio com garantia.
pub fn criar_produto(repos: &ProdutoRepos<'_>, cmd: CriarProdutoCmd) -> DomainResult<Produto> {
    // Unicidade de código_barras
    if let Some(cb) = &cmd.codigo_barras {
        if repos.produtos.buscar_por_codigo_barras(cb)?.is_some() {
            return Err(DomainError::EstadoInvalido(format!(
                "Código de barras '{}' já cadastrado",
                cb
            )));
        }
    }

    // Unicidade de SKU
    if let Some(sku) = &cmd.sku {
        if repos.produtos.buscar_por_sku(sku)?.is_some() {
            return Err(DomainError::EstadoInvalido(format!(
                "SKU '{}' já cadastrado",
                sku
            )));
        }
    }

    let preco_validado = cmd
        .preco
        .assert_nao_negativo()
        .map_err(|_| DomainError::Invalido("Preço não pode ser negativo".into()))?;

    let mut produto = Produto::novo(cmd.produto_id, cmd.nome, preco_validado)?;
    produto = produto
        .com_codigo_barras(cmd.codigo_barras)
        .com_sku(cmd.sku)
        .com_categoria(cmd.categoria_id)
        .com_unidade(cmd.unidade)
        .com_custo(cmd.custo)
        .com_estoque_minimo(cmd.estoque_minimo)
        .com_perecivel(cmd.perecivel, cmd.alerta_validade_dias);

    repos.produtos.salvar(&produto)?;

    tracing::info!(
        produto_id = %produto.id,
        nome = %produto.nome,
        "produto criado"
    );

    Ok(produto)
}

/// Comando para atualização completa de produto.
#[derive(Debug)]
pub struct AtualizarProdutoCmd {
    pub produto_id: ProdutoId,
    pub codigo_barras: Option<String>,
    pub sku: Option<String>,
    pub nome: String,
    pub categoria_id: Option<CategoriaId>,
    pub unidade: Unidade,
    pub preco: Money,
    pub custo: Money,
    pub estoque_minimo: i32,
    pub perecivel: bool,
    pub alerta_validade_dias: u32,
}

/// Atualiza todos os campos mutáveis de um produto.
/// Retorna erro se o produto não existir ou estiver arquivado.
pub fn atualizar_produto(
    repos: &ProdutoRepos<'_>,
    cmd: AtualizarProdutoCmd,
) -> DomainResult<Produto> {
    let mut produto = repos
        .produtos
        .buscar_por_id(cmd.produto_id)?
        .ok_or_else(|| DomainError::NaoEncontrado {
            entidade: "Produto",
            id: cmd.produto_id.to_string(),
        })?;

    if produto.arquivado {
        return Err(DomainError::EstadoInvalido(format!(
            "Produto {} está arquivado — não pode ser editado",
            produto.id
        )));
    }

    // Unicidade de código_barras (exceto self)
    if let Some(cb) = &cmd.codigo_barras {
        if let Some(existente) = repos.produtos.buscar_por_codigo_barras(cb)? {
            if existente.id != produto.id {
                return Err(DomainError::EstadoInvalido(format!(
                    "Código de barras '{}' já cadastrado para outro produto",
                    cb
                )));
            }
        }
    }

    // Unicidade de SKU (exceto self)
    if let Some(sku) = &cmd.sku {
        if let Some(existente) = repos.produtos.buscar_por_sku(sku)? {
            if existente.id != produto.id {
                return Err(DomainError::EstadoInvalido(format!(
                    "SKU '{}' já cadastrado para outro produto",
                    sku
                )));
            }
        }
    }

    produto.nome = cmd.nome.trim().to_string();
    produto.codigo_barras = cmd.codigo_barras;
    produto.sku = cmd.sku;
    produto.categoria_id = cmd.categoria_id;
    produto.unidade = cmd.unidade;
    produto.custo_centavos = cmd.custo;
    produto.estoque_minimo = cmd.estoque_minimo;
    produto.perecivel = cmd.perecivel;
    produto.alerta_validade_dias = cmd.alerta_validade_dias;

    // Atualiza preço (valida não-negativo)
    produto.atualizar_preco_retorna_anterior(cmd.preco)?;

    repos.produtos.atualizar(&produto)?;

    tracing::info!(
        produto_id = %produto.id,
        nome = %produto.nome,
        "produto atualizado"
    );

    Ok(produto)
}

/// Arquiva (soft-delete) um produto. Nunca deleta fisicamente (D8).
/// Marca `arquivado = true` e registra `arquivado_em`.
pub fn arquivar_produto(repos: &ProdutoRepos<'_>, produto_id: ProdutoId) -> DomainResult<()> {
    let mut produto =
        repos
            .produtos
            .buscar_por_id(produto_id)?
            .ok_or_else(|| DomainError::NaoEncontrado {
                entidade: "Produto",
                id: produto_id.to_string(),
            })?;

    produto.arquivar(Utc::now())?;
    repos.produtos.atualizar(&produto)?;

    tracing::info!(produto_id = %produto_id, "produto arquivado");
    Ok(())
}
