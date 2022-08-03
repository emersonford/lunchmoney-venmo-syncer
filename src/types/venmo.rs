use std::fmt;
use std::str::FromStr;

use chrono::{offset::TimeZone, DateTime, NaiveDateTime, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use rusty_money::iso::Currency;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
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
    #[error("expected field {0} to be defined on record {1:?}")]
    InvalidRecord(String, TransactionRecord),
    #[error("expected field {0} to be defined due to {1} on record {2:?}")]
    InvalidTransaction(String, String, Transaction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
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
    static ref VENMO_AMOUNT_RE: Regex = Regex::new(r"^([-+]?)[ ]?([^0-9])([0-9.]+)$").unwrap();
}

#[derive(Debug, Clone)]
pub struct Amount {
    pub currency: String,
    pub val: f64,
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{:.4}",
            if self.val.is_sign_negative() { "-" } else { "" },
            self.currency,
            self.val.abs()
        )
    }
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
#[serde_as]
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct TransactionRecord {
    #[serde(rename = "ID")]
    pub id: Option<u64>,
    pub datetime: Option<NaiveDateTime>,
    #[serde(rename = "Type")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub type_: Option<TransactionType>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub status: Option<TransactionStatus>,
    pub note: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    #[serde(rename = "Amount (total)")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub amount_total: Option<Amount>,
    #[serde(rename = "Amount (tip)")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub amount_tip: Option<Amount>,
    #[serde(rename = "Amount (fee)")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub amount_fee: Option<Amount>,
    #[serde(rename = "Funding Source")]
    pub funding_source: Option<String>,
    pub destination: Option<String>,
    #[serde(rename = "Beginning Balance")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub beginning_balance: Option<Amount>,
    #[serde(rename = "Ending Balance")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub ending_balance: Option<Amount>,
    #[serde(rename = "Statement Period Venmo Fees")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub statment_period_venmo_fees: Option<Amount>,
    #[serde(rename = "Terminal Location")]
    pub terminal_location: Option<String>,
    #[serde(rename = "Year to Date Venmo Fees")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub year_to_date_venmo_fees: Option<Amount>,
    pub disclaimer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: u64,
    pub datetime: DateTime<Utc>,
    pub type_: TransactionType,
    pub status: TransactionStatus,
    pub note: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub amount_total: Amount,
    pub funding_source: Option<String>,
    pub destination: Option<String>,
}

impl TryFrom<TransactionRecord> for Transaction {
    type Error = Error;

    fn try_from(val: TransactionRecord) -> Result<Self, Self::Error> {
        if val.id.is_none() {
            return Err(Error::InvalidRecord("id".to_string(), val));
        }

        if val.datetime.is_none() {
            return Err(Error::InvalidRecord("datetime".to_string(), val));
        }

        if val.type_.is_none() {
            return Err(Error::InvalidRecord("type_".to_string(), val));
        }

        if val.status.is_none() {
            return Err(Error::InvalidRecord("status".to_string(), val));
        }

        if val.amount_total.is_none() {
            return Err(Error::InvalidRecord("amount_total".to_string(), val));
        }

        Ok(Self {
            id: val.id.unwrap(),
            datetime: Utc.from_utc_datetime(&val.datetime.unwrap()),
            type_: val.type_.unwrap(),
            status: val.status.unwrap(),
            note: val.note,
            from: val.from,
            to: val.to,
            amount_total: val.amount_total.unwrap(),
            funding_source: val.funding_source,
            destination: val.destination,
        })
    }
}

#[derive(Debug)]
pub struct Statement {
    pub beginning_balance: Amount,
    pub ending_balance: Amount,
    pub transactions: Vec<Transaction>,
}

impl Transaction {
    pub fn to_lunchmoney_transactions(
        &self,
        expected_currency: Currency,
        asset_id: u64,
    ) -> Result<Vec<lunchmoney::Transaction>, Error> {
        if self.amount_total.currency != expected_currency.symbol {
            return Err(Error::WrongCurrencyError(
                expected_currency.symbol.to_string(),
                expected_currency.iso_alpha_code.to_string(),
                self.amount_total.currency.clone(),
            ));
        }

        let payee = match self.type_ {
            TransactionType::StandardTransfer => self
                .destination
                .as_ref()
                .map(|val| format!("TRANSFER TO {}", val))
                .ok_or_else(|| {
                    Error::InvalidTransaction(
                        "destination".to_string(),
                        "'Transaction Type' is set to 'Standard Transfer'".to_string(),
                        self.clone(),
                    )
                })?,
            TransactionType::Charge => {
                if self.amount_total.val.is_sign_positive() {
                    self.to.as_ref().cloned().ok_or_else(|| {
                        Error::InvalidTransaction(
                            "to".to_string(),
                            "'Transaction Type' is set to 'Charge' and 'Amount' is positive"
                                .to_string(),
                            self.clone(),
                        )
                    })?
                } else {
                    self.from.as_ref().cloned().ok_or_else(|| {
                        Error::InvalidTransaction(
                            "from".to_string(),
                            "'Transaction Type' is set to 'Charge' and 'Amount' is negative"
                                .to_string(),
                            self.clone(),
                        )
                    })?
                }
            }
            TransactionType::Payment | TransactionType::MerchantTransaction => {
                if self.amount_total.val.is_sign_positive() {
                    self.from.as_ref().cloned().ok_or_else(|| {
                        Error::InvalidTransaction(
                            "from".to_string(),
                            "'Transaction Type' is set to 'Payment' or 'Merchant Transaction' and 'Amount' is positive"
                                .to_string(),
                            self.clone(),
                        )
                    })?
                } else {
                    self.to.as_ref().cloned().ok_or_else(|| {
                        Error::InvalidTransaction(
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
                amount: lunchmoney::Amount(self.amount_total.val),
                currency: Some(expected_currency.iso_alpha_code.to_string().to_lowercase()),
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
                        amount: lunchmoney::Amount(-self.amount_total.val),
                        currency: Some(expected_currency.iso_alpha_code.to_string().to_lowercase()),
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
                        amount: lunchmoney::Amount(-self.amount_total.val),
                        currency: Some(expected_currency.iso_alpha_code.to_string().to_lowercase()),
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

pub struct AccountRecord {
    pub profile_id: u64,
    pub api_token: String,
    pub currency: Currency,
}
