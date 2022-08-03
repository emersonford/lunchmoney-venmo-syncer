use anyhow::bail;
use anyhow::Result;
use hyper::header::{AUTHORIZATION, CONTENT_TYPE};
use hyper::{body, Method, Request, StatusCode};

use crate::types::lunchmoney::{
    Asset, GetAllAssetsResponse, InsertTransactionRequest, InsertTransactionResponse, Transaction,
};
use crate::types::HttpsClient;

pub async fn get_all_assets(client: &HttpsClient, api_token: &str) -> Result<Vec<Asset>> {
    let request = Request::builder()
        .method(Method::GET)
        .uri("https://dev.lunchmoney.app/v1/assets")
        .header(AUTHORIZATION, format!("Bearer {}", api_token))
        .body(body::Body::empty())
        .unwrap();

    let response = client.request(request).await?;

    let status = response.status();
    let bytes = body::to_bytes(response).await?;

    if status != StatusCode::OK {
        bail!(
            "Failed to get Lunch Money assets, code {}, err:\n{:#?}",
            status,
            bytes
        );
    }

    let response: GetAllAssetsResponse = serde_json::from_slice(&bytes)?;

    Ok(response.assets)
}

pub async fn insert_transactions(
    client: &HttpsClient,
    api_token: &str,
    transactions: Vec<Transaction>,
) -> Result<Vec<u64>> {
    let request_body = InsertTransactionRequest {
        transactions,
        apply_rules: Some(true),
        check_for_recurring: Some(true),
        debit_as_negative: Some(true),
        skip_balance_update: None,
        skip_duplicates: None,
    };

    let request = Request::builder()
        .method(Method::POST)
        .uri("https://dev.lunchmoney.app/v1/transactions")
        .header(AUTHORIZATION, format!("Bearer {}", api_token))
        .header(CONTENT_TYPE, "application/json; charset=utf-8")
        .body(serde_json::to_vec(&request_body)?.into())
        .unwrap();

    let response = client.request(request).await?;

    let status = response.status();
    let bytes = body::to_bytes(response).await?;

    if status != StatusCode::OK {
        bail!(
            "Failed to insert Lunch Money transactions, code {}, err:\n{:#?}",
            status,
            bytes
        );
    }

    let response: InsertTransactionResponse = serde_json::from_slice(&bytes)?;

    Ok(response.ids)
}
