//! mikella-infra-hardware
//! ESC/POS, geração de comprovantes de venda, impressora térmica e abertura de gaveta.

pub mod error;
pub mod printer;
pub mod receipt;

pub use error::HardwareError;
pub use printer::{ArquivoPrinter, DispositivoLocalPrinter, MockPrinter, PrinterPort};
pub use receipt::{
    formatar_data_no_fuso, ComprovanteEstorno, ComprovanteFechamentoCaixa, ComprovanteVenda,
    EscPosGerador, ItemComprovante, LarguraPapel, PagamentoComprovante,
};
