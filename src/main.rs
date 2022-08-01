use anyhow::Result;
use clap::Parser;

mod types;

/// A CLI to sync Venmo transactions to Lunch Money, using the unofficial Venmo API.
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("Hello, world!");

    Ok(())
}
