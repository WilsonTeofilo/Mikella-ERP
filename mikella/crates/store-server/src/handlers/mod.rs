//! Handlers HTTP — camada de apresentação.
//!
//! Cada handler é uma função async que recebe `State<AppState>` (e opcionalmente
//! `Form`, `Json`, `Query` ou `Parts`) e retorna algo que implementa `IntoResponse`.
//!
//! Handlers públicos (sem auth): onboarding, login, 2FA.
//! Handlers protegidos: tudo sob middleware `exigir_token`.

pub mod auth;
pub mod cash;
pub mod config_pagamento;
pub mod csv;
pub mod customers;
pub mod donation;
pub mod finance;
pub mod health;
pub mod products;
pub mod receipt;
pub mod sales;
pub mod sales_write;
pub mod system;
pub mod users;

pub use auth::{
    auth_me, login, logout, onboarding_create, onboarding_status, solicitar_2fa, verificar_2fa,
};
pub use cash::{abrir_caixa_handler, caixa_status, fechar_caixa_handler, realizar_sangria_handler};
pub use config_pagamento::{atualizar_config_pagamento, obter_config_pagamento};
pub use csv::import_csv_handler;
pub use customers::{
    excluir_ou_anonimizar_cliente_handler, exportar_titular_handler, liquidar_divida_handler,
    listar_clientes_handler, listar_devedores_handler, salvar_cliente_handler,
};
pub use donation::{donation_ack, donation_prompt};
pub use finance::{
    liquidar_parcela_handler, liquidar_parcela_receber_handler, listar_contas_abertas_handler,
    obter_fluxo_caixa_handler, obter_resultado_dre_handler, obter_resumo_dashboard_handler,
    registrar_conta_pagar_handler, registrar_conta_receber_handler,
};
pub use health::health;
pub use products::{
    arquivar_produto_handler, atualizar_produto_handler, buscar_produto_handler,
    criar_produto_handler,
};
pub use receipt::{
    imprimir_comprovante, imprimir_comprovante_estorno, imprimir_comprovante_fechamento,
    obter_comprovante, obter_comprovante_estorno, obter_comprovante_fechamento,
};
pub use sales::{listar_produtos, livro_caixa, simular_parcelas, vendas_listar};
pub use sales_write::{
    autorizar_cancelamento_handler, cancelar_venda_handler, finalizar_venda_handler,
};
pub use system::{
    backup_local, consultar_cep, diagnostico, expurgo_lgpd_handler, listar_backups_handler,
    pacote_suporte_handler, teste_impressora_handler, verificar_backup_handler,
};
pub use users::{
    atualizar_senha_handler, atualizar_usuario_handler, buscar_usuario_handler,
    criar_usuario_handler, desativar_usuario_handler, gerenciar_capacidades_handler,
    listar_usuarios_handler,
};
