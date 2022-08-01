use std::str::FromStr;

use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use rusty_money::iso::Currency;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::lunchmoney;

#[derive(Error, Debug)]
pub enum Error {
    #[error("unexpected Venmo transaction type: {0}")]
    ParseTransactionTypeError(String),
    #[error("unexpected Venmo transaction status: {0}")]
    ParseStatusError(String),
    #[error("failed to parse Venmo amount: {0}")]
    ParseAmountError(String),
    #[error("expected currency marker {0} for {1}, got {2} from Venmo")]
    WrongCurrencyError(String, String, String),
    #[error("expected field {0} to be defined due to {1} on record {2:?}")]
    InvalidRecord(String, String, Transaction),
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Transaction {
    #[serde(rename = "ID")]
    pub id: u64,
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

impl Transaction {
    fn to_lunchmoney_transactions(
        &self,
        expected_currency: Currency,
        asset_id: u64,
    ) -> Result<Vec<lunchmoney::Transaction>, Error> {
        if self.total_amount.currency != expected_currency.symbol {
            return Err(Error::WrongCurrencyError(
                expected_currency.symbol.to_string(),
                expected_currency.iso_alpha_code.to_string(),
                self.total_amount.currency.clone(),
            ));
        }

        let payee = match self.type_ {
            TransactionType::StandardTransfer => self
                .destination
                .as_ref()
                .map(|val| format!("TRANSFER TO {}", val))
                .ok_or_else(|| {
                    Error::InvalidRecord(
                        "destination".to_string(),
                        "'Transaction Type' is set to 'Standard Transfer'".to_string(),
                        self.clone(),
                    )
                })?,
            TransactionType::Charge => {
                if self.total_amount.val.is_sign_positive() {
                    self.to.as_ref().cloned().ok_or_else(|| {
                        Error::InvalidRecord(
                            "to".to_string(),
                            "'Transaction Type' is set to 'Charge' and 'Amount' is positive"
                                .to_string(),
                            self.clone(),
                        )
                    })?
                } else {
                    self.from.as_ref().cloned().ok_or_else(|| {
                        Error::InvalidRecord(
                            "from".to_string(),
                            "'Transaction Type' is set to 'Charge' and 'Amount' is negative"
                                .to_string(),
                            self.clone(),
                        )
                    })?
                }
            }
            TransactionType::Payment | TransactionType::MerchantTransaction => {
                if self.total_amount.val.is_sign_positive() {
                    self.from.as_ref().cloned().ok_or_else(|| {
                        Error::InvalidRecord(
                            "from".to_string(),
                            "'Transaction Type' is set to 'Payment' or 'Merchant Transaction' and 'Amount' is positive"
                                .to_string(),
                            self.clone(),
                        )
                    })?
                } else {
                    self.to.as_ref().cloned().ok_or_else(|| {
                        Error::InvalidRecord(
                            "to".to_string(),
                            "'Transaction Type' is set to 'Payment' or 'Merchant Transaction' and 'Amount' is negative"
                                .to_string(),
                            self.clone(),
                        )
                    })?
                }
            }
        };

        let transactions = {
            let mut txn = vec![lunchmoney::Transaction {
                date: self.datetime,
                payee: Some(payee),
                amount: lunchmoney::Amount(self.total_amount.val),
                currency: Some(expected_currency.iso_alpha_code.to_string()),
                notes: self.note.as_ref().cloned(),
                asset_id: Some(asset_id),
                external_id: Some(self.id.to_string()),
                status: lunchmoney::TransactionStatus::Uncleared,
                ..Default::default()
            }];

            if let Some(ref funding_source) = self.funding_source {
                if !funding_source.is_empty() && funding_source != "Venmo balance" {
                    // Create a "shadow" transaction to indicate we transfered money from one
                    // bank to our Venmo balance.
                    txn.push(lunchmoney::Transaction {
                        date: self.datetime,
                        payee: Some(format!("TRANSFER FROM {}", funding_source)),
                        amount: lunchmoney::Amount(-self.total_amount.val),
                        currency: Some(expected_currency.iso_alpha_code.to_string()),
                        notes: self
                            .note
                            .as_ref()
                            .map(|val| format!("To fund Venmo transaction with note: '{}'", val)),
                        asset_id: Some(asset_id),
                        external_id: Some(format!("{}T", self.id)),
                        status: lunchmoney::TransactionStatus::Uncleared,
                        ..Default::default()
                    });
                }
            }

            if let Some(ref destination) = self.destination {
                // It should never be possible to direct deposit a Venmo transaction to your bank
                // account since Venmo always deposits it in your "Venmo balance" first... but just
                // to cover our bases.
                if !destination.is_empty()
                    && destination != "Venmo balance"
                    && self.type_ != TransactionType::StandardTransfer
                {
                    txn.push(lunchmoney::Transaction {
                        date: self.datetime,
                        payee: Some(format!("TRANSFER TO {}", destination)),
                        amount: lunchmoney::Amount(-self.total_amount.val),
                        currency: Some(expected_currency.iso_alpha_code.to_string()),
                        notes: self
                            .note
                            .as_ref()
                            .map(|val| format!("From Venmo transaction with note: '{}'", val)),
                        asset_id: Some(asset_id),
                        external_id: Some(format!("{}TDEPOSIT", self.id)),
                        status: lunchmoney::TransactionStatus::Uncleared,
                        ..Default::default()
                    });
                }
            }

            txn
        };

        Ok(transactions)
    }
}
