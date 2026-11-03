use thiserror::Error;

#[derive(Debug, Error)]
pub enum HardwareError {
    #[error("Erro de E/S na impressora: {0}")]
    Io(#[from] std::io::Error),

    #[error("Impressora não encontrada: {0}")]
    ImpressoraNaoEncontrada(String),

    #[error("Falha ao comunicar com spooler de impressão: {0}")]
    SpoolerFalha(String),

    #[error("Dados de impressão inválidos: {0}")]
    DadosInvalidos(String),
}
