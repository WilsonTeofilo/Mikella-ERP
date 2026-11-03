//! Data Transfer Objects — formulários e respostas da API.
//!
//! Os structs aqui definidos são apenas para serialização/deserialização JSON
//! e form-urlencoded. Não contêm lógica de domínio.

pub mod auth;
pub mod cash;
pub mod finance;
pub mod products;
pub mod response;
pub mod sales;
pub mod two_factor;
pub mod users;

pub use auth::{LoginForm, OnboardingForm, OnboardingStatus};
pub use cash::{AbrirCaixaForm, FecharCaixaForm, SangriaForm};
pub use finance::{
    ListarContasQuery, ParcelaForm, RegistrarContaPagarForm, RegistrarContaReceberForm,
};
pub use products::{AtualizarProdutoForm, CriarProdutoForm};
pub use response::{HealthResponse, ProdutoResponse, VendaSummary};
pub use sales::{FinalizarVendaForm, LivroCaixaQuery, VendasQuery};
pub use two_factor::{DoisFARequestForm, DoisFAVerifyForm};
pub use users::{AtualizarSenhaForm, AtualizarUsuarioForm, CapacidadesForm, CriarUsuarioForm};
