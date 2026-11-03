//! Formato canônico de data/hora para o banco.
//!
//! Todas as colunas de tempo são TEXT e algumas consultas ordenam/paginam por
//! elas. Misturar `+00:00` com `Z` quebraria a ordem lexicográfica — por isso
//! a infraestrutura grava sempre UTC, com milissegundos e sufixo `Z`, igual ao
//! default do schema (`strftime('%Y-%m-%dT%H:%M:%fZ','now')`).
//!
//! O mesmo formato entra no hash do livro: o hash é recalculado a partir do
//! que foi lido do banco, e sub-milissegundos não sobrevivem ao TEXT.

use chrono::{DateTime, SecondsFormat, Utc};

pub(crate) fn data_hora_sql(valor: DateTime<Utc>) -> String {
    valor.to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usa_formato_canonico_utc() {
        let instante = DateTime::parse_from_rfc3339("2026-10-30T12:34:56.789123-03:00")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(data_hora_sql(instante), "2026-10-30T15:34:56.789Z");
    }

    #[test]
    fn milissegundos_sobrevivem_ao_round_trip() {
        let instante = DateTime::parse_from_rfc3339("2026-10-30T15:34:56.789Z")
            .unwrap()
            .with_timezone(&Utc);
        let texto = data_hora_sql(instante);
        let recarregado = DateTime::parse_from_rfc3339(&texto)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(data_hora_sql(recarregado), texto);
    }
}
