use std::process::Stdio;

use axum::routing::post;
use axum::Json;
use axum::{routing::get, Router};
use bdk::bitcoin::Network;
use bdk::blockchain::ElectrumBlockchain;
use bdk::database::MemoryDatabase;
use bdk::electrum_client::Client;
use bdk::keys::{
    bip39::{Language, Mnemonic, WordCount},
    DerivableKey, ExtendedKey, GeneratableKey, GeneratedKey,
};
use bdk::template::Bip84;
use bdk::wallet::AddressIndex::New;
use bdk::{miniscript, Balance, KeychainKind, SyncOptions, Wallet};

fn _main_btc() {
    let network = Network::Testnet; // Or this can be Network::Bitcoin, Network::Signet or Network::Regtest

    // Generate fresh mnemonic
    let mnemonic: GeneratedKey<_, miniscript::Segwitv0> = Mnemonic::generate_with_entropy(
        (WordCount::Words12, Language::English),
        [
            1, 2, 3, 4, 5, 6, 7, 8, 9,
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
            0, 1, 2,
        ],
    )
    .unwrap();
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
    wallet.sync(&blockchain, SyncOptions::default()).unwrap();

    println!(
        "mnemonic: {}\n\nrecv desc (pub key): {:#?}\n\nchng desc (pub key): {:#?}",
        mnemonic_words,
        wallet
            .get_descriptor_for_keychain(KeychainKind::External)
            .to_string(),
        wallet
            .get_descriptor_for_keychain(KeychainKind::Internal)
            .to_string()
    );

    println!("Address #0: {}", wallet.get_address(New).unwrap());
    println!("Descriptor balance: {} SAT", wallet.get_balance().unwrap());
}

#[tokio::main]
async fn main() {
    // build our application with a route
    let app = Router::new()
        .route("/get_address", get(get_address))
        .route("/check_balance", post(check_balance))
        .route("/mint_token", post(mint_token))
        ;

    // run it
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    println!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn mint_token() -> Json<serde_json::Value> {
    let mut c = std::process::Command::new("/home/zotho/DOMI/BUILD_VERIFY_3/domichain-program-library/target/release/spl-token");
    let command = c
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("create-token")
        .arg("--output")
        .arg("json")
        ;
    let o = command.spawn().unwrap().wait_with_output();
    let o = o.unwrap().stdout;
    Json::from_bytes(&o).unwrap()
}

async fn check_balance() -> Json<Balance> {
    let network = Network::Testnet; // Or this can be Network::Bitcoin, Network::Signet or Network::Regtest

    // Generate fresh mnemonic
    let mnemonic: GeneratedKey<_, miniscript::Segwitv0> = Mnemonic::generate_with_entropy(
        (WordCount::Words12, Language::English),
        [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
            0, 1, 2,
        ],
    )
    .unwrap();
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
    wallet.sync(&blockchain, SyncOptions::default()).unwrap();

    let balance = wallet.get_balance().unwrap();
    Json(balance)
}

async fn get_address() -> Json<String> {
    let network = Network::Testnet; // Or this can be Network::Bitcoin, Network::Signet or Network::Regtest

    // Generate fresh mnemonic
    let mnemonic: GeneratedKey<_, miniscript::Segwitv0> = Mnemonic::generate_with_entropy(
        (WordCount::Words12, Language::English),
        [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
            0, 1, 2,
        ],
    )
    .unwrap();
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
    let address = wallet.get_address(New).unwrap();
    Json(address.to_string())
}
