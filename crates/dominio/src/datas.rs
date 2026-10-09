//! Datas como o visitante lê.

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};

const MESES: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];

/// O dia em Brasília, três horas atrás de Greenwich. O Brasil não tem horário
/// de verão desde 2019.
fn dia_em_brasilia(momento: DateTime<Utc>) -> NaiveDate {
    (momento - Duration::hours(3)).date_naive()
}

/// "12 de agosto de 2026".
pub fn por_extenso(momento: DateTime<Utc>) -> String {
    let dia = dia_em_brasilia(momento);
    let mes = MESES.get(dia.month0() as usize).copied().unwrap_or("");
    format!("{} de {mes} de {}", dia.day(), dia.year())
}

/// "2026-08-12", para o atributo `datetime`.
pub fn iso(momento: DateTime<Utc>) -> String {
    dia_em_brasilia(momento).format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod testes {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn escreve_a_data_em_portugues() {
        let momento = Utc
            .with_ymd_and_hms(2026, 8, 12, 15, 0, 0)
            .single()
            .expect("data válida");
        assert_eq!(por_extenso(momento), "12 de agosto de 2026");
        assert_eq!(iso(momento), "2026-08-12");
    }

    #[test]
    fn a_data_e_a_de_brasilia_nao_a_de_greenwich() {
        // 1h da manhã em Greenwich ainda é o dia anterior em Brasília.
        let momento = Utc
            .with_ymd_and_hms(2026, 3, 1, 1, 0, 0)
            .single()
            .expect("data válida");
        assert_eq!(por_extenso(momento), "28 de fevereiro de 2026");
    }
}
