use anyhow::Ok;
use bdk::{Balance, TransactionDetails};
use clap::{arg, command, value_parser, Command};
use domichain_program::pubkey::Pubkey;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

struct Service {
    addr: String,
}

impl Service {
    fn new(addr: String) -> Self {
        Self { addr }
    }

    async fn post<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let response = reqwest::Client::new()
            .post(format!("http://{addr}{path}", addr = self.addr))
            .send()
            .await?;
        let data = response.json().await?;
        Ok(data)
    }

    async fn post_json<T: DeserializeOwned>(&self, path: &str, json: Value) -> anyhow::Result<T> {
        let response = reqwest::Client::new()
            .post(format!("http://{addr}{path}", addr = self.addr))
            .json(&json)
            .send()
            .await?;
        // dbg!(&response);
        // let text = response.text().await?;
        // dbg!(text);
        // todo!()

        // let raw_data: serde_json::Value = response.json().await?;
        // let data = serde_json::from_value(raw_data)?;

        let data = response.json().await?;
        Ok(data)
    }
}

fn parse_pubkey(raw_address: &str) -> Result<Pubkey, String> {
    Pubkey::try_from(raw_address).map_err(|err| err.to_string())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = command!()
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("get_address").about("Get unused BTC address"))
        .subcommand(Command::new("check_balance").about("Check BTC balance"))
        .subcommand(Command::new("check_destination_balance").about("Check BTC balance of user"))
        .subcommand(
            Command::new("mint_token")
                .about("Mint BTCi token")
                .arg(arg!(<AMOUNT>).value_parser(value_parser!(u64))),
        )
        .subcommand(
            Command::new("burn_token")
                .about("Burn BTCi token")
                .arg(arg!(<ADDRESS>).value_parser(parse_pubkey))
                .arg(arg!(<AMOUNT>).value_parser(value_parser!(u64))),
        )
        .subcommand(Command::new("send_btc_to_user").about("Send BTC to user"))
        .get_matches();

    let addr = "0.0.0.0:3001".to_string();
    let service = Service::new(addr);

    match matches.subcommand() {
        Some(("get_address", sub_matches)) => {
            println!("{}", service.post::<String>("/get_address").await?);
        }
        Some(("check_balance", sub_matches)) => {
            println!("{:#?}", service.post::<Balance>("/check_balance").await?);
        }
        Some(("check_destination_balance", sub_matches)) => {
            println!(
                "{:#?}",
                service
                    .post::<Balance>("/check_destination_balance")
                    .await?
            );
        }
        Some(("mint_token", sub_matches)) => {
            let amount = *sub_matches.get_one::<u64>("AMOUNT").unwrap();
            let data = service
                .post_json::<serde_json::Value>("/mint_token", json!({"amount": amount}))
                .await?;
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        Some(("burn_token", sub_matches)) => {
            let address = sub_matches.get_one::<Pubkey>("ADDRESS").unwrap();
            let amount = *sub_matches.get_one::<u64>("AMOUNT").unwrap();
            let data = service
                .post_json::<serde_json::Value>(
                    "/burn_token",
                    json!({
                        "account_address": address.to_string(),
                        "amount": amount,
                    }),
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        Some(("send_btc_to_user", sub_matches)) => {
            println!(
                "{:#?}",
                service
                    .post::<TransactionDetails>("/send_btc_to_user")
                    .await?
            );
        }
        _ => unreachable!("Exhausted list of subcommands and subcommand_required prevents `None`"),
    }
    Ok(())
}
