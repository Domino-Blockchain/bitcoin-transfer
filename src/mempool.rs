use std::str::FromStr;

use bdk::bitcoin::Network;

pub fn get_mempool_url() -> &'static str {
    let btc_network = Network::from_str(&std::env::var("BTC_NETWORK").unwrap()).unwrap();
    match btc_network {
        Network::Bitcoin => "https://mempool.space",
        Network::Testnet => "https://mempool.space/testnet",
        Network::Signet => todo!(),
        Network::Regtest => todo!(),
        _ => todo!(),
    }
}
