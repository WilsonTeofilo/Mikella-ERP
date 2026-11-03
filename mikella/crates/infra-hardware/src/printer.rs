use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::error::HardwareError;

/// Porta de comunicação com a impressora térmica física ou emulada.
pub trait PrinterPort: Send + Sync {
    /// Envia uma sequência bruta de bytes para a impressora.
    fn imprimir(&self, raw_bytes: &[u8]) -> Result<(), HardwareError>;

    /// Envia comando de pulso para abertura da gaveta de dinheiro.
    fn abrir_gaveta(&self) -> Result<(), HardwareError>;
}

/// Implementação mock em memória para testes automatizados.
#[derive(Debug, Default, Clone)]
pub struct MockPrinter {
    buffers: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl MockPrinter {
    pub fn novo() -> Self {
        Self::default()
    }

    pub fn total_impressoes(&self) -> usize {
        self.buffers.lock().unwrap().len()
    }

    pub fn ultimos_bytes(&self) -> Option<Vec<u8>> {
        self.buffers.lock().unwrap().last().cloned()
    }
}

impl PrinterPort for MockPrinter {
    fn imprimir(&self, raw_bytes: &[u8]) -> Result<(), HardwareError> {
        self.buffers.lock().unwrap().push(raw_bytes.to_vec());
        Ok(())
    }

    fn abrir_gaveta(&self) -> Result<(), HardwareError> {
        self.imprimir(b"\x1bp\x00\x19\xfa")
    }
}

/// Implementação que grava os bytes em um arquivo (útil para desenvolvimento/depuração ou spool de arquivo).
pub struct ArquivoPrinter {
    caminho: PathBuf,
}

impl ArquivoPrinter {
    pub fn novo(caminho: impl Into<PathBuf>) -> Self {
        Self {
            caminho: caminho.into(),
        }
    }
}

impl PrinterPort for ArquivoPrinter {
    fn imprimir(&self, raw_bytes: &[u8]) -> Result<(), HardwareError> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.caminho)?;
        file.write_all(raw_bytes)?;
        file.flush()?;
        Ok(())
    }

    fn abrir_gaveta(&self) -> Result<(), HardwareError> {
        self.imprimir(b"\x1bp\x00\x19\xfa")
    }
}

/// Impressora padrão do sistema (dispositivo de porta USB/Serial ou Spooler local).
pub struct DispositivoLocalPrinter {
    identificador: String,
}

impl DispositivoLocalPrinter {
    pub fn novo(identificador: impl Into<String>) -> Self {
        Self {
            identificador: identificador.into(),
        }
    }
}

impl PrinterPort for DispositivoLocalPrinter {
    fn imprimir(&self, raw_bytes: &[u8]) -> Result<(), HardwareError> {
        #[cfg(target_os = "linux")]
        {
            // Em distribuições Linux leves, impressoras térmicas conectadas por USB aparecem em /dev/usb/lp0..3
            let caminho = if self.identificador.starts_with("/dev/") {
                &self.identificador
            } else {
                "/dev/usb/lp0"
            };

            let mut file = OpenOptions::new()
                .write(true)
                .open(caminho)
                .map_err(|e| HardwareError::ImpressoraNaoEncontrada(format!("{}: {}", caminho, e)))?;
            file.write_all(raw_bytes)?;
            file.flush()?;
            Ok(())
        }

        #[cfg(target_os = "windows")]
        {
            // No Windows, pode ser uma porta direta (COM1, LPT1) ou salva para spooler
            if self.identificador.to_uppercase().starts_with("COM") || self.identificador.to_uppercase().starts_with("LPT") {
                let mut file = OpenOptions::new()
                    .write(true)
                    .open(&self.identificador)
                    .map_err(|e| HardwareError::ImpressoraNaoEncontrada(format!("{}: {}", self.identificador, e)))?;
                file.write_all(raw_bytes)?;
                file.flush()?;
                Ok(())
            } else {
                // Para impressoras mapeadas por nome, cria o arquivo temporário de spool RAW
                let temp_dir = std::env::temp_dir();
                let temp_file = temp_dir.join("mikella_cupom.raw");
                let mut file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&temp_file)?;
                file.write_all(raw_bytes)?;
                file.flush()?;
                tracing::info!(impressora = %self.identificador, arquivo = ?temp_file, "Spool de impressão térmica gerado");
                Ok(())
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = raw_bytes;
            Ok(())
        }
    }

    fn abrir_gaveta(&self) -> Result<(), HardwareError> {
        self.imprimir(b"\x1bp\x00\x19\xfa")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_printer_registra_impressoes() {
        let printer = MockPrinter::novo();
        assert_eq!(printer.total_impressoes(), 0);

        printer.imprimir(b"TESTE").unwrap();
        assert_eq!(printer.total_impressoes(), 1);
        assert_eq!(printer.ultimos_bytes().unwrap(), b"TESTE");

        printer.abrir_gaveta().unwrap();
        assert_eq!(printer.total_impressoes(), 2);
    }
}
