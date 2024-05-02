// mod spl_token_cli_lib;

mod balance_by_addresses;
mod bdk_cli;
mod db;
mod get_address;
mod get_mint_info;
mod log_progress;
mod mint_token;
mod sign_multisig_tx;
mod spl_token;
mod watch_addresses;
mod watch_tx;

use std::sync::Arc;
use std::time::Duration;

use axum::http::{self, HeaderValue, Method};
use axum::routing::post;
use axum::Json;
use axum::Router;
use bdk::bitcoin::{Address, Network};
use bdk::blockchain::{Blockchain, ElectrumBlockchain};
use bdk::database::MemoryDatabase;
use bdk::electrum_client::Client;
use bdk::keys::{
    bip39::{Language, Mnemonic, WordCount},
    DerivableKey, ExtendedKey, GeneratableKey, GeneratedKey,
};
use bdk::template::Bip84;
use bdk::wallet::AddressIndex::{LastUnused, Peek};
use bdk::{
    miniscript, Balance, KeychainKind, SignOptions, SyncOptions, TransactionDetails, Wallet,
};
use kms_sign::load_dotenv;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tower_http::cors::{any, CorsLayer};

use crate::balance_by_addresses::get_known_addresses;
use crate::db::DB;
use crate::get_address::get_address_from_db;
use crate::get_mint_info::get_mint_info;
use crate::mint_token::mint_token;
use crate::sign_multisig_tx::sign_multisig_tx;
use crate::spl_token::spl_token;
use crate::watch_addresses::watch_addresses;
use crate::watch_tx::watch_tx;

// e:0:tb1q6dsqge320xzu7g64d5arp4qx6ldvz6xd27zvgy:0
// e:1:tb1qsvsqza56mdcmp8d02ttq06grdrcjmtcnxd08pf:779
// e:2:tb1q2kpgx8474rkttkxl9yq6e8e06u9egw7ep2k4vf:0
// e:3:tb1q2p9nlkfkjpx68ex24uv2cau4rdjy0ft7qxwjl0:0
// i:0:tb1q5fyz7lm2xmvlj0808lzytlavu487qhwc4n7m4v:2302
// i:1:tb1q255gg70ev9xhld0uywz2w6knnrvlalkju44td9:0
// i:2:tb1q4ve39l7gn8zyr8zf2luxfn3legtd777j2fcjw4:418
// i:3:tb1qqjahfdv2hlyvfghnmnzddwwsv5ww6uv582cvwc:0
const SERVICE_ADDRESS: [u8; 32] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2,
];

// e:0:tb1q76ksqtcp3ph20ne5rlduz3xvvc0ysv6chj8w3n:841
// e:1:tb1q8y067gys5k6pm8gdcpnq4evf6z477h9fkpcdl3:0
// i:0:tb1qqph07frjg6fng2dwr2hkz9dh66katry8a28hv8:0
const USER_ADDRESS: [u8; 32] = [
    10, 20, 30, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2,
    3,
];

fn mnemonic_from_entropy(entropy: [u8; 32]) -> GeneratedKey<Mnemonic, miniscript::Segwitv0> {
    Mnemonic::generate_with_entropy((WordCount::Words12, Language::English), entropy).unwrap()
}

#[derive(Clone)]
struct AppState {
    db: Arc<DB>,
}

impl AppState {
    fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
}

#[tokio::main]
async fn main() {
    load_dotenv();

    let allow_origin = std::env::var("ALLOW_ORIGIN")
        .unwrap_or_else(|_| "http://devnet.domichain.io:3000".to_string());

    // DB::test().await.unwrap();
    let db = Arc::new(DB::new().await);
    let db_clone = Arc::clone(&db);

    let ws_handle = tokio::spawn(async move {
        let all_multisig_addresses = db_clone.get_all_multisig_addresses().await;
        dbg!(&all_multisig_addresses);
        dbg!(all_multisig_addresses.len());
        // vec!["tb1qalaejg4ve63htr8pxfr9l76cq8qqq52pgrevwy2vdqywsxlxegesh0mh6n"]

        for (i, chunk) in all_multisig_addresses.chunks(10).enumerate() {
            let chunk: Vec<_> = chunk.into_iter().cloned().collect();
            tokio::spawn(async move {
                // watch_addresses(i, chunk, todo!(), todo!(), todo!()).await;
            });
            sleep(Duration::from_secs(2)).await;
        }
    });

    let app = Router::new()
        .route(
            "/get_address",
            post(|| async { Json(get_new_service_address().await.to_string()) }),
        )
        .route("/get_address_from_db", post(get_address_from_db))
        .route("/watch_tx", post(watch_tx))
        .route("/get_mint_info", post(get_mint_info))
        .route("/sign_multisig_tx", post(sign_multisig_tx))
        .route("/check_balance", post(check_balance))
        .route("/mint_token", post(mint_token))
        .route("/burn_token", post(burn_token))
        .route("/send_btc_to_user", post(send_btc_to_user))
        .route(
            "/check_destination_balance",
            post(check_destination_balance),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(any())
                // .allow_origin(allow_origin.parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(vec![http::header::CONTENT_TYPE]),
        )
        .with_state(AppState::new(db.into()));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await.unwrap();
    println!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    ws_handle.await.unwrap();
}

#[derive(Deserialize)]
struct BurnTokenRequest {
    account_address: String,
    amount: u64,
}
async fn burn_token(Json(request): Json<BurnTokenRequest>) -> Json<serde_json::Value> {
    Json(spl_token(&[
        "burn",
        &request.account_address,
        &request.amount.to_string(),
    ]))
}

async fn check_balance() -> Json<Balance> {
    let mnemonic = mnemonic_from_entropy(SERVICE_ADDRESS);
    check_balance_by_mnemonic(mnemonic).await
}

async fn check_destination_balance() -> Json<Balance> {
    let mnemonic = mnemonic_from_entropy(USER_ADDRESS);
    check_balance_by_mnemonic(mnemonic).await
}

async fn get_synched_wallet_from_mnemonic(
    mnemonic: GeneratedKey<Mnemonic, miniscript::Segwitv0>,
) -> (Wallet<MemoryDatabase>, ElectrumBlockchain) {
    let network = Network::Testnet; // Or this can be Network::Bitcoin, Network::Signet or Network::Regtest

    // Convert mnemonic to string
    let mnemonic_words = mnemonic.to_string();
    // Parse a mnemonic
    let mnemonic = Mnemonic::parse(&mnemonic_words).unwrap();
    // Generate the extended key
    let xkey: ExtendedKey = mnemonic.into_extended_key().unwrap();
    // Get xprv from the extended key
    let xprv = xkey.into_xprv(network).unwrap();

    // tb1qedg9fdlf8cnnqfd5mks6uz5w4kgpk2pr6y4qc7
    // let key = bitcoin::util::bip32::ExtendedPubKey::from_str("tpubDC2Qwo2TFsaNC4ju8nrUJ9mqVT3eSgdmy1yPqhgkjwmke3PRXutNGRYAUo6RCHTcVQaDR3ohNU9we59brGHuEKPvH1ags2nevW5opEE9Z5Q").unwrap();
    // let fingerprint = bitcoin::util::bip32::Fingerprint::from_str("c55b303a").unwrap();

    // Create a BDK wallet structure using BIP 84 descriptor ("m/84h/1h/0h/0" and "m/84h/1h/0h/1")
    let wallet = Wallet::new(
        // Bip84Public(key.clone(), fingerprint, KeychainKind::External),
        // Some(Bip84Public(key, fingerprint, KeychainKind::Internal)),
        Bip84(xprv, KeychainKind::External),
        Some(Bip84(xprv, KeychainKind::Internal)),
        network,
        MemoryDatabase::default(),
    )
    .unwrap();

    let client = Client::new("ssl://electrum.blockstream.info:60002").unwrap();
    let blockchain = ElectrumBlockchain::from(client);
    wallet
        .sync(
            &blockchain,
            SyncOptions {
                progress: Some(Box::new(log_progress::log_progress())),
            },
        )
        .unwrap();

    let addresses = get_known_addresses(&wallet, &blockchain);
    println!();
    addresses.iter().for_each(|s| println!("{s}"));
    println!();

    (wallet, blockchain)
}

async fn check_balance_by_mnemonic(
    mnemonic: GeneratedKey<Mnemonic, miniscript::Segwitv0>,
) -> Json<Balance> {
    let (wallet, blockchain) = get_synched_wallet_from_mnemonic(mnemonic).await;
    let balance = wallet.get_balance().unwrap();
    Json(balance)
}

async fn get_new_service_address() -> Address {
    let network = Network::Testnet; // Or this can be Network::Bitcoin, Network::Signet or Network::Regtest

    // Generate fresh mnemonic
    // tb1q6dsqge320xzu7g64d5arp4qx6ldvz6xd27zvgy
    let mnemonic = mnemonic_from_entropy(SERVICE_ADDRESS);

    let (wallet, blockchain) = get_synched_wallet_from_mnemonic(mnemonic).await;

    let address = wallet.get_address(LastUnused).unwrap();
    address.address
}

async fn get_destination_address_0() -> Address {
    let network = Network::Testnet; // Or this can be Network::Bitcoin, Network::Signet or Network::Regtest

    // Generate fresh mnemonic
    // tb1q76ksqtcp3ph20ne5rlduz3xvvc0ysv6chj8w3n
    let mnemonic = mnemonic_from_entropy(USER_ADDRESS);

    let (wallet, _blockchain) = get_synched_wallet_from_mnemonic(mnemonic).await;

    let address = wallet.get_address(Peek(0)).unwrap();
    address.address
}

async fn send_btc_to_user() -> Json<TransactionDetails> {
    // OUR: tb1q2p9nlkfkjpx68ex24uv2cau4rdjy0ft7qxwjl0, SEND_TO: tb1q76ksqtcp3ph20ne5rlduz3xvvc0ysv6chj8w3n

    // tb1q2p9nlkfkjpx68ex24uv2cau4rdjy0ft7qxwjl0
    let mnemonic = mnemonic_from_entropy(SERVICE_ADDRESS);

    let (wallet, blockchain) = get_synched_wallet_from_mnemonic(mnemonic).await;

    let (mut psbt, details) = {
        // tb1q76ksqtcp3ph20ne5rlduz3xvvc0ysv6chj8w3n
        let recipient = get_destination_address_0().await;
        assert_eq!(
            recipient.to_string().as_str(),
            "tb1q76ksqtcp3ph20ne5rlduz3xvvc0ysv6chj8w3n"
        );
        let mut builder = wallet.build_tx();
        let dust_amount = recipient.script_pubkey().dust_value();
        builder.add_recipient(recipient.script_pubkey(), dust_amount.to_sat());
        builder.finish().unwrap()
    };

    let finalized = wallet.sign(&mut psbt, SignOptions::default()).unwrap();
    assert!(finalized);

    let tx = psbt.extract_tx();
    blockchain.broadcast(&tx).unwrap();

    Json(details)
}

pub fn serde_convert<F, T>(a: F) -> T
where
    F: Serialize,
    T: DeserializeOwned,
{
    let string = serde_json::to_string(&a).unwrap();
    serde_json::from_str(&string).unwrap()
}
