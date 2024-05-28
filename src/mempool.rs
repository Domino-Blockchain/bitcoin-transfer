use std::str::FromStr;

use bdk::{bitcoin::Network, FeeRate};
use serde::Deserialize;
use serde_json::Number;

pub fn get_mempool_url() -> &'static str {
    let btc_network = Network::from_str(&std::env::var("BTC_NETWORK").unwrap()).unwrap();
    match btc_network {
        Network::Bitcoin => "https://mempool.space",
        Network::Testnet => "https://mempool.space/testnet", // https://mempool.space/testnet4
        Network::Signet => "https://mempool.space/signet",
        Network::Regtest => todo!(),
        _ => todo!(),
    }
}

pub fn get_mempool_ws_url() -> &'static str {
    let btc_network = Network::from_str(&std::env::var("BTC_NETWORK").unwrap()).unwrap();
    match btc_network {
        Network::Bitcoin => "wss://mempool.space/api/v1/ws",
        Network::Testnet => "wss://mempool.space/testnet/api/v1/ws", // wss://mempool.space/testnet4/api/v1/ws
        Network::Signet => "wss://mempool.space/signet/api/v1/ws",
        Network::Regtest => todo!(),
        _ => todo!(),
    }
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecommendedFeesResp {
    fastest_fee: Number,
    half_hour_fee: Number,
    hour_fee: Number,
    economy_fee: Number,
    minimum_fee: Number,
}

pub async fn get_recommended_fee_rate() -> FeeRate {
    let url = format!("{}/api/v1/fees/recommended", get_mempool_url());
    let resp: RecommendedFeesResp = reqwest::get(url).await.unwrap().json().await.unwrap();
    let recommended_fee = resp.fastest_fee;

    FeeRate::from_sat_per_vb(recommended_fee.as_f64().unwrap() as f32)
}

#[tokio::test]
async fn test_get_recommended_fee_rate() {
    std::env::set_var("BTC_NETWORK", "bitcoin");
    dbg!(get_recommended_fee_rate().await);
}
