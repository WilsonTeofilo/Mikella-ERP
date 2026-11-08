//! mikella-infra-hardware
//! ESC/POS, geração de comprovantes de venda, impressora térmica e abertura de gaveta.

pub mod error;
pub mod receipt;
pub mod printer;

pub use error::HardwareError;
pub use receipt::{
    formatar_data_no_fuso, ComprovanteEstorno, ComprovanteFechamentoCaixa, ComprovanteVenda,
    EscPosGerador, ItemComprovante, LarguraPapel, PagamentoComprovante,
};
pub use printer::{PrinterPort, MockPrinter, ArquivoPrinter, DispositivoLocalPrinter};
