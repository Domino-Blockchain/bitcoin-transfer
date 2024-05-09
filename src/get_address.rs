use std::{path::PathBuf, str::FromStr};

use axum::{extract::State, Json};
use bdk::bitcoin::Network;
use mongodb::bson::{doc, Document};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use serde_json::from_value;
use sha2::{Digest, Sha256};

use crate::{
    bdk_cli::{bdk_cli, bdk_cli_wallet},
    bdk_cli_struct::{BdkCli, CliGenerateKeyResult},
    serde_convert,
    watch_addresses::watch_address,
    AppState,
};

/*
{
  "fingerprint": "14b779cc",
  "mnemonic": "pool trap pudding toy wasp recipe army wife pumpkin sign bacon all laugh teach home mother shock then age blossom fabric awful guess safe",
  "xprv": "tprv8ZgxMBicQKsPefw5n4dj6LbMmeTRSiUryfTRbMFyBkGGwVEtEyRkVRDthNJcKRJGnye64j5FNEbdWVFeUAGE2pggYjMwSgK8VwtxvNZaH3k"
}

bdk-cli key derive --xprv $XPRV_00 --path "m/84'/1'/0'/0" | jq -r ".xpub"

*/

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateKeyResult {
    pub fingerprint: String,
    pub mnemonic: String,
    pub xprv: String,
}
pub async fn generate_key() -> GenerateKeyResult {
    let result = bdk_cli(&["key", "generate"]).await;
    from_value(result).unwrap()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPubkeyResult {
    pub xprv: String,
    pub xpub: String,
}
// export XPUB_00=$(bdk-cli key derive --xprv $XPRV_00 --path "m/84'/1'/0'/0" | jq -r ".xpub")
pub async fn get_pubkey(xprv: &str) -> GetPubkeyResult {
    let result = bdk_cli(&["key", "derive", "--xprv", xprv, "--path", "m/84'/1'/0'/0"]).await;
    from_value(result).unwrap()
}

pub async fn new_multisig_address(state: &AppState, domi_address: String) -> String {
    let btc_network = Network::from_str(&std::env::var("BTC_NETWORK").unwrap()).unwrap();
    let cli_path = PathBuf::from(std::env::var("BDK_CLI_PATH_DEFAULT").unwrap());
    let cli_path_patched = PathBuf::from(std::env::var("BDK_CLI_PATH_PATCHED").unwrap());
    let temp_wallet_dir = PathBuf::from(std::env::var("BDK_TEMP_WALLET_DIR").unwrap());
    let descriptor = None;
    let cli = BdkCli::new(
        btc_network,
        cli_path,
        cli_path_patched,
        temp_wallet_dir,
        descriptor,
    )
    .await;

    let key_00 = cli.generate_key().await;
    let xprv_00 = &key_00.xprv;
    // let key_00 = generate_key().await;

    let to_save_encrypted: Document = serde_convert(&key_00);

    // Hardcoded hardware key

    // BTC Testnet
    let key_02 = match btc_network {
        Network::Bitcoin => GenerateKeyResult {
            fingerprint: "2ea57a1f".to_string(),
            mnemonic: "inherit visual crew carry roast visa video swear faculty prepare shoulder upset trend fossil evoke time satisfy pledge grain switch stable win enforce solid".to_string(),
            xprv: "xprv9s21ZrQH143K2hUZkrsVcZVHXRPJNFRzAUhHKk3xvtKfpaF1b1GJ2njQrgCQm7WU7TrFSXdPk9vcd3WSb3rJg4EiebZ1uUkj7szSvc4rwvn".to_string(),
        },
        Network::Testnet => GenerateKeyResult {
            fingerprint: "41a64ac3".to_string(),
            mnemonic: "elegant rack glad merge guess because fancy girl paper together inherit retire mom ribbon tissue dose rule click forum used beef cluster wrestle loyal".to_string(),
            xprv: "tprv8ZgxMBicQKsPfQEGu2E2hYdjGwZovwNeKJzjECzmbZVTnE94n5PVnLTx6isQZn9sHpnVHo81EWRNepTHbTa6AzfuhpWRsuoNtVaDfZFoqb5".to_string(),
        },
        Network::Signet => todo!(),
        Network::Regtest => todo!(),
        _ => todo!(),
    };
    let xprv_02 = &key_02.xprv;

    let xpub_00 = cli.get_pubkey(xprv_00).await;
    // let xpub_00 = get_pubkey(xprv_00).await.xpub;

    let hash = get_hash(xpub_00.as_bytes());
    let (pub_name_01, pub_arn_01, xpub_01) = state.db.get_kms_pubkey(hash).await;

    let xpub_02 = cli.get_pubkey(xprv_02).await;
    // let xpub_02 = get_pubkey(&xprv_02).await.xpub;

    let multi_descriptor_00 = cli.get_multi_descriptor(xprv_00, &xpub_01, &xpub_02).await;
    // let descriptor_00 = format!("{xprv_00}/84h/1h/0h/0/*");
    // // let _descriptor_02 = format!("{xprv_02}/84h/1h/0h/0/*");
    // let desc_00 = format!("thresh(2,pk({descriptor_00}),pk({xpub_01}),pk({xpub_02}))");
    // let multi_descriptor_00_ = bdk_cli(&["compile", &desc_00]).await;
    // let multi_descriptor_00 = multi_descriptor_00_["descriptor"].as_str().unwrap();

    // Clear temporary bdk cache
    let multi_address = cli.get_multi_address(&multi_descriptor_00).await;
    // let multi_address_ = bdk_cli_wallet(&multi_descriptor_00, &["get_new_address"]).await;
    // let multi_address = multi_address_["address"].as_str().unwrap().to_owned();

    let to_save = doc! {
        "public_key_00": &xpub_00,
        "public_key_name_01": &pub_name_01,
        "public_key_arn_01": &pub_arn_01,
        "public_key_01": &xpub_01,
        "public_key_02": &xpub_02,
        "multi_address": &multi_address,
        "domi_address": domi_address,
    };

    dbg!(&to_save_encrypted);
    dbg!(&to_save);
    dbg!(&multi_address);

    state
        .db
        .save_private_key(to_save_encrypted, to_save)
        .await
        .unwrap();

    multi_address
}

#[derive(Deserialize)]
pub struct NewMiltisigAddressRequest {
    pub domi_address: String,
}

pub async fn get_address_from_db(
    State(state): State<AppState>,
    Json(request): Json<NewMiltisigAddressRequest>,
) -> Json<String> {
    let address = new_multisig_address(&state, request.domi_address).await;

    let h = tokio::spawn(watch_address(address.clone(), state.db.clone()));

    // Json(state.db.get_address().await)
    Json(address)
}

fn get_hash(data: &[u8]) -> U256 {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    assert_eq!(result.len() * 8, 256);
    let hash: U256 = (&result[..]).try_into().unwrap();
    hash
}
