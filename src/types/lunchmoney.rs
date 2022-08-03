use std::fmt;
use std::num::ParseFloatError;
use std::str::FromStr;
use std::time::UNIX_EPOCH;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, skip_serializing_none, DisplayFromStr};

/// Tag object as described in https://lunchmoney.dev/#tags-object.
#[derive(Debug, Serialize)]
pub struct Tag {
    pub id: u64,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Cleared,
    Uncleared,
    Recurring,
    RecurringSuggested,
}

/// An f64 that serializes to a float up to 4 decimal places, as specified in the `Transaction`
/// amount field description in https://lunchmoney.dev/#transaction-object.
#[derive(Debug)]
pub struct Amount(pub f64);

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
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct Transaction {
    pub id: Option<u64>,
    pub date: DateTime<Utc>,
    pub payee: Option<String>,
    #[serde_as(as = "DisplayFromStr")]
    pub amount: Amount,
    pub currency: Option<String>,
    pub notes: Option<String>,
    pub category_id: Option<u64>,
    pub asset_id: Option<u64>,
    pub status: TransactionStatus,
    pub parent_id: Option<u64>,
    pub is_group: Option<bool>,
    pub group_id: Option<u64>,
    pub tags: Option<Vec<Tag>>,
    pub external_id: Option<String>,
    pub original_name: Option<String>,
}

impl Default for Transaction {
    fn default() -> Self {
        Self {
            id: None,
            date: UNIX_EPOCH.into(),
            payee: None,
            amount: Amount(0.0),
            currency: None,
            notes: None,
            category_id: None,
            asset_id: None,
            status: TransactionStatus::Uncleared,
            parent_id: None,
            is_group: None,
            group_id: None,
            tags: None,
            external_id: None,
            original_name: None,
        }
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct Asset {
    pub id: u64,
    #[serde(rename = "type_name")]
    pub type_: String,
    #[serde(rename = "subtype_name")]
    pub subtype: Option<String>,
    pub name: String,
    pub display_name: Option<String>,
    #[serde_as(as = "DisplayFromStr")]
    pub balance: Amount,
    pub balance_as_of: DateTime<Utc>,
    pub closed_on: Option<String>,
    pub currency: String,
    pub institution_name: String,
    pub exclude_transactions: Option<bool>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct GetAllAssetsResponse {
    pub assets: Vec<Asset>,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
pub struct InsertTransactionRequest {
    pub transactions: Vec<Transaction>,
    pub apply_rules: Option<bool>,
    pub skip_duplicates: Option<bool>,
    pub check_for_recurring: Option<bool>,
    pub debit_as_negative: Option<bool>,
    pub skip_balance_update: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct InsertTransactionResponse {
    pub ids: Vec<u64>,
}
