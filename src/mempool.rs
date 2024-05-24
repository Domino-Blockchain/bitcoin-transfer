use std::str::FromStr;

use bdk::bitcoin::Network;

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
