use std::time::Duration;

use anyhow::anyhow;
use anyhow::Result;
use chrono::offset::{Local, Utc};
use chrono::DateTime;
use clap::{Args, Parser, Subcommand};
use hyper::client::Client;
use hyper_tls::HttpsConnector;
use itertools::Itertools;

mod lunchmoney;
mod types;
mod venmo;

use lunchmoney::{get_all_assets, insert_transactions};
use types::venmo::AccountRecord;
use types::HttpsClient;
use venmo::fetch_venmo_transactions;

#[derive(Args)]
struct ListVenmoTransactionsArgs {
    #[clap(long, value_parser = humantime::parse_duration, default_value = "30d")]
    start_from: Duration,

    #[clap(long, value_parser = humantime::parse_duration)]
    end_to: Option<Duration>,

    #[clap(long)]
    profile_id: u64,

    #[clap(long)]
    api_token: String,

    #[clap(long, default_value = "USD")]
    currency: String,
}

async fn cmd_list_venmo_transactions(
    client: &HttpsClient,
    args: ListVenmoTransactionsArgs,
) -> Result<()> {
    let end_date: DateTime<Utc> = {
        let mut end_date = Local::now();

        if let Some(duration) = args.end_to {
            end_date = end_date - chrono::Duration::from_std(duration).unwrap();
        }

        end_date.into()
    };

    let start_date: DateTime<Utc> =
        (Local::now() - chrono::Duration::from_std(args.start_from).unwrap()).into();

    let account = AccountRecord {
        profile_id: args.profile_id,
        api_token: args.api_token.clone(),
        currency: *rusty_money::iso::find(&args.currency)
            .ok_or_else(|| anyhow!("Given currency {} is not valid", args.currency))?,
    };

    let transactions = fetch_venmo_transactions(client, &account, &start_date, &end_date).await?;

    println!("{:#?}", transactions);

    Ok(())
}

async fn cmd_list_lunch_money_assets(client: &HttpsClient, api_token: String) -> Result<()> {
    let assets = get_all_assets(client, &api_token).await?;

    println!("{:#?}", assets);

    Ok(())
}

#[derive(Args)]
struct SyncVenmoTransactionsArgs {
    #[clap(long, value_parser = humantime::parse_duration, default_value = "30d")]
    start_from: Duration,

    #[clap(long, value_parser = humantime::parse_duration)]
    end_to: Option<Duration>,

    #[clap(long)]
    venmo_profile_id: u64,

    #[clap(long)]
    venmo_api_token: String,

    #[clap(long)]
    lunch_money_api_token: String,

    #[clap(long)]
    lunch_money_asset_id: u64,

    #[clap(long, default_value = "USD")]
    currency: String,
}

async fn cmd_sync_venmo_transactions(
    client: &HttpsClient,
    args: SyncVenmoTransactionsArgs,
) -> Result<()> {
    let end_date: DateTime<Utc> = {
        let mut end_date = Local::now();

        if let Some(duration) = args.end_to {
            end_date = end_date - chrono::Duration::from_std(duration).unwrap();
        }

        end_date.into()
    };

    let start_date: DateTime<Utc> =
        (Local::now() - chrono::Duration::from_std(args.start_from).unwrap()).into();

    let currency = rusty_money::iso::find(&args.currency)
        .ok_or_else(|| anyhow!("Given currency {} is not valid", args.currency))?;

    let venmo_account = AccountRecord {
        profile_id: args.venmo_profile_id,
        api_token: args.venmo_api_token.clone(),
        currency: *currency,
    };

    let venmo_transactions =
        fetch_venmo_transactions(client, &venmo_account, &start_date, &end_date).await?;

    println!(
        "Beginning balance: {}",
        venmo_transactions.beginning_balance
    );
    println!("Ending balance: {}", venmo_transactions.ending_balance);

    let lunchmoney_transactions = venmo_transactions
        .transactions
        .into_iter()
        .map(|transaction| {
            transaction.to_lunchmoney_transactions(*currency, args.lunch_money_asset_id)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten();

    // println!("syncing:\n{:#?}", lunchmoney_transactions);

    let mut synced_transactions: Vec<u64> = Vec::new();

    for transaction_chunk in &lunchmoney_transactions.into_iter().chunks(50) {
        synced_transactions.extend(
            insert_transactions(
                client,
                &args.lunch_money_api_token,
                transaction_chunk.collect(),
            )
            .await?,
        );
    }

    println!("inserted transactions: {:?}", synced_transactions);

    Ok(())
}

/// A CLI to sync Venmo transactions to Lunch Money, using the unofficial Venmo API.
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cmd {
    #[clap(subcommand)]
    verb: Verb,
}

#[derive(Subcommand)]
enum Verb {
    /// List Venmo transactions for a given time period.
    ListVenmoTransactions(ListVenmoTransactionsArgs),

    /// List assets for your Lunch Money account, used to get the asset ID you care about.
    ListLunchMoneyAssets {
        #[clap(long)]
        api_token: String,
    },

    /// Sync Venmo transactions to Lunch Money asset.
    SyncVenmoTransactions(SyncVenmoTransactionsArgs),

    /// Get a Venmo API token for syncing use.
    GetVenmoApiToken,

    /// Invalidate an existing Venmo API token.
    LogoutVenmoApiToken {
        /// The API token to invalidate
        api_token: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cmd = Cmd::parse();

    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);

    match cmd.verb {
        Verb::ListVenmoTransactions(args) => cmd_list_venmo_transactions(&client, args).await,
        Verb::ListLunchMoneyAssets { api_token } => {
            cmd_list_lunch_money_assets(&client, api_token).await
        }
        Verb::SyncVenmoTransactions(args) => cmd_sync_venmo_transactions(&client, args).await,
        Verb::GetVenmoApiToken => venmo::cmd_get_venmo_api_token(&client).await,
        Verb::LogoutVenmoApiToken { api_token } => {
            venmo::cmd_logout_venmo_api_token(&client, &api_token).await
        }
    }
}
