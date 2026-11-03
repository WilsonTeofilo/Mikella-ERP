use chrono::{DateTime, Utc};
use mikella_shared_kernel::money::Money;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LarguraPapel {
    /// 58mm: aproximadamente 32 colunas de caracteres padrão
    Colunas32,
    /// 80mm: aproximadamente 48 colunas de caracteres padrão
    Colunas48,
}

impl LarguraPapel {
    pub fn colunas(&self) -> usize {
        match self {
            Self::Colunas32 => 32,
            Self::Colunas48 => 48,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemComprovante {
    pub nome: String,
    pub quantidade: u32,
    pub preco_unitario: Money,
    pub subtotal: Money,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagamentoComprovante {
    pub forma: String,
    pub valor: Money,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprovanteVenda {
    pub loja_nome: String,
    pub loja_documento: Option<String>,
    pub venda_id: String,
    pub sessao_id: String,
    pub operador_nome: String,
    pub data_hora: DateTime<Utc>,
    pub itens: Vec<ItemComprovante>,
    pub subtotal: Money,
    pub desconto: Money,
    pub total: Money,
    pub pagamentos: Vec<PagamentoComprovante>,
    pub troco: Option<Money>,
    pub hash_verificacao: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprovanteFechamentoCaixa {
    pub loja_nome: String,
    pub loja_documento: Option<String>,
    pub sessao_id: String,
    pub numero_caixa: u8,
    pub operador_nome: String,
    pub aberta_em: DateTime<Utc>,
    pub fechada_em: DateTime<Utc>,
    pub saldo_inicial: Money,
    pub total_vendas_dinheiro: Money,
    pub total_vendas_eletronico: Money,
    pub total_suprimentos: Money,
    pub total_sangrias: Money,
    pub saldo_esperado_dinheiro: Money,
    pub saldo_informado_dinheiro: Money,
    pub diferenca: Money,
    pub quantidade_vendas: u32,
    pub quantidade_cancelamentos: u32,
    pub quantidade_estornos: u32,
    /// Observação do fechamento (até 512 caracteres), impressa no cupom.
    pub observacao: Option<String>,
    pub ultimo_hash: String,
}

pub struct EscPosGerador;

impl EscPosGerador {
    // Comandos de controle ESC/POS
    const ESC_INIT: &'static [u8] = b"\x1b@";
    const ESC_ALIGN_LEFT: &'static [u8] = b"\x1ba\x00";
    const ESC_ALIGN_CENTER: &'static [u8] = b"\x1ba\x01";
    #[allow(dead_code)]
    const ESC_ALIGN_RIGHT: &'static [u8] = b"\x1ba\x02";
    const ESC_BOLD_ON: &'static [u8] = b"\x1bE\x01";
    const ESC_BOLD_OFF: &'static [u8] = b"\x1bE\x00";
    const ESC_DOUBLE_HEIGHT_ON: &'static [u8] = b"\x1b!\x10";
    const ESC_DOUBLE_HEIGHT_OFF: &'static [u8] = b"\x1b!\x00";
    const ESC_CUT_FULL: &'static [u8] = b"\x1dV\x00";
    const ESC_DRAWER_PULSE: &'static [u8] = b"\x1bp\x00\x19\xfa";

    /// Gera o buffer de bytes binários ESC/POS pronto para impressora térmica.
    pub fn gerar_bytes(c: &ComprovanteVenda, largura: LarguraPapel, cortar: bool, abrir_gaveta: bool) -> Vec<u8> {
        let cols = largura.colunas();
        let sep = "-".repeat(cols);
        let mut buf = Vec::with_capacity(1024);

        buf.extend_from_slice(Self::ESC_INIT);

        if abrir_gaveta {
            buf.extend_from_slice(Self::ESC_DRAWER_PULSE);
        }

        // Cabeçalho Centralizado
        buf.extend_from_slice(Self::ESC_ALIGN_CENTER);
        buf.extend_from_slice(Self::ESC_BOLD_ON);
        buf.extend_from_slice(Self::ESC_DOUBLE_HEIGHT_ON);
        buf.extend_from_slice(c.loja_nome.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(Self::ESC_DOUBLE_HEIGHT_OFF);
        buf.extend_from_slice(Self::ESC_BOLD_OFF);

        if let Some(doc) = &c.loja_documento {
            buf.extend_from_slice(format!("CNPJ/CPF: {}\n", doc).as_bytes());
        }
        buf.extend_from_slice(b"COMPROVANTE DE VENDA\n");
        buf.extend_from_slice(format!("Data: {}\n", c.data_hora.format("%d/%m/%Y %H:%M:%S")).as_bytes());
        buf.extend_from_slice(format!("Venda: {}\n", &c.venda_id[..c.venda_id.len().min(18)]).as_bytes());
        buf.extend_from_slice(format!("Operador: {}\n", c.operador_nome).as_bytes());

        // Linha divisória
        buf.extend_from_slice(Self::ESC_ALIGN_LEFT);
        buf.extend_from_slice(sep.as_bytes());
        buf.push(b'\n');

        // Cabeçalho dos itens
        let cabecalho_itens = Self::formatar_linha_colunas("ITEM / QTD x UN", "SUBTOTAL", cols);
        buf.extend_from_slice(Self::ESC_BOLD_ON);
        buf.extend_from_slice(cabecalho_itens.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(Self::ESC_BOLD_OFF);
        buf.extend_from_slice(sep.as_bytes());
        buf.push(b'\n');

        // Itens
        for (i, item) in c.itens.iter().enumerate() {
            let num = i + 1;
            let nome_max = cols.saturating_sub(4);
            let nome_trunc = if item.nome.len() > nome_max {
                &item.nome[..nome_max]
            } else {
                &item.nome
            };
            buf.extend_from_slice(format!("{:02} {}\n", num, nome_trunc).as_bytes());

            let qtd_preco = format!("  {} un x {}", item.quantidade, item.preco_unitario);
            let subtotal = format!("{}", item.subtotal);
            let linha_sub = Self::formatar_linha_colunas(&qtd_preco, &subtotal, cols);
            buf.extend_from_slice(linha_sub.as_bytes());
            buf.push(b'\n');
        }

        buf.extend_from_slice(sep.as_bytes());
        buf.push(b'\n');

        // Totais
        buf.extend_from_slice(Self::formatar_linha_colunas("Subtotal:", &format!("{}", c.subtotal), cols).as_bytes());
        buf.push(b'\n');

        if c.desconto.centavos() > 0 {
            buf.extend_from_slice(Self::formatar_linha_colunas("Desconto:", &format!("-{}", c.desconto), cols).as_bytes());
            buf.push(b'\n');
        }

        buf.extend_from_slice(Self::ESC_BOLD_ON);
        buf.extend_from_slice(Self::formatar_linha_colunas("TOTAL:", &format!("{}", c.total), cols).as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(Self::ESC_BOLD_OFF);

        buf.extend_from_slice(sep.as_bytes());
        buf.push(b'\n');

        // Pagamentos
        buf.extend_from_slice(Self::ESC_BOLD_ON);
        buf.extend_from_slice(b"FORMA DE PAGAMENTO\n");
        buf.extend_from_slice(Self::ESC_BOLD_OFF);

        for p in &c.pagamentos {
            buf.extend_from_slice(Self::formatar_linha_colunas(&p.forma, &format!("{}", p.valor), cols).as_bytes());
            buf.push(b'\n');
        }

        if let Some(troco) = c.troco {
            if troco.centavos() > 0 {
                buf.extend_from_slice(Self::formatar_linha_colunas("Troco:", &format!("{}", troco), cols).as_bytes());
                buf.push(b'\n');
            }
        }

        buf.extend_from_slice(sep.as_bytes());
        buf.push(b'\n');

        // Rodapé centralizado
        buf.extend_from_slice(Self::ESC_ALIGN_CENTER);
        if let Some(hash) = &c.hash_verificacao {
            let hash_curto = &hash[..hash.len().min(16)];
            buf.extend_from_slice(format!("Cód. Verificação: {}\n", hash_curto).as_bytes());
        }
        buf.extend_from_slice(b"Obrigado pela preferencia!\n");
        buf.extend_from_slice(b"Mikella ERP - Autonomia Local\n\n\n\n");

        if cortar {
            buf.extend_from_slice(Self::ESC_CUT_FULL);
        }

        buf
    }

    /// Gera representação textual limpa (útil para pré-visualização na tela ou logs).
    pub fn gerar_texto_puro(c: &ComprovanteVenda, largura: LarguraPapel) -> String {
        let cols = largura.colunas();
        let sep = "-".repeat(cols);
        let mut out = String::with_capacity(512);

        out.push_str(&Self::centralizar(&c.loja_nome, cols));
        out.push('\n');
        if let Some(doc) = &c.loja_documento {
            out.push_str(&Self::centralizar(&format!("CNPJ/CPF: {}", doc), cols));
            out.push('\n');
        }
        out.push_str(&Self::centralizar("COMPROVANTE DE VENDA", cols));
        out.push('\n');
        out.push_str(&format!("Data: {}\n", c.data_hora.format("%d/%m/%Y %H:%M:%S")));
        out.push_str(&format!("Venda: {}\n", &c.venda_id[..c.venda_id.len().min(18)]));
        out.push_str(&format!("Operador: {}\n", c.operador_nome));
        out.push_str(&sep);
        out.push('\n');

        out.push_str(&Self::formatar_linha_colunas("ITEM / QTD x UN", "SUBTOTAL", cols));
        out.push('\n');
        out.push_str(&sep);
        out.push('\n');

        for (i, item) in c.itens.iter().enumerate() {
            let num = i + 1;
            let nome_max = cols.saturating_sub(4);
            let nome_trunc = if item.nome.len() > nome_max {
                &item.nome[..nome_max]
            } else {
                &item.nome
            };
            out.push_str(&format!("{:02} {}\n", num, nome_trunc));
            let qtd_preco = format!("  {} un x {}", item.quantidade, item.preco_unitario);
            let subtotal = format!("{}", item.subtotal);
            out.push_str(&Self::formatar_linha_colunas(&qtd_preco, &subtotal, cols));
            out.push('\n');
        }

        out.push_str(&sep);
        out.push('\n');
        out.push_str(&Self::formatar_linha_colunas("Subtotal:", &format!("{}", c.subtotal), cols));
        out.push('\n');
        if c.desconto.centavos() > 0 {
            out.push_str(&Self::formatar_linha_colunas("Desconto:", &format!("-{}", c.desconto), cols));
            out.push('\n');
        }
        out.push_str(&Self::formatar_linha_colunas("TOTAL:", &format!("{}", c.total), cols));
        out.push('\n');
        out.push_str(&sep);
        out.push('\n');

        out.push_str("FORMA DE PAGAMENTO\n");
        for p in &c.pagamentos {
            out.push_str(&Self::formatar_linha_colunas(&p.forma, &format!("{}", p.valor), cols));
            out.push('\n');
        }
        if let Some(troco) = c.troco {
            if troco.centavos() > 0 {
                out.push_str(&Self::formatar_linha_colunas("Troco:", &format!("{}", troco), cols));
                out.push('\n');
            }
        }
        out.push_str(&sep);
        out.push('\n');

        if let Some(hash) = &c.hash_verificacao {
            let hash_curto = &hash[..hash.len().min(16)];
            out.push_str(&Self::centralizar(&format!("Cód: {}", hash_curto), cols));
            out.push('\n');
        }
        out.push_str(&Self::centralizar("Obrigado pela preferencia!", cols));
        out.push('\n');
        out.push_str(&Self::centralizar("Mikella ERP", cols));
        out.push('\n');

        out
    }

    /// Gera o buffer de bytes binários ESC/POS para o Comprovante de Fechamento de Caixa.
    pub fn gerar_bytes_fechamento(c: &ComprovanteFechamentoCaixa, largura: LarguraPapel, cortar: bool) -> Vec<u8> {
        let cols = largura.colunas();
        let sep = "=".repeat(cols);
        let sep_fina = "-".repeat(cols);
        let mut buf = Vec::with_capacity(1024);

        buf.extend_from_slice(Self::ESC_INIT);
        buf.extend_from_slice(Self::ESC_ALIGN_CENTER);
        buf.extend_from_slice(Self::ESC_BOLD_ON);
        buf.extend_from_slice(Self::ESC_DOUBLE_HEIGHT_ON);
        buf.extend_from_slice(c.loja_nome.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(Self::ESC_DOUBLE_HEIGHT_OFF);
        buf.extend_from_slice(Self::ESC_BOLD_OFF);

        if let Some(doc) = &c.loja_documento {
            buf.extend_from_slice(format!("CNPJ/CPF: {}\n", doc).as_bytes());
        }
        buf.extend_from_slice(Self::ESC_BOLD_ON);
        buf.extend_from_slice(b"FECHAMENTO DE CAIXA\n");
        buf.extend_from_slice(Self::ESC_BOLD_OFF);
        buf.extend_from_slice(format!("Caixa Nº: {:02}\n", c.numero_caixa).as_bytes());
        buf.extend_from_slice(format!("Operador: {}\n", c.operador_nome).as_bytes());
        buf.extend_from_slice(format!("Abertura: {}\n", c.aberta_em.format("%d/%m/%Y %H:%M")).as_bytes());
        buf.extend_from_slice(format!("Fechamento: {}\n", c.fechada_em.format("%d/%m/%Y %H:%M")).as_bytes());

        buf.extend_from_slice(Self::ESC_ALIGN_LEFT);
        buf.extend_from_slice(sep.as_bytes());
        buf.push(b'\n');

        buf.extend_from_slice(Self::formatar_linha_colunas("Saldo Inicial:", &format!("{}", c.saldo_inicial), cols).as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(Self::formatar_linha_colunas("(+) Vendas Dinheiro:", &format!("{}", c.total_vendas_dinheiro), cols).as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(Self::formatar_linha_colunas("(+) Suprimentos:", &format!("{}", c.total_suprimentos), cols).as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(Self::formatar_linha_colunas("(-) Sangrias:", &format!("-{}", c.total_sangrias), cols).as_bytes());
        buf.push(b'\n');

        buf.extend_from_slice(sep_fina.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(Self::ESC_BOLD_ON);
        buf.extend_from_slice(Self::formatar_linha_colunas("SALDO ESPERADO:", &format!("{}", c.saldo_esperado_dinheiro), cols).as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(Self::formatar_linha_colunas("SALDO CONTADO:", &format!("{}", c.saldo_informado_dinheiro), cols).as_bytes());
        buf.push(b'\n');

        let dif_str = if c.diferenca.centavos() == 0 {
            "R$ 0,00 (CONFERE)".to_string()
        } else if c.diferenca.centavos() > 0 {
            format!("+{} (SOBRA)", c.diferenca)
        } else {
            format!("{} (FALTA)", c.diferenca)
        };
        buf.extend_from_slice(Self::formatar_linha_colunas("DIFERENÇA:", &dif_str, cols).as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(Self::ESC_BOLD_OFF);

        buf.extend_from_slice(sep_fina.as_bytes());
        buf.push(b'\n');
        buf.extend_from_slice(Self::formatar_linha_colunas("Vendas Cartão/Pix:", &format!("{}", c.total_vendas_eletronico), cols).as_bytes());
        buf.push(b'\n');

        buf.extend_from_slice(sep.as_bytes());
        buf.push(b'\n');

        // Assinaturas para conferência
        buf.extend_from_slice(Self::ESC_ALIGN_CENTER);
        buf.extend_from_slice(b"\n_______________________________\nOperador\n\n");
        buf.extend_from_slice(b"_______________________________\nGerente / Responsavel\n\n");

        let hash_curto = &c.ultimo_hash[..c.ultimo_hash.len().min(16)];
        buf.extend_from_slice(format!("Integridade: {}\n", hash_curto).as_bytes());
        buf.extend_from_slice(b"Mikella ERP - Fechamento Seguro\n\n\n\n");

        if cortar {
            buf.extend_from_slice(Self::ESC_CUT_FULL);
        }

        buf
    }

    /// Gera representação textual do comprovante de fechamento.
    pub fn gerar_texto_fechamento(c: &ComprovanteFechamentoCaixa, largura: LarguraPapel) -> String {
        let cols = largura.colunas();
        let sep = "=".repeat(cols);
        let sep_fina = "-".repeat(cols);
        let mut out = String::with_capacity(512);

        out.push_str(&Self::centralizar(&c.loja_nome, cols));
        out.push('\n');
        if let Some(doc) = &c.loja_documento {
            out.push_str(&Self::centralizar(&format!("CNPJ/CPF: {}", doc), cols));
            out.push('\n');
        }
        out.push_str(&Self::centralizar("FECHAMENTO DE CAIXA", cols));
        out.push('\n');
        out.push_str(&format!("Caixa: {:02} | Operador: {}\n", c.numero_caixa, c.operador_nome));
        out.push_str(&format!("Abertura: {}\n", c.aberta_em.format("%d/%m/%Y %H:%M")));
        out.push_str(&format!("Fechamento: {}\n", c.fechada_em.format("%d/%m/%Y %H:%M")));
        out.push_str(&sep);
        out.push('\n');

        out.push_str(&Self::formatar_linha_colunas("Vendas:", &c.quantidade_vendas.to_string(), cols));
        out.push('\n');
        out.push_str(&Self::formatar_linha_colunas("Cancelamentos:", &c.quantidade_cancelamentos.to_string(), cols));
        out.push('\n');
        out.push_str(&Self::formatar_linha_colunas("Estornos:", &c.quantidade_estornos.to_string(), cols));
        out.push('\n');
        out.push_str(&sep_fina);
        out.push('\n');

        out.push_str(&Self::formatar_linha_colunas("Saldo Inicial:", &format!("{}", c.saldo_inicial), cols));
        out.push('\n');
        out.push_str(&Self::formatar_linha_colunas("(+) Vendas Dinheiro:", &format!("{}", c.total_vendas_dinheiro), cols));
        out.push('\n');
        out.push_str(&Self::formatar_linha_colunas("(+) Suprimentos:", &format!("{}", c.total_suprimentos), cols));
        out.push('\n');
        out.push_str(&Self::formatar_linha_colunas("(-) Sangrias:", &format!("-{}", c.total_sangrias), cols));
        out.push('\n');
        out.push_str(&sep_fina);
        out.push('\n');

        out.push_str(&Self::formatar_linha_colunas("SALDO ESPERADO:", &format!("{}", c.saldo_esperado_dinheiro), cols));
        out.push('\n');
        out.push_str(&Self::formatar_linha_colunas("SALDO CONTADO:", &format!("{}", c.saldo_informado_dinheiro), cols));
        out.push('\n');

        let dif_str = if c.diferenca.centavos() == 0 {
            "R$ 0,00 (CONFERE)".to_string()
        } else if c.diferenca.centavos() > 0 {
            format!("+{} (SOBRA)", c.diferenca)
        } else {
            format!("{} (FALTA)", c.diferenca)
        };
        out.push_str(&Self::formatar_linha_colunas("DIFERENÇA:", &dif_str, cols));
        out.push('\n');
        out.push_str(&sep_fina);
        out.push('\n');

        out.push_str(&Self::formatar_linha_colunas("Vendas Cartão/Pix:", &format!("{}", c.total_vendas_eletronico), cols));
        out.push('\n');
        out.push_str(&sep);
        out.push('\n');

        if let Some(obs) = c.observacao.as_deref().map(str::trim).filter(|o| !o.is_empty()) {
            out.push_str("Observacao:\n");
            for linha in Self::quebrar_texto(obs, cols) {
                out.push_str(&linha);
                out.push('\n');
            }
            out.push_str(&sep_fina);
            out.push('\n');
        }

        out.push_str("\n_______________________________\n");
        out.push_str(&format!("{}\n", c.operador_nome));
        out.push_str("Operador\n\n");
        out.push_str("_______________________________\n");
        out.push_str("Gerente / Responsavel\n\n");

        let hash_curto = &c.ultimo_hash[..c.ultimo_hash.len().min(16)];
        out.push_str(&Self::centralizar(&format!("Integridade: {}", hash_curto), cols));
        out.push('\n');
        out.push_str(&Self::centralizar("Mikella ERP", cols));
        out.push('\n');

        out
    }

    /// Quebra um texto em linhas de até `largura` caracteres, respeitando
    /// palavras e contando caracteres (não bytes) para não cortar acentos.
    fn quebrar_texto(texto: &str, largura: usize) -> Vec<String> {
        let mut linhas = Vec::new();
        let mut atual = String::new();
        for palavra in texto.split_whitespace() {
            let extra = if atual.is_empty() { 0 } else { 1 };
            if atual.chars().count() + extra + palavra.chars().count() > largura && !atual.is_empty() {
                linhas.push(atual);
                atual = String::new();
            }
            if !atual.is_empty() {
                atual.push(' ');
            }
            atual.push_str(palavra);
        }
        if !atual.is_empty() {
            linhas.push(atual);
        }
        linhas
    }

    fn centralizar(texto: &str, largura: usize) -> String {
        let total = texto.chars().count();
        if total >= largura {
            return texto.chars().take(largura).collect();
        }
        let espacos = (largura - total) / 2;
        format!("{}{}", " ".repeat(espacos), texto)
    }

    fn formatar_linha_colunas(esq: &str, dir: &str, largura: usize) -> String {
        let total_chars = esq.chars().count() + dir.chars().count();
        if total_chars >= largura {
            return format!("{} {}", esq, dir);
        }
        let espacos = largura - total_chars;
        format!("{}{}{}", esq, " ".repeat(espacos), dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gera_texto_puro_58mm_correto() {
        let comp = ComprovanteVenda {
            loja_nome: "Mercadinho São José".into(),
            loja_documento: Some("12.345.678/0001-90".into()),
            venda_id: "venda-12345".into(),
            sessao_id: "sessao-1".into(),
            operador_nome: "Maria".into(),
            data_hora: Utc::now(),
            itens: vec![
                ItemComprovante {
                    nome: "Cafe Torrado 500g".into(),
                    quantidade: 2,
                    preco_unitario: Money::from_centavos(1500),
                    subtotal: Money::from_centavos(3000),
                },
                ItemComprovante {
                    nome: "Acucar 1kg".into(),
                    quantidade: 1,
                    preco_unitario: Money::from_centavos(450),
                    subtotal: Money::from_centavos(450),
                },
            ],
            subtotal: Money::from_centavos(3450),
            desconto: Money::from_centavos(200),
            total: Money::from_centavos(3250),
            pagamentos: vec![
                PagamentoComprovante {
                    forma: "PIX".into(),
                    valor: Money::from_centavos(3250),
                }
            ],
            troco: None,
            hash_verificacao: Some("b1a4e8f9901234567890".into()),
        };

        let texto = EscPosGerador::gerar_texto_puro(&comp, LarguraPapel::Colunas32);
        assert!(texto.contains("Mercadinho São José"));
        assert!(texto.contains("Cafe Torrado"));
        assert!(texto.contains("TOTAL:"));
        assert!(texto.contains("PIX"));

        let bytes = EscPosGerador::gerar_bytes(&comp, LarguraPapel::Colunas32, true, false);
        assert!(!bytes.is_empty());
        assert!(bytes.starts_with(EscPosGerador::ESC_INIT));
        assert!(bytes.ends_with(EscPosGerador::ESC_CUT_FULL));
    }

    #[test]
    fn gera_comprovante_fechamento_correto() {
        let fechamento = ComprovanteFechamentoCaixa {
            loja_nome: "Mercadinho São José".into(),
            loja_documento: Some("12.345.678/0001-90".into()),
            sessao_id: "sessao-99".into(),
            numero_caixa: 1,
            operador_nome: "Maria".into(),
            aberta_em: Utc::now(),
            fechada_em: Utc::now(),
            saldo_inicial: Money::from_centavos(10000), // R$ 100,00
            total_vendas_dinheiro: Money::from_centavos(35000), // R$ 350,00
            total_vendas_eletronico: Money::from_centavos(50000), // R$ 500,00
            total_suprimentos: Money::ZERO,
            total_sangrias: Money::from_centavos(5000), // R$ 50,00
            saldo_esperado_dinheiro: Money::from_centavos(40000), // R$ 400,00
            saldo_informado_dinheiro: Money::from_centavos(40000), // R$ 400,00
            diferenca: Money::ZERO,
            quantidade_vendas: 12,
            quantidade_cancelamentos: 1,
            quantidade_estornos: 0,
            observacao: Some("Conferido com o gerente; sem ocorrências.".into()),
            ultimo_hash: "blake3_hash_genesis_abc123".into(),
        };

        let texto = EscPosGerador::gerar_texto_fechamento(&fechamento, LarguraPapel::Colunas48);
        assert!(texto.contains("FECHAMENTO DE CAIXA"));
        assert!(texto.contains("SALDO ESPERADO:"));
        assert!(texto.contains("SALDO CONTADO:"));
        assert!(texto.contains("CONFERE"));
        assert!(texto.contains("Cancelamentos:"));
        assert!(texto.contains("Conferido com o gerente"));
        assert!(texto.contains("Operador"));
        assert!(texto.contains("Gerente"));

        let bytes = EscPosGerador::gerar_bytes_fechamento(&fechamento, LarguraPapel::Colunas48, true);
        assert!(!bytes.is_empty());
        assert!(bytes.starts_with(EscPosGerador::ESC_INIT));
        assert!(bytes.ends_with(EscPosGerador::ESC_CUT_FULL));
    }
}
