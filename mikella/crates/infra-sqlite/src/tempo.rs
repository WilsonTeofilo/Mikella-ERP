//! Formato canônico de data/hora para o banco.
//!
//! Todas as colunas de tempo são TEXT e algumas consultas ordenam/paginam por
//! elas. Misturar `+00:00` com `Z` quebraria a ordem lexicográfica — por isso
//! a infraestrutura grava sempre UTC, com milissegundos e sufixo `Z`, igual ao
//! default do schema (`strftime('%Y-%m-%dT%H:%M:%fZ','now')`).
//!
//! O mesmo formato entra no hash do livro: o hash é recalculado a partir do
//! que foi lido do banco, e sub-milissegundos não sobrevivem ao TEXT.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, SecondsFormat, TimeZone, Utc};
use chrono_tz::Tz;

use crate::error::InfraResult;

pub(crate) fn data_hora_sql(valor: DateTime<Utc>) -> String {
    valor.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Fuso IANA da loja, com o mesmo fallback usado no restante do sistema.
pub fn fuso_ou_padrao(fuso: &str) -> Tz {
    fuso.parse().unwrap_or(chrono_tz::America::Sao_Paulo)
}

/// Data de "hoje" no fuso da loja.
///
/// Nunca use `date('now')` do SQLite para regra de vencimento: ele é UTC e,
/// entre 21h e 0h em `America/Sao_Paulo`, aponta para o dia seguinte.
pub fn hoje_local(fuso: &str) -> NaiveDate {
    Utc::now().with_timezone(&fuso_ou_padrao(fuso)).date_naive()
}

/// `hoje` no fuso da loja, formatado como `AAAA-MM-DD` para parâmetros SQL.
pub fn data_local_iso(fuso: &str) -> String {
    hoje_local(fuso).to_string()
}

/// Soma (ou subtrai) dias da data local da loja, devolvendo `AAAA-MM-DD`.
pub fn data_local_mais_dias(fuso: &str, dias: i64) -> String {
    (hoje_local(fuso) + Duration::days(dias)).to_string()
}

/// Mês corrente no fuso da loja, formatado como `AAAA-MM`.
pub fn mes_local(fuso: &str) -> String {
    hoje_local(fuso).format("%Y-%m").to_string()
}

fn inicio_do_dia_utc(tz: Tz, dia: NaiveDate) -> InfraResult<DateTime<Utc>> {
    tz.from_local_datetime(&dia.and_time(NaiveTime::MIN))
        .single()
        .map(|dt| dt.with_timezone(&Utc))
        .ok_or_else(|| crate::error::erro_campo("data", "horário inexistente no fuso"))
}

/// Converte um dia local (`AAAA-MM-DD`) no intervalo UTC fechado do dia.
pub fn intervalo_utc_do_dia(fuso: &str, data_local: &str) -> InfraResult<(String, String)> {
    let tz = fuso_ou_padrao(fuso);
    let dia = NaiveDate::parse_from_str(data_local.trim(), "%Y-%m-%d")
        .map_err(|_| crate::error::erro_campo("data", "use AAAA-MM-DD"))?;
    let inicio = inicio_do_dia_utc(tz, dia)?;
    let fim = inicio_do_dia_utc(tz, dia + Duration::days(1))? - Duration::milliseconds(1);
    Ok((data_hora_sql(inicio), data_hora_sql(fim)))
}

/// Converte um mês local (`AAAA-MM`) no intervalo UTC fechado do mês.
pub fn intervalo_utc_do_mes(fuso: &str, mes: &str) -> InfraResult<(String, String)> {
    let tz = fuso_ou_padrao(fuso);
    let primeiro = NaiveDate::parse_from_str(&format!("{}-01", mes.trim()), "%Y-%m-%d")
        .map_err(|_| crate::error::erro_campo("mes", "use AAAA-MM"))?;
    let proximo = if primeiro.month() == 12 {
        NaiveDate::from_ymd_opt(primeiro.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(primeiro.year(), primeiro.month() + 1, 1)
    }
    .ok_or_else(|| crate::error::erro_campo("mes", "mês inválido"))?;

    let inicio = inicio_do_dia_utc(tz, primeiro)?;
    let fim = inicio_do_dia_utc(tz, proximo)? - Duration::milliseconds(1);
    Ok((data_hora_sql(inicio), data_hora_sql(fim)))
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

    #[test]
    fn intervalo_do_dia_respeita_fuso_da_loja() {
        let (ini_sp, fim_sp) = intervalo_utc_do_dia("America/Sao_Paulo", "2026-11-07").unwrap();
        assert_eq!(ini_sp, "2026-11-07T03:00:00.000Z");
        assert_eq!(fim_sp, "2026-11-08T02:59:59.999Z");

        let (ini_mao, fim_mao) = intervalo_utc_do_dia("America/Manaus", "2026-11-07").unwrap();
        assert_eq!(ini_mao, "2026-11-07T04:00:00.000Z");
        assert_eq!(fim_mao, "2026-11-08T03:59:59.999Z");
    }

    #[test]
    fn intervalo_do_mes_atravessa_virada_de_ano() {
        let (ini, fim) = intervalo_utc_do_mes("America/Sao_Paulo", "2026-12").unwrap();
        assert_eq!(ini, "2026-12-01T03:00:00.000Z");
        assert_eq!(fim, "2027-01-01T02:59:59.999Z");
    }

    #[test]
    fn data_invalida_vira_erro_observavel() {
        assert!(intervalo_utc_do_dia("America/Sao_Paulo", "07/11/2026").is_err());
        assert!(intervalo_utc_do_mes("America/Sao_Paulo", "2026-13").is_err());
    }
}
