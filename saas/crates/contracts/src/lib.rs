use serde::{Deserialize, Serialize};
pub mod money;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountInput {
    pub code: String,
    pub number: String,
    pub name: String,
    pub opening_balance: String,
    pub active: bool,
    pub expected_version: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub code: String,
    pub number: String,
    pub name: String,
    pub opening_balance: String,
    pub active: bool,
    pub version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryInput {
    pub code: String,
    pub description: String,
    pub direction: i16,
    pub active: bool,
    pub expected_version: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    pub id: String,
    pub code: String,
    pub description: String,
    pub direction: i16,
    pub active: bool,
    pub version: i64,
}

/// Public operational status only; never includes credentials or customer data.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub service: String,
    pub version: String,
    pub database: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryInput {
    pub account_id: String,
    pub history_id: String,
    pub date: String,
    pub document: String,
    pub description: String,
    pub amount: String,
    pub cleared: bool,
    pub archived: bool,
    pub request_id: Option<String>,
    pub expected_version: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub account_id: String,
    pub history_id: String,
    pub date: String,
    pub document: String,
    pub description: String,
    pub amount: String,
    pub direction: i16,
    pub cleared: bool,
    pub archived: bool,
    pub cancelled: bool,
    pub version: i64,
    pub history_label: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ledger {
    pub items: Vec<Entry>,
    pub initial_balance: String,
    pub previous_balance: String,
    pub period_income: String,
    pub period_expense: String,
    pub period_balance: String,
    pub current_balance: String,
    pub bank_balance: String,
    pub total: usize,
    pub next_offset: Option<usize>,
}
pub fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || !b
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return false;
    }
    let y: u32 = s[..4].parse().unwrap();
    let m: usize = s[5..7].parse().unwrap();
    let d: u32 = s[8..].parse().unwrap();
    let leap = y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    y > 0
        && (1..=12).contains(&m)
        && d > 0
        && d <= [
            31,
            if leap { 29 } else { 28 },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ][m - 1]
}
#[cfg(test)]
mod date_tests {
    #[test]
    fn calendar_boundaries() {
        for d in ["0001-01-01", "2024-02-29", "2000-02-29", "9999-12-31"] {
            assert!(super::valid_date(d));
        }
        for d in [
            "0000-01-01",
            "2026-02-29",
            "1900-02-29",
            "2026-04-31",
            "2026-00-10",
            "2026-13-01",
            "2026-1-01",
            "é026-01-01",
        ] {
            assert!(!super::valid_date(d));
        }
    }
}
