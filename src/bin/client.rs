use anyhow::Ok;
use bdk::{Balance, TransactionDetails};
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
        let data = response.json().await?;
        Ok(data)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = "0.0.0.0:3000".to_string();

    let service = Service::new(addr);

    match std::env::args().nth(1).unwrap().as_str() {
        "get_address" => {
            dbg!(service.post::<String>("/get_address").await?);
        }
        "check_balance" => {
            dbg!(service.post::<Balance>("/check_balance").await?);
        }
        "check_destination_balance" => {
            dbg!(
                service
                    .post::<Balance>("/check_destination_balance")
                    .await?
            );
        }
        "mint_token" => {
            let data = service.post::<serde_json::Value>("/mint_token").await?;
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        "burn_token" => {
            let address = std::env::args().nth(2).unwrap();
            let data = service
                .post_json::<serde_json::Value>("/burn_token", json!(address))
                .await?;
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        "send_btc_to_user" => {
            dbg!(
                service
                    .post::<TransactionDetails>("/send_btc_to_user")
                    .await?
            );
        }
        _ => panic!(),
    }
    Ok(())
}
