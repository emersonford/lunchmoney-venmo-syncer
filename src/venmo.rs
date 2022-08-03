use std::io::BufRead;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;

use chrono::{DateTime, Utc};
use hyper::header::COOKIE;
use hyper::{body, body::Buf, Method, Request, StatusCode};

use crate::types::venmo::{AccountRecord, Statement, TransactionRecord};
use crate::types::HttpsClient;

async fn fetch_venmo_statement(
    client: &HttpsClient,
    account: &AccountRecord,
    start_date: &DateTime<Utc>,
    end_date: &DateTime<Utc>,
) -> Result<body::Bytes> {
    let request = Request::builder()
        .method(Method::GET)
        .uri(
            format!(
                "https://venmo.com/transaction-history/statement?startDate={}&endDate={}&profileId={}&accountType=personal", 
                start_date.format("%m-%d-%Y"), 
                end_date.format("%m-%d-%Y"), 
                account.profile_id
            )
        )
        .header(COOKIE, format!("api_access_token={}", account.api_token)) 
        .body(body::Body::empty())
        .unwrap();

    let response = client.request(request).await?;

    if response.status() != StatusCode::OK {
        bail!(
            "Failed to get Venmo statement, code {}, err:\n{:#?}",
            response.status(),
            response
        );
    }

    let bytes = body::to_bytes(response).await?;

    if bytes.starts_with(b"Unable to fetch transaction history") {
        bail!("Venmo transaction history request failed: {:#?}", bytes);
    }

    Ok(bytes)
}

pub async fn fetch_venmo_transactions(
    client: &HttpsClient,
    account: &AccountRecord,
    start_date: &DateTime<Utc>,
    end_date: &DateTime<Utc>,
) -> Result<Statement> {
    let bytes = fetch_venmo_statement(client, account, start_date, end_date).await?;
    let bytes_clone = bytes.clone();

    let reader = {
        let mut reader = bytes.reader();
        let mut dummy_buf = String::new();

        reader.read_line(&mut dummy_buf).with_context(|| {
            anyhow!(
                "Failed to skip first line in Venmo statement:\n{:#?}",
                bytes_clone
            )
        })?;
        reader.read_line(&mut dummy_buf).with_context(|| {
            anyhow!(
                "Failed to skip second line in Venmo statement:\n{:#?}",
                bytes_clone
            )
        })?;

        reader
    };

    let mut rdr = csv::Reader::from_reader(reader);

    let mut transactions = Vec::new();

    let mut records_iter = rdr.deserialize().peekable();

    let beginning_record: TransactionRecord = records_iter.next().ok_or_else(|| {
        anyhow!(
            "Expected there to be a beginning balance record, found none in response:\n{:#?}",
            bytes_clone
        )
    })??;

    let beginning_balance = beginning_record.beginning_balance.ok_or_else(|| {
        anyhow!(
            "Expected 'Beginning Balance' to be set for the first record, got response:\n{:#?}",
            bytes_clone
        )
    })?;

    let ending_balance = loop {
        let record: TransactionRecord = records_iter.next().ok_or_else(|| {
            anyhow!(
                "Expected there to be an ending balance record, found none in response:\n{:#?}",
                bytes_clone
            )
        })??;

        // We're at our last record, meaning this should be the ending balance record.
        if records_iter.peek().is_none() {
            break record.ending_balance.ok_or_else(|| {
                anyhow!(
                    "Expected 'Ending Balance' to be set for the last record, got response:\n{:#?}",
                    bytes_clone
                )
            })?;
        }

        let record_clone = record.clone();
        transactions.push(record.try_into().with_context(|| {
            anyhow!(
                "Failed to convert TransactionRecord to Transaction: {:#?}",
                record_clone
            )
        })?);
    };

    Ok(Statement {
        beginning_balance,
        ending_balance,
        transactions,
    })
}
