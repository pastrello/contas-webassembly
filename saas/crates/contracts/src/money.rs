/// Monetary input never passes through floating point. Both decimal separators
/// are accepted; thousands separators and more than two decimals are rejected.
pub const MAX_CENTS: i64 = 9_000_000_000_000;
pub fn parse_cents(text: &str) -> Result<i64, &'static str> {
    let text = text.trim();
    let (negative, digits) = if let Some(rest) = text.strip_prefix('-') {
        (true, rest)
    } else {
        (false, text)
    };
    let mut parts = digits.split(['.', ',']);
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some() || whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Informe um valor sem separador de milhar, como 1234,56.");
    }
    let cents = match fraction {
        None => 0,
        Some(f) if (1..=2).contains(&f.len()) && f.bytes().all(|b| b.is_ascii_digit()) => {
            f.parse::<i64>().unwrap() * if f.len() == 1 { 10 } else { 1 }
        }
        _ => return Err("Use no máximo duas casas decimais, como 1234,56."),
    };
    let amount = whole
        .parse::<i64>()
        .ok()
        .and_then(|v| v.checked_mul(100))
        .and_then(|v| v.checked_add(cents))
        .filter(|v| *v <= MAX_CENTS)
        .ok_or("Valor fora do limite permitido.")?;
    Ok(if negative { -amount } else { amount })
}
pub fn decimal(cents: i128) -> String {
    let value = cents.unsigned_abs();
    format!(
        "{}{}.{:02}",
        if cents < 0 { "-" } else { "" },
        value / 100,
        value % 100
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn money_is_exact_and_rejects_ambiguous_or_out_of_range_input() {
        for (text, expected) in [
            ("0", 0),
            ("1234,56", 123456),
            ("-0.01", -1),
            (" 12.3 ", 1230),
            ("90000000000.00", MAX_CENTS),
            ("-90000000000,00", -MAX_CENTS),
        ] {
            assert_eq!(parse_cents(text), Ok(expected));
            assert_eq!(parse_cents(&decimal(expected as i128)), Ok(expected));
        }
        for bad in [
            "",
            "1.234,56",
            "1,234.56",
            "1.234",
            "1,",
            ".50",
            "NaN",
            "1e3",
            "+1",
            "--1",
            "90000000000.01",
            "99999999999999999999999999",
        ] {
            assert!(parse_cents(bad).is_err(), "{bad}");
        }
        assert_eq!(decimal(10i128 + 20i128), "0.30");
    }
}
