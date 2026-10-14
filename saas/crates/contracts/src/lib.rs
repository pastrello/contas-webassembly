use serde::{Deserialize, Serialize};

/// Public operational status only; never includes credentials or customer data.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub service: String,
    pub version: String,
    pub database: String,
}
