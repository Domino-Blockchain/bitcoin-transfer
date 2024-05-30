use std::{str::FromStr, time::Instant};

use bdk::{bitcoin::Network, FeeRate};
use cached::{proc_macro::cached, Return};
use serde::Deserialize;
use serde_json::Number;
use tracing::debug;

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

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedFeesResp {
    pub fastest_fee: Number,
    pub half_hour_fee: Number,
    pub hour_fee: Number,
    pub economy_fee: Number,
    pub minimum_fee: Number,
}

#[cached(time = 10, sync_writes = true, with_cached_flag = true)]
pub async fn get_recommended_fee_rates(mempool_url: &'static str) -> Return<RecommendedFeesResp> {
    let url = format!("{mempool_url}/api/v1/fees/recommended");
    let start = Instant::now();
    let resp: RecommendedFeesResp = reqwest::get(url).await.unwrap().json().await.unwrap();
    debug!("get_recommended_fee_rates took: {:?}", start.elapsed());

    Return::new(resp)
}

pub async fn get_recommended_fee_rate() -> FeeRate {
    let cached_return = get_recommended_fee_rates(get_mempool_url()).await;
    debug!(
        "get_recommended_fee_rates was_cached: {:?}",
        cached_return.was_cached
    );
    let fee_rates = cached_return.value;

    let recommended_fee = fee_rates.fastest_fee;
    FeeRate::from_sat_per_vb(recommended_fee.as_f64().unwrap() as f32)
}

#[tokio::test]
async fn test_get_recommended_fee_rate() {
    std::env::set_var("BTC_NETWORK", "bitcoin");
    dbg!(get_recommended_fee_rate().await);
}
