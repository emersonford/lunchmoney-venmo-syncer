use std::io::BufRead;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use chrono::{DateTime, Utc};
use dialoguer::{Confirm, Input, Password};
use hyper::header::{AUTHORIZATION, CONTENT_TYPE, COOKIE};
use hyper::{body, body::Buf, Method, Request, StatusCode};
use serde_json::{json, Value};

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

pub async fn cmd_get_venmo_api_token(client: &HttpsClient) -> Result<()> {
    println!("** TREAT VENMO API TOKENS LIKE YOUR VENMO PASSWORD, DO NOT SHARE IT WITH ANYONE AND KEEP IT SECURE. ANYONE WITH THIS API TOKEN HAS FULL ACCESS TO YOUR ACCOUNT, INCLUDING SENDING TRANSACTIONS. API TOKENS ARE NOT AUTOMATICALLY INVALIDATED, YOU MUST USE `logout-venmo-api-token` TO INVALIDATE THEM WHEN YOU ARE DONE WITH THEM. **\n");

    if !Confirm::new()
        .with_prompt("Do you understand the risk?")
        .default(false)
        .wait_for_newline(true)
        .interact()?
    {
        bail!("Risk was not acknowledged.");
    }

    let username: String = Input::new()
        .with_prompt("Venmo email/phone number")
        .interact_text()?;
    let password: String = Password::new().with_prompt("Venmo password").interact()?;

    let machine_id = machine_uid::get().unwrap();

    let request = json!({
        "phone_email_or_username": username,
        "client_id": "1",
        "password": password,
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("https://api.venmo.com/v1/oauth/access_token")
        .header("device-id", machine_id.clone())
        .header(CONTENT_TYPE, "application/json")
        .body(serde_json::to_vec(&request)?.into())
        .unwrap();

    let response = client.request(request).await?;

    let otp_secret = response.headers().get("venmo-otp-secret").cloned();
    let bytes = body::to_bytes(response).await?;
    let response: Value = serde_json::from_slice(&bytes)?;

    let api_token_response = if let Some(error) = response.get("error") {
        let message = if let Some(message) = error.get("message") {
            message.as_str().ok_or_else(|| {
                anyhow!(
                    "Failed to parse 'message' field, response was: {:?}",
                    response
                )
            })?
        } else {
            bail!(
                "Failed to get 'message' field, response was: {:?}",
                response
            );
        };

        if message == "Your email or password was incorrect." {
            bail!("Email or password was incorrect!");
        }

        if message != "Additional authentication is required" {
            bail!("Unknown response: {:?}", response);
        }

        let otp_secret = otp_secret.ok_or_else(|| {
            anyhow!("2FA required, but did not get venmo-otp-secret in header...")
        })?;

        println!("Two-factor auth required, using text message...");

        let twofa_request = json!({
            "via": "sms"
        });

        let twofa_request = Request::builder()
            .method(Method::POST)
            .uri("https://api.venmo.com/v1/account/two-factor/token")
            .header("device-id", machine_id.clone())
            .header(CONTENT_TYPE, "application/json")
            .header("venmo-otp-secret", otp_secret.clone())
            .body(serde_json::to_vec(&twofa_request)?.into())
            .unwrap();

        let twofa_response = client.request(twofa_request).await?;
        let twofa_bytes = body::to_bytes(twofa_response).await?;
        let twofa_response: Value = serde_json::from_slice(&twofa_bytes)?;

        if let Some(val) = twofa_response
            .get("data")
            .and_then(|data| data.get("status"))
        {
            if val != "sent" {
                bail!(
                    "Failed to request 2FA code, response was: {:?}",
                    twofa_response
                );
            }
        } else {
            bail!(
                "Failed to request 2FA code, response was: {:?}",
                twofa_response
            );
        }

        let twofa_code: String = Input::new().with_prompt("2FA code").interact_text()?;

        let request = json!({
            "phone_email_or_username": username,
            "client_id": "1",
            "password": password,
        });

        let twofa_submit_request = Request::builder()
            .method(Method::POST)
            .uri("https://api.venmo.com/v1/oauth/access_token?client_id=1")
            .header("device-id", machine_id)
            .header(CONTENT_TYPE, "application/json")
            .header("venmo-otp-secret", otp_secret)
            .header("Venmo-Otp", twofa_code)
            .body(serde_json::to_vec(&request)?.into())
            .unwrap();

        let twofa_submit_response = client.request(twofa_submit_request).await?;
        let twofa_submit_bytes = body::to_bytes(twofa_submit_response).await?;
        let twofa_submit_response: Value = serde_json::from_slice(&twofa_submit_bytes)?;

        if let Some(_error) = twofa_submit_response.get("error") {
            bail!(
                "Failed to confirm 2FA code, response was: {:?}",
                twofa_submit_response
            );
        }

        twofa_submit_response
    } else {
        response
    };

    let access_token = if let Some(token) = api_token_response.get("access_token") {
        token.as_str().ok_or_else(|| {
            anyhow!(
                "Failed to parse 'access_token' field, response was: {:?}",
                api_token_response
            )
        })?
    } else {
        bail!(
            "Did not get error but no 'access_token' field was found, response was: {:?}",
            api_token_response
        );
    };

    let profile_id = if let Some(id) = api_token_response
        .get("user")
        .and_then(|user| user.get("id"))
    {
        id.as_str().ok_or_else(|| {
            anyhow!(
                "Failed to parse user.id, response was: {:?}",
                api_token_response
            )
        })?
    } else {
        bail!(
            "Did not get error but no 'user.id' field was found, response was: {:?}",
            api_token_response
        );
    };

    println!("Venmo profile ID: {}", profile_id);
    println!("Venmo API token: {}", access_token);

    Ok(())
}

pub async fn cmd_logout_venmo_api_token(client: &HttpsClient, api_token: &str) -> Result<()> {
    let request = Request::builder()
        .method(Method::DELETE)
        .uri("https://api.venmo.com/v1/oauth/access_token")
        .header(AUTHORIZATION, api_token)
        .body(body::Body::empty())
        .unwrap();

    let response = client.request(request).await?;
    let bytes = body::to_bytes(response).await?;
    let response: Value = serde_json::from_slice(&bytes)?;

    println!("Response: {:?}", response);
    Ok(())
}
