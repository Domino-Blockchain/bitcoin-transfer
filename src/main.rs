// mod spl_token_cli_lib;

mod balance_by_addresses;
mod log_progress;

use std::process::Stdio;
use std::str::FromStr;
use std::time::Instant;

use axum::http::{self, HeaderValue, Method};
use axum::routing::post;
use axum::Json;
use axum::{routing::get, Router};
use bdk::bitcoin::{Address, Network};
use bdk::bitcoincore_rpc::RawTx;
use bdk::blockchain::{Blockchain, ElectrumBlockchain, GetHeight};
use bdk::database::MemoryDatabase;
use bdk::electrum_client::Client;
use bdk::keys::{
    bip39::{Language, Mnemonic, WordCount},
    DerivableKey, ExtendedKey, GeneratableKey, GeneratedKey,
};
use bdk::template::Bip84;
use bdk::wallet::AddressIndex::{self, LastUnused, New, Peek};
use bdk::wallet::AddressInfo;
use bdk::{
    miniscript, Balance, KeychainKind, SignOptions, SyncOptions, TransactionDetails, Wallet,
};
use domichain_program::pubkey::Pubkey;
use ron::extensions::Extensions;
use ron::Options;
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::CorsLayer;

use crate::balance_by_addresses::{get_balance_by_address, get_known_addresses};

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

fn _main_btc() {
    let network = Network::Testnet; // Or this can be Network::Bitcoin, Network::Signet or Network::Regtest

    // Generate fresh mnemonic
    let mnemonic = mnemonic_from_entropy(SERVICE_ADDRESS);
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
    let app = Router::new()
        .route(
            "/get_address",
            post(|| async { Json(get_new_service_address().await.to_string()) }),
        )
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
                .allow_origin("http://193.107.109.22:3000".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(vec![http::header::CONTENT_TYPE]),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    println!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

const SPL_TOKEN_CLI_PATH: &str =
    "/home/zotho/DOMI/BUILD_VERIFY_3/domichain-program-library/target_0/release/spl-token";

fn spl_token(args: &[&str]) -> serde_json::Value {
    // TODO: use spl-token library to create token
    let mut c = std::process::Command::new(SPL_TOKEN_CLI_PATH);
    let command = c
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--output")
        .arg("json")
        .args(args);
    let o = command.spawn().unwrap().wait_with_output().unwrap();
    let stdout = o.stdout;
    let stderr = o.stderr;
    // if !stdout.is_empty() {
    //     println!("stdout = {}", String::from_utf8_lossy(&stdout));
    // }
    if !stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&stderr);
        println!("stderr = {stderr}");
        // let stderr = stderr
        //     .trim()
        //     .strip_prefix("Error: Client(Error ")
        //     .unwrap()
        //     .strip_suffix(")")
        //     .unwrap();
        // println!("stderr = {stderr:?}");
        // let options = Options::default().with_default_extension(Extensions::EXPLICIT_STRUCT_NAMES);
        // let stderr: ron::Value = match options.from_str(&stderr) {
        //     Ok(val) => val,
        //     Err(err) => {
        //         stderr.lines().for_each(|line| {
        //             dbg!(line.get(err.position.col - 10..err.position.col + 10));
        //         });
        //         dbg!(&err);
        //         panic!("ERR: {err:?}");
        //     }
        // };
        // println!(
        //     "stderr = {}",
        //     ron::ser::to_string_pretty(&stderr, ron::ser::PrettyConfig::default()).unwrap(),
        // );
    }
    serde_json::Value::from_str(std::str::from_utf8(&stdout).unwrap()).unwrap()
}

fn spl_token_plain(args: &[&str]) {
    // TODO: use spl-token library to create token
    let mut c = std::process::Command::new(SPL_TOKEN_CLI_PATH);
    let command = c.stdout(Stdio::piped()).stderr(Stdio::piped()).args(args);
    let o = command.spawn().unwrap().wait_with_output().unwrap();
    let stdout = o.stdout;
    let stderr = o.stderr;
    // if !stdout.is_empty() {
    //     println!("stdout = {}", String::from_utf8_lossy(&stdout));
    // }
    if !stderr.is_empty() {
        println!("stderr = {}", String::from_utf8_lossy(&stderr));
    }
}

async fn get_account_address(token_address: Pubkey) -> Pubkey {
    let token_program_id =
        Pubkey::from_str("7t5SuBhmxxKuQyjwTnmPpFpqJurCDM4dvM14nUGiza4s").unwrap();
    let associated_token_program_id =
        Pubkey::from_str("Dt8fRCpjeV6JDemhPmtcTKijgKdPxXHn9Wo9cXY5agtG").unwrap();
    // owner == Fk2HRYuDw9h29yKs1tNDjvjdvYMqQ2dGg9sS4JhUzQ6w
    let owner =
        Pubkey::from_str(spl_token(&["address"])["walletAddress"].as_str().unwrap()).unwrap();
    let mint = token_address;
    /*
    const TOKEN_PROGRAM_ID = new PublicKey('7t5SuBhmxxKuQyjwTnmPpFpqJurCDM4dvM14nUGiza4s');
    const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('Dt8fRCpjeV6JDemhPmtcTKijgKdPxXHn9Wo9cXY5agtG');
    const owner = new PublicKey('Fk2HRYuDw9h29yKs1tNDjvjdvYMqQ2dGg9sS4JhUzQ6w');
    const mint = new PublicKey('9bLgyijGKGrKHfT72JUQZNgEH4GVTuHosk3VcdpHYo19');

    const [address] = await PublicKey.findProgramAddress(
        [owner.toBuffer(), TOKEN_PROGRAM_ID.toBuffer(), mint.toBuffer()],
        ASSOCIATED_TOKEN_PROGRAM_ID
    );
    */

    let (pubkey, _bump_seed) = Pubkey::find_program_address(
        &[owner.as_ref(), token_program_id.as_ref(), mint.as_ref()],
        &associated_token_program_id,
    );
    pubkey
}

#[derive(Deserialize)]
struct MintTokenRequest {
    amount: u64,
}
async fn mint_token(Json(request): Json<MintTokenRequest>) -> Json<Vec<serde_json::Value>> {
    let mut out = Vec::new();
    let create_token_result = spl_token(&["create-token", "--decimals", "8"]);
    let token_address = create_token_result["commandOutput"]["address"]
        .as_str()
        .unwrap()
        .to_string();
    out.push(create_token_result);

    let account_address = get_account_address(Pubkey::from_str(&token_address).unwrap()).await;
    out.push(json!({
        "accountAddress": account_address.to_string(),
    }));

    out.push(spl_token(&["create-account", &token_address]));
    out.push(spl_token(&[
        "mint",
        &token_address,
        &request.amount.to_string(),
    ]));

    Json(out)
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
