use std::fmt;
use std::num::ParseFloatError;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Tag object as described in https://lunchmoney.dev/#tags-object.
#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Cleared,
    Uncleared,
    Recurring,
    RecurringSuggested,
}

/// An f64 that serializes to a float up to 4 decimal places, as specified in the `Transaction`
/// amount field description in https://lunchmoney.dev/#transaction-object.
#[derive(Debug, Serialize, Deserialize)]
pub struct Amount(f64);

impl FromStr for Amount {
    type Err = ParseFloatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Amount(s.parse::<f64>()?))
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}

impl From<f64> for Amount {
    fn from(val: f64) -> Self {
        Amount(val)
    }
}

/// Transaction object as defined in https://lunchmoney.dev/#transaction-object
#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub id: i64,
    pub date: DateTime<Utc>,
    pub payee: Option<String>,
    pub amount: Amount,
    pub currency: Option<String>,
    pub notes: Option<String>,
    pub category_id: Option<i64>,
    pub asset_id: Option<i64>,
    pub status: TransactionStatus,
    pub parent_id: Option<i64>,
    pub is_group: Option<bool>,
    pub group_id: Option<i64>,
    pub tags: Option<Vec<Tag>>,
    pub external_id: Option<String>,
    pub original_name: Option<String>,
}
