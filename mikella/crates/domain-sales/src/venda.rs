use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use mikella_shared_kernel::{
    error::{DomainError, DomainResult},
    ids::{DispositivoId, SessaoId, UsuarioId, VendaId},
    money::Money,
};

use crate::item::ItemVenda;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FormaPagamento {
    Dinheiro,
    Debito,
    Credito,
    Pix,
    Voucher,
    Crediario,
    Misto,
    Outro,
}

impl FormaPagamento {
    /// Valor gravado na coluna `forma` do banco.
    ///
    /// O schema histórico aceita apenas DINHEIRO/DEBITO/CREDITO/PIX/OUTRO.
    /// Voucher e crediário são operacionalmente distintos, mas são gravados
    /// como OUTRO e identificados em `forma_detalhe` (migração 0006).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dinheiro => "DINHEIRO",
            Self::Debito => "DEBITO",
            Self::Credito => "CREDITO",
            Self::Pix => "PIX",
            Self::Voucher | Self::Crediario | Self::Outro => "OUTRO",
            Self::Misto => "MISTO",
        }
    }

    /// Detalhe opcional que desfaz a ambiguidade de OUTRO.
    pub fn as_detalhe(&self) -> Option<&'static str> {
        match self {
            Self::Voucher => Some("VOUCHER"),
            Self::Crediario => Some("CREDIARIO"),
            _ => None,
        }
    }

    pub fn from_db(valor: &str) -> DomainResult<Self> {
        Self::from_db_com_detalhe(valor, None)
    }

    /// Reconstrói a forma a partir do valor canônico + detalhe da migração 0006.
    pub fn from_db_com_detalhe(valor: &str, detalhe: Option<&str>) -> DomainResult<Self> {
        match (valor, detalhe.map(str::to_ascii_uppercase).as_deref()) {
            ("OUTRO", Some("VOUCHER")) => Ok(Self::Voucher),
            ("OUTRO", Some("CREDIARIO")) => Ok(Self::Crediario),
            _ => match valor {
                "DINHEIRO" => Ok(Self::Dinheiro),
                "DEBITO" => Ok(Self::Debito),
                "CREDITO" => Ok(Self::Credito),
                "PIX" => Ok(Self::Pix),
                "MISTO" => Ok(Self::Misto),
                "OUTRO" => Ok(Self::Outro),
                outro => Err(DomainError::Invalido(format!(
                    "Forma de pagamento desconhecida no banco: {outro}"
                ))),
            },
        }
    }

    /// `Misto` não é uma forma concreta: é o resultado de dois ou mais
    /// pagamentos. Nunca deve ser usada como um pagamento individual.
    pub fn is_concreta(&self) -> bool {
        !matches!(self, Self::Misto)
    }

    pub fn movimenta_dinheiro_fisico(&self) -> bool {
        matches!(self, Self::Dinheiro)
    }

    /// Crediário não entra no caixa: gera uma conta a receber em aberto.
    pub fn e_crediario(&self) -> bool {
        matches!(self, Self::Crediario)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EstadoVenda {
    Finalizada,
    Cancelada,
    Estornada,
}

impl EstadoVenda {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Finalizada => "FINALIZADA",
            Self::Cancelada => "CANCELADA",
            Self::Estornada => "ESTORNADA",
        }
    }

    pub fn from_db(valor: &str) -> DomainResult<Self> {
        match valor {
            "FINALIZADA" => Ok(Self::Finalizada),
            "CANCELADA" => Ok(Self::Cancelada),
            "ESTORNADA" => Ok(Self::Estornada),
            outro => Err(DomainError::Invalido(format!(
                "Estado de venda desconhecido no banco: {outro}"
            ))),
        }
    }
}

/// Um pagamento aplicado a uma venda.
///
/// `valor` é quanto este pagamento abate do total. Em dinheiro, `recebido`
/// é o valor entregue pelo cliente (para calcular o troco); nas demais
/// formas, `recebido` é `None` e o valor cobrado é o próprio `valor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pagamento {
    pub forma: FormaPagamento,
    pub valor: Money,
    pub recebido: Option<Money>,
}

impl Pagamento {
    /// Pagamento em dinheiro. `recebido` deve cobrir `valor`.
    pub fn dinheiro(valor: Money, recebido: Money) -> DomainResult<Self> {
        valor
            .assert_nao_negativo()
            .and_then(|_| recebido.assert_nao_negativo())
            .map_err(|_| {
                DomainError::Invalido("Valores de pagamento não podem ser negativos".into())
            })?;
        if valor.is_zero() {
            return Err(DomainError::Invalido("Pagamento não pode ser zero".into()));
        }
        if recebido < valor {
            return Err(DomainError::Invalido(format!(
                "Valor recebido ({recebido}) menor que o valor do pagamento ({valor})"
            )));
        }
        Ok(Self {
            forma: FormaPagamento::Dinheiro,
            valor,
            recebido: Some(recebido),
        })
    }

    /// Pagamento eletrônico (débito, crédito, Pix ou outro).
    pub fn eletronico(forma: FormaPagamento, valor: Money) -> DomainResult<Self> {
        if !forma.is_concreta() {
            return Err(DomainError::Invalido(
                "Pagamento misto exige dois ou mais pagamentos concretos".into(),
            ));
        }
        if forma == FormaPagamento::Dinheiro {
            return Err(DomainError::Invalido(
                "Use Pagamento::dinheiro para pagamentos em dinheiro".into(),
            ));
        }
        valor.assert_nao_negativo().map_err(|_| {
            DomainError::Invalido("Valor de pagamento não pode ser negativo".into())
        })?;
        if valor.is_zero() {
            return Err(DomainError::Invalido("Pagamento não pode ser zero".into()));
        }
        Ok(Self {
            forma,
            valor,
            recebido: None,
        })
    }

    /// Troco deste pagamento (só dinheiro pode gerar troco).
    pub fn troco(&self) -> Money {
        match (self.forma.movimenta_dinheiro_fisico(), self.recebido) {
            (true, Some(recebido)) => recebido - self.valor,
            _ => Money::ZERO,
        }
    }

    /// Valida que o pagamento é internamente consistente.
    pub fn validar(&self) -> DomainResult<()> {
        if !self.forma.is_concreta() {
            return Err(DomainError::Invalido(
                "Pagamento misto exige dois ou mais pagamentos concretos".into(),
            ));
        }
        if self.valor.is_zero() {
            return Err(DomainError::Invalido("Pagamento não pode ser zero".into()));
        }
        if self.valor.is_negative() {
            return Err(DomainError::Invalido(
                "Pagamento não pode ser negativo".into(),
            ));
        }
        if self.forma.movimenta_dinheiro_fisico() {
            match self.recebido {
                None => {
                    return Err(DomainError::Invalido(
                        "Pagamento em dinheiro exige o valor recebido".into(),
                    ))
                }
                Some(recebido) if recebido < self.valor => {
                    return Err(DomainError::Invalido(format!(
                        "Valor recebido ({recebido}) menor que o valor do pagamento ({})",
                        self.valor
                    )))
                }
                Some(_) => {}
            }
        } else if self.recebido.is_some() {
            return Err(DomainError::Invalido(
                "Somente pagamento em dinheiro tem valor recebido/troco".into(),
            ));
        }
        Ok(())
    }
}

/// Uma venda finalizada e imutável.
/// Construída via `VendaBuilder` para garantir que todos os invariantes sejam verificados
/// antes de persistir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Venda {
    pub id: VendaId,
    pub sessao_id: SessaoId,
    pub dispositivo_id: DispositivoId,
    pub operador_id: UsuarioId,
    pub vendedor_id: Option<UsuarioId>,
    pub comanda_id: Option<String>,
    pub estado: EstadoVenda,
    /// Forma única, ou `Misto` quando há dois ou mais pagamentos.
    pub forma_pagamento: FormaPagamento,
    pub pagamentos: Vec<Pagamento>,
    pub itens: Vec<ItemVenda>,
    // Calculados no servidor (regra inviolável — a UI não determina o total)
    pub subtotal: Money,
    pub desconto_total: Money,
    pub total: Money,
    /// Soma do dinheiro físico entregue (só pagamentos em dinheiro).
    pub recebido: Money,
    /// Soma do troco (só pagamentos em dinheiro).
    pub troco: Money,
    pub created_at: DateTime<Utc>,
}

impl Venda {
    /// Recalcula o troco a partir dos pagamentos.
    pub fn troco_esperado(&self) -> DomainResult<Money> {
        Ok(self
            .pagamentos
            .iter()
            .fold(Money::ZERO, |acc, p| acc + p.troco()))
    }

    /// Soma dos pagamentos — deve ser igual ao total (validado no build,
    /// reconferido aqui para quem carregar a venda do banco).
    pub fn total_pago(&self) -> Money {
        self.pagamentos
            .iter()
            .fold(Money::ZERO, |acc, p| acc + p.valor)
    }
}

/// Builder de venda — garante que todos os invariantes sejam checados
/// antes de criar uma `Venda` pronta para persistência.
#[derive(Debug)]
pub struct VendaBuilder {
    id: VendaId,
    sessao_id: SessaoId,
    dispositivo_id: DispositivoId,
    operador_id: UsuarioId,
    vendedor_id: Option<UsuarioId>,
    comanda_id: Option<String>,
    itens: Vec<ItemVenda>,
    pagamentos: Vec<Pagamento>,
    // Atalho para o caso de pagamento único (compatibilidade e ergonomia).
    forma_unica: Option<FormaPagamento>,
    recebido_unico: Money,
    desconto_venda: Money, // desconto sobre o total (além dos descontos por item)
}

impl VendaBuilder {
    pub fn novo(
        id: VendaId,
        sessao_id: SessaoId,
        dispositivo_id: DispositivoId,
        operador_id: UsuarioId,
    ) -> Self {
        Self {
            id,
            sessao_id,
            dispositivo_id,
            operador_id,
            vendedor_id: None,
            comanda_id: None,
            itens: Vec::new(),
            pagamentos: Vec::new(),
            forma_unica: None,
            recebido_unico: Money::ZERO,
            desconto_venda: Money::ZERO,
        }
    }

    pub fn vendedor(mut self, id: UsuarioId) -> Self {
        self.vendedor_id = Some(id);
        self
    }

    pub fn comanda(mut self, id: String) -> Self {
        self.comanda_id = Some(id);
        self
    }

    pub fn adicionar_item(mut self, item: ItemVenda) -> Self {
        self.itens.push(item);
        self
    }

    /// Adiciona um pagamento concreto. Use `Pagamento::dinheiro` ou
    /// `Pagamento::eletronico` para construir cada um.
    pub fn pagamento(mut self, pagamento: Pagamento) -> Self {
        self.pagamentos.push(pagamento);
        self
    }

    /// Atalho para venda de pagamento único.
    pub fn forma_pagamento(mut self, forma: FormaPagamento) -> Self {
        self.forma_unica = Some(forma);
        self
    }

    /// Atalho para venda de pagamento único em dinheiro.
    pub fn recebido(mut self, valor: Money) -> Self {
        self.recebido_unico = valor;
        self
    }

    pub fn desconto_venda(mut self, desconto: Money) -> Self {
        self.desconto_venda = desconto;
        self
    }

    /// Resolve os pagamentos: usa a lista explícita ou o atalho de forma única.
    fn resolver_pagamentos(&self, total: Money) -> DomainResult<Vec<Pagamento>> {
        if !self.pagamentos.is_empty() {
            if self.forma_unica.is_some() || !self.recebido_unico.is_zero() {
                return Err(DomainError::Invalido(
                    "Não misture `pagamento()` com `forma_pagamento()`/`recebido()`".into(),
                ));
            }
            let mut soma = Money::ZERO;
            for pagamento in &self.pagamentos {
                pagamento.validar()?;
                soma = soma.somar(pagamento.valor).map_err(|_| {
                    DomainError::InvarianteViolada("Overflow na soma dos pagamentos".into())
                })?;
            }
            if soma != total {
                return Err(DomainError::Invalido(format!(
                    "Soma dos pagamentos ({soma}) diferente do total ({total})"
                )));
            }
            if self.pagamentos.len() == 1 {
                return Ok(vec![self.pagamentos[0].clone()]);
            }
            return Ok(self.pagamentos.clone());
        }

        let forma = self.forma_unica.clone().unwrap_or(FormaPagamento::Dinheiro);
        if !forma.is_concreta() {
            return Err(DomainError::Invalido(
                "Pagamento misto exige dois ou mais pagamentos concretos".into(),
            ));
        }

        let pagamento = if forma.movimenta_dinheiro_fisico() {
            Pagamento::dinheiro(total, self.recebido_unico)?
        } else {
            Pagamento::eletronico(forma, total)?
        };
        Ok(vec![pagamento])
    }

    /// Finaliza e valida a venda. Retorna erro se algum invariante for violado.
    pub fn build(self, agora: DateTime<Utc>) -> DomainResult<Venda> {
        if self.itens.is_empty() {
            return Err(DomainError::Invalido("Venda sem itens".into()));
        }

        // Calcular subtotal a partir dos itens (servidor recalcula — nunca confia na UI)
        let subtotal = self
            .itens
            .iter()
            .map(ItemVenda::subtotal)
            .collect::<DomainResult<Vec<Money>>>()?
            .into_iter()
            .try_fold(Money::ZERO, |acc, s| {
                acc.somar(s).map_err(|_| {
                    DomainError::InvarianteViolada("Overflow no subtotal da venda".into())
                })
            })?;

        self.desconto_venda
            .assert_nao_negativo()
            .map_err(|_| DomainError::Invalido("Desconto de venda não pode ser negativo".into()))?;

        if self.desconto_venda > subtotal {
            return Err(DomainError::Invalido(
                "Desconto total não pode ser maior que o subtotal".into(),
            ));
        }

        let total = subtotal - self.desconto_venda;
        if total.is_zero() {
            return Err(DomainError::Invalido(
                "Total da venda não pode ser zero".into(),
            ));
        }

        let pagamentos = self.resolver_pagamentos(total)?;

        let forma_pagamento = if pagamentos.len() == 1 {
            pagamentos[0].forma.clone()
        } else {
            FormaPagamento::Misto
        };

        let recebido = pagamentos
            .iter()
            .filter_map(|p| p.recebido)
            .fold(Money::ZERO, |acc, v| acc + v);
        let troco = pagamentos
            .iter()
            .fold(Money::ZERO, |acc, p| acc + p.troco());

        Ok(Venda {
            id: self.id,
            sessao_id: self.sessao_id,
            dispositivo_id: self.dispositivo_id,
            operador_id: self.operador_id,
            vendedor_id: self.vendedor_id,
            comanda_id: self.comanda_id,
            estado: EstadoVenda::Finalizada,
            forma_pagamento,
            pagamentos,
            itens: self.itens,
            subtotal,
            desconto_total: self.desconto_venda,
            total,
            recebido,
            troco,
            created_at: agora,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ItemVenda;
    use mikella_shared_kernel::ids::*;

    fn item_simples(preco: i64, qty: u32) -> ItemVenda {
        ItemVenda::novo(
            ProdutoId::new(),
            None,
            qty,
            Money::from_centavos(preco),
            Money::ZERO,
        )
        .unwrap()
    }

    #[test]
    fn venda_calcula_total_corretamente() {
        let venda = VendaBuilder::novo(
            VendaId::new(),
            SessaoId::new(),
            DispositivoId::new(),
            UsuarioId::new(),
        )
        .adicionar_item(item_simples(12990, 1)) // R$ 129,90
        .adicionar_item(item_simples(5480, 1)) // R$ 54,80
        .forma_pagamento(FormaPagamento::Dinheiro)
        .recebido(Money::from_centavos(20000)) // R$ 200,00
        .build(Utc::now())
        .unwrap();

        assert_eq!(venda.subtotal.centavos(), 18470); // R$ 184,70
        assert_eq!(venda.total.centavos(), 18470);
        assert_eq!(venda.troco.centavos(), 1530); // R$ 15,30
        assert_eq!(venda.forma_pagamento, FormaPagamento::Dinheiro);
        assert_eq!(venda.pagamentos.len(), 1);
    }

    #[test]
    fn venda_com_desconto() {
        let venda = VendaBuilder::novo(
            VendaId::new(),
            SessaoId::new(),
            DispositivoId::new(),
            UsuarioId::new(),
        )
        .adicionar_item(item_simples(10000, 2)) // R$ 200,00
        .desconto_venda(Money::from_centavos(1000)) // R$ 10,00 desconto
        .forma_pagamento(FormaPagamento::Pix)
        .build(Utc::now())
        .unwrap();

        assert_eq!(venda.subtotal.centavos(), 20000);
        assert_eq!(venda.total.centavos(), 19000);
        assert_eq!(venda.forma_pagamento, FormaPagamento::Pix);
        assert_eq!(venda.troco.centavos(), 0);
    }

    #[test]
    fn venda_sem_itens_falha() {
        let r = VendaBuilder::novo(
            VendaId::new(),
            SessaoId::new(),
            DispositivoId::new(),
            UsuarioId::new(),
        )
        .build(Utc::now());
        assert!(r.is_err());
    }

    #[test]
    fn venda_recebido_menor_que_total_falha() {
        let r = VendaBuilder::novo(
            VendaId::new(),
            SessaoId::new(),
            DispositivoId::new(),
            UsuarioId::new(),
        )
        .adicionar_item(item_simples(10000, 1))
        .forma_pagamento(FormaPagamento::Dinheiro)
        .recebido(Money::from_centavos(5000)) // menos que R$ 100
        .build(Utc::now());
        assert!(r.is_err());
    }

    #[test]
    fn pagamento_misto_corretamente() {
        let venda = VendaBuilder::novo(
            VendaId::new(),
            SessaoId::new(),
            DispositivoId::new(),
            UsuarioId::new(),
        )
        .adicionar_item(item_simples(10000, 1)) // R$ 100,00
        .pagamento(
            Pagamento::dinheiro(Money::from_centavos(4000), Money::from_centavos(5000)).unwrap(),
        )
        .pagamento(Pagamento::eletronico(FormaPagamento::Pix, Money::from_centavos(6000)).unwrap())
        .build(Utc::now())
        .unwrap();

        assert_eq!(venda.total.centavos(), 10000);
        assert_eq!(venda.total_pago().centavos(), 10000);
        assert_eq!(venda.recebido.centavos(), 5000);
        assert_eq!(venda.troco.centavos(), 1000);
        assert_eq!(venda.forma_pagamento, FormaPagamento::Misto);
    }

    #[test]
    fn pagamento_misto_com_soma_errada_falha() {
        let r = VendaBuilder::novo(
            VendaId::new(),
            SessaoId::new(),
            DispositivoId::new(),
            UsuarioId::new(),
        )
        .adicionar_item(item_simples(10000, 1))
        .pagamento(Pagamento::eletronico(FormaPagamento::Pix, Money::from_centavos(3000)).unwrap())
        .pagamento(
            Pagamento::eletronico(FormaPagamento::Debito, Money::from_centavos(3000)).unwrap(),
        )
        .build(Utc::now());
        assert!(r.is_err());
    }

    #[test]
    fn forma_mista_sem_split_falha() {
        let r = VendaBuilder::novo(
            VendaId::new(),
            SessaoId::new(),
            DispositivoId::new(),
            UsuarioId::new(),
        )
        .adicionar_item(item_simples(10000, 1))
        .forma_pagamento(FormaPagamento::Misto)
        .build(Utc::now());
        assert!(r.is_err());
    }

    #[test]
    fn misturar_atalho_e_lista_falha() {
        let r = VendaBuilder::novo(
            VendaId::new(),
            SessaoId::new(),
            DispositivoId::new(),
            UsuarioId::new(),
        )
        .adicionar_item(item_simples(10000, 1))
        .forma_pagamento(FormaPagamento::Pix)
        .pagamento(Pagamento::eletronico(FormaPagamento::Pix, Money::from_centavos(10000)).unwrap())
        .build(Utc::now());
        assert!(r.is_err());
    }
}
