use std::str::FromStr;

use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("unexpected Venmo transaction type: {0}")]
    ParseTransactionTypeError(String),
    #[error("unexpected Venmo transaction status: {0}")]
    ParseStatusError(String),
    #[error("failed to parse Venmo amount: {0}")]
    ParseAmountError(String),
}

#[derive(Debug, Deserialize)]
pub enum TransactionType {
    Charge,
    Payment,
    StandardTransfer,
    MerchantTransaction,
}

impl FromStr for TransactionType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "Charge" => TransactionType::Charge,
            "Payment" => TransactionType::Payment,
            "Standard Transfer" => TransactionType::StandardTransfer,
            "Merchant Transaction" => TransactionType::MerchantTransaction,
            _ => {
                return Err(Error::ParseTransactionTypeError(s.to_string()));
            }
        })
    }
}

#[derive(Debug, Deserialize)]
pub enum TransactionStatus {
    Complete,
    Issued,
}

impl FromStr for TransactionStatus {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "Complete" => TransactionStatus::Complete,
            "Issued" => TransactionStatus::Issued,
            _ => {
                return Err(Error::ParseStatusError(s.to_string()));
            }
        })
    }
}

lazy_static! {
    static ref VENMO_AMOUNT_RE: Regex = Regex::new(r"^(-?)([^0-9])([0-9.]+)$").unwrap();
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Amount {
    pub currency: String,
    pub val: f64,
}

impl FromStr for Amount {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(captures) = VENMO_AMOUNT_RE.captures(s) {
            Ok(Amount {
                currency: captures.get(2).unwrap().as_str().to_string(),
                val: format!(
                    "{}{}",
                    captures.get(1).unwrap().as_str(),
                    captures.get(3).unwrap().as_str()
                )
                .parse()
                .map_err(|_| Error::ParseAmountError(s.to_string()))?,
            })
        } else {
            Err(Error::ParseAmountError(s.to_string()))
        }
    }
}

/// Venmo transaction structure as found in their statement CSVs.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Transaction {
    #[serde(rename = "ID")]
    pub id: i64,
    pub datetime: DateTime<Utc>,
    #[serde(rename = "Type")]
    pub type_: TransactionType,
    pub status: TransactionStatus,
    pub note: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    #[serde(rename = "Amount (total)")]
    pub total_amount: Amount,
    #[serde(rename = "Funding Source")]
    pub funding_source: Option<String>,
    pub destination: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BalanceRecord {
    #[serde(rename = "Beginning Balance")]
    pub beginning_balance: Option<Amount>,
    #[serde(rename = "Ending Balance")]
    pub ending_balance: Option<Amount>,
}
