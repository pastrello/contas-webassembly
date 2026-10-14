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
