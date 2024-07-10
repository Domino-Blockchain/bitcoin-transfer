use std::{path::PathBuf, str::FromStr};

use axum::{extract::State, Json};
use bdk::bitcoin::Network;
use mongodb::bson::{doc, Document};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::fs::read_to_string;
use tracing::info;

use crate::{
    bdk_cli_struct::BdkCli, serde_convert, watch_addresses::watch_address, AppState, Args,
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
struct GenerateKeyResult {
    pub fingerprint: String,
    pub mnemonic: String,
    pub xprv: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GetPubkeyResult {
    pub xprv: String,
    pub xpub: String,
}

pub async fn new_multisig_address(state: &AppState, domi_address: String) -> String {
    let Args {
        bdk_cli_path_default,
        bdk_cli_path_patched,
        btc_network,
        ..
    } = &state.config;

    let temp_wallet_dir = None;
    let descriptor = None;
    let cli = BdkCli::new(
        *btc_network,
        bdk_cli_path_default.clone(),
        bdk_cli_path_patched.clone(),
        temp_wallet_dir,
        descriptor,
    )
    .await;

    let key_00 = cli.generate_key().await;
    let xprv_00 = &key_00.xprv;
    // let key_00 = generate_key().await;

    let to_save_encrypted: Document = serde_convert(&key_00);

    // Hardcoded hardware key
    let ledger_keys_string = read_to_string(std::env::var("LEDGER_KEYS_PATH").unwrap())
        .await
        .unwrap();
    let ledger_keys: Value = serde_json::from_str(&ledger_keys_string).unwrap();
    let key_02 = match btc_network {
        Network::Bitcoin => &ledger_keys["bitcoin"],
        Network::Testnet => &ledger_keys["testnet"],
        Network::Signet => todo!(),
        Network::Regtest => todo!(),
        _ => todo!(),
    };
    // let xprv_02 = key_02["xprv"].as_str().unwrap();

    let xpub_00 = cli.get_pubkey(xprv_00).await;
    // let xpub_00 = get_pubkey(xprv_00).await.xpub;

    let hash = get_hash(xpub_00.as_bytes());
    let (pub_name_01, pub_arn_01, xpub_01) = state.db.get_aws_kms_pubkey(hash).await;

    // let xpub_02 = cli.get_pubkey(xprv_02).await;
    let xpub_02 = key_02["xpub"].as_str().unwrap();
    // let xpub_02 = get_pubkey(&xprv_02).await.xpub;

    let (pub_name_03, xpub_03) = state.db.get_google_kms_pubkey(hash).await;
    // let (pub_name_03, xpub_03) = (
    //     "projects/domichain-archive/locations/global/keyRings/TestKeyring/cryptoKeys/TestKey1/cryptoKeyVersions/1",
    //     "036f0694a43f05fd642f1fe0b3bd023b1322df39080c5624a5ba8bede20fcd9dc2",
    // );

    let multi_descriptor_00 = cli
        .get_multi_descriptor(xprv_00, &xpub_01, &xpub_02, &xpub_03)
        .await;
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
        "public_key_name_03": &pub_name_03,
        "public_key_03": &xpub_03,
        "multi_address": &multi_address,
        "domi_address": domi_address,
    };

    // info!("to_save_encrypted: {:#?}", &to_save_encrypted);
    info!("to_save: {:#?}", &to_save);
    info!("multi_address: {:#?}", &multi_address);

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
    let Args { btc_network, .. } = state.config;

    let address = new_multisig_address(&state, request.domi_address).await;

    let _h = tokio::spawn(watch_address(
        address.clone(),
        state.db.clone(),
        btc_network,
    ));

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
