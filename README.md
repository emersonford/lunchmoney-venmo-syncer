# lunchmoney-venmo-syncer
A Rust CLI to sync [Venmo](https://venmo.com) transactions with the [Lunch Money budget app](https://lunchmoney.app).

**This project is a work in progress and requires a Venmo API token that grants full access to your Venmo account (including sending transactions), USE THIS PROJECT AT YOUR OWN RISK.**

## Description
This project uses a previously documented [Venmo API](https://github.com/mmohades/VenmoApiDocumentation) to get a Venmo API token, which is then used to pull Venmo statements. This statement is then read and each listed Venmo transaction is converted to a [Lunch Money transaction](https://lunchmoney.dev/#transaction-object) and sent to Lunch Money. When necessary, additional transactions are generated to indicate when money was transfered to/from a bank account to your Venmo balance (or used to cover a Venmo payment).

The ultimate goal of this is to allow Lunch Money to treat your Venmo wallet almost as if its a "native" bank account in the app.

## Setup
### Lunch Money
1. Create a "manually-managed asset" in Lunch Money to sync your Venmo transactions to. You can do this on the [accounts page](https://my.lunchmoney.app/accounts) -> "Add Account" -> Scroll down to the "manually-managed assets" section and select "Cash" -> Select "Digital wallet (paypal, venmo)" -> configure the name as you desire.
2. Generate a Lunch Money API key. Go to the [developer page](https://my.lunchmoney.app/developers) and select "Request New Access Token". Copy this token to somewhere secure for later use. TREAT THIS TOKEN AS IF IT WERE A PASSWORD.

### Project Setup
1. Setup the Rust toolchain locally. I recommend using [rustup.rs](https://rustup.rs). You should now be able to run `cargo` in your terminal.
2. Clone this repo somewhere and `cd` to it, e.g. `git clone https://github.com/emersonford/lunchmoney-venmo-syncer.git && cd lunchmoney-venmo-syncer`.
3. Run `cargo run -- get-venmo-api-token` and follow the instructions to generate a Venmo API token and get your Venmo profile ID. Copy this token and ID to somewhere secure for later use. TREAT THIS TOKEN AS IF IT WERE YOUR VENMO PASSWORD.
    * You can later invalidate this Venmo API token, if you wish, with `cargo run -- logout-venmo-api-token <VENMO_API_TOKEN>`.
4. Run `cargo run -- list-lunch-money-assets --api-token <LUNCHMONEY_API_TOKEN>` where `<LUNCHMONEY_API_TOKEN>` is the Lunch Money API token you generated earlier. Find the asset corresponding to the "manually-managed asset" you created earlier and make note of the ID of that asset.


## Running the Command
The core command is `sync-venmo-transaction`:
```
❯ cargo run -- sync-venmo-transactions --help
lunchmoney-venmo-sync-venmo-transactions
Sync Venmo transactions to Lunch Money asset

USAGE:
    lunchmoney-venmo sync-venmo-transactions [OPTIONS] --venmo-profile-id <VENMO_PROFILE_ID> --venmo-api-token <VENMO_API_TOKEN> --lunch-money-api-token <LUNCH_MONEY_API_TOKEN> --lunch-money-asset-id <LUNCH_MONEY_ASSET_ID>

OPTIONS:
        --currency <CURRENCY>                              [default: USD]
        --end-to <END_TO>
    -h, --help                                             Print help information
        --lunch-money-api-token <LUNCH_MONEY_API_TOKEN>
        --lunch-money-asset-id <LUNCH_MONEY_ASSET_ID>
        --start-from <START_FROM>                          [default: 30d]
        --venmo-api-token <VENMO_API_TOKEN>
        --venmo-profile-id <VENMO_PROFILE_ID>
```

Here you pass in the Venmo API token you generated, the Venmo profile ID that was printed to you, the Lunch Money API token, and asset ID. You can configure the date range you want to sync by also setting `--start-from DATE`.

The output of this command will tell you the beginning/ending balance of your Venmo wallet for this date range (which you can then set in your Lunch Money asset) and the transactions that were newly synced to Lunch Money. For example,

```
❯ cargo run -- sync-venmo-transactions --lunch-money-api-token your_lunch_money_api_token --lunch-money-asset-id 123yourassetid456 --venmo-api-token your_venmo_api_token --venmo-profile-id 123yourvenmoprofileid456
...

Beginning balance: $390.0000
Ending balance: $50.8900
inserted transactions: [111820582, 111820583, 111820584, 111820585, 111820586, 111820587, 111820588, 111820589, 111820590, 111820591, 111820592, 111820593, 111820594, 111820595, 111820596, 111820597, 111820598, 111820599, 111820600, 111820601, 111820602, 111820603, 111820604, 111820605, 111820606, 111820607, 111820608, 111820609, 111820610, 111820611, 111820612, 111820613, 111820614, 111820615, 111820616, 111820617, 111820618, 111820619, 111820620, 111820621, 111820622, 111820623, 111820624, 111820625, 111820626, 111820627, 111820628, 111820629, 111820630, 111820631, 111820632, 111820633, 111820634]
```

