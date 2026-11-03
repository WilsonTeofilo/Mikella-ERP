//! Handlers HTTP — camada de apresentação.
//!
//! Cada handler é uma função async que recebe `State<AppState>` (e opcionalmente
//! `Form`, `Json`, `Query` ou `Parts`) e retorna algo que implementa `IntoResponse`.
//!
//! Handlers públicos (sem auth): onboarding, login, 2FA.
//! Handlers protegidos: tudo sob middleware `exigir_token`.

pub mod auth;
pub mod cash;
pub mod csv;
pub mod donation;
pub mod finance;
pub mod health;
pub mod products;
pub mod receipt;
pub mod sales;
pub mod sales_write;
pub mod system;
pub mod users;

pub use auth::{auth_me, login, onboarding_create, onboarding_status, solicitar_2fa, verificar_2fa};
pub use cash::{abrir_caixa_handler, caixa_status, fechar_caixa_handler};
pub use csv::import_csv_handler;
pub use donation::{donation_ack, donation_prompt};
pub use finance::{
    listar_contas_abertas_handler, liquidar_parcela_handler, liquidar_parcela_receber_handler,
    registrar_conta_pagar_handler, registrar_conta_receber_handler,
};
pub use health::health;
pub use products::{
    arquivar_produto_handler, atualizar_produto_handler, buscar_produto_handler,
    criar_produto_handler,
};
pub use receipt::{
    imprimir_comprovante, imprimir_comprovante_fechamento, obter_comprovante,
    obter_comprovante_fechamento,
};
pub use sales::{listar_produtos, livro_caixa, vendas_listar};
pub use sales_write::finalizar_venda_handler;
pub use system::{backup_local, diagnostico};
pub use users::{
    atualizar_senha_handler, atualizar_usuario_handler, buscar_usuario_handler,
    criar_usuario_handler, desativar_usuario_handler, gerenciar_capacidades_handler,
    listar_usuarios_handler,
};
