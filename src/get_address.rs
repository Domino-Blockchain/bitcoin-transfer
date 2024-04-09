use axum::{extract::State, Json};
use mongodb::bson::{doc, Document};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use serde_json::from_value;
use sha2::{Digest, Sha256};

use crate::{
    bdk_cli::{bdk_cli, bdk_cli_wallet},
    serde_convert, AppState,
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

pub async fn new_multisig_address(state: &AppState) -> String {
    let key_00 = generate_key().await;

    let to_save_encrypted: Document = serde_convert(&key_00);

    // Hardcoded hardware key
    let key_02 = GenerateKeyResult {
        fingerprint: "41a64ac3".to_string(),
        mnemonic: "elegant rack glad merge guess because fancy girl paper together inherit retire mom ribbon tissue dose rule click forum used beef cluster wrestle loyal".to_string(),
        xprv: "tprv8ZgxMBicQKsPfQEGu2E2hYdjGwZovwNeKJzjECzmbZVTnE94n5PVnLTx6isQZn9sHpnVHo81EWRNepTHbTa6AzfuhpWRsuoNtVaDfZFoqb5".to_string(),
    };

    let xprv_00 = &key_00.xprv;
    let xprv_02 = key_02.xprv;

    let xpub_00 = get_pubkey(xprv_00).await.xpub;

    let mut to_save = doc! { "public_key_00": &xpub_00 };

    let hash = get_hash(xpub_00.as_bytes());
    let (pub_name_01, pub_arn_01, xpub_01) = state.db.get_kms_pubkey(hash).await;
    to_save.insert("public_key_name_01", &pub_name_01);
    to_save.insert("public_key_arn_01", &pub_arn_01);
    to_save.insert("public_key_01", &xpub_01);

    let xpub_02 = get_pubkey(&xprv_02).await.xpub;
    to_save.insert("public_key_02", &xpub_02);

    let descriptor_00 = format!("{xprv_00}/84h/1h/0h/0/*");
    let _descriptor_02 = format!("{xprv_02}/84h/1h/0h/0/*");

    let desc_00 = format!("thresh(2,pk({descriptor_00}),pk({xpub_01}),pk({xpub_02}))");
    let multi_descriptor_00_ = bdk_cli(&["compile", &desc_00]).await;
    let multi_descriptor_00 = multi_descriptor_00_["descriptor"].as_str().unwrap();

    // Clear temporary bdk cache
    let multi_address_ = bdk_cli_wallet(multi_descriptor_00, &["get_new_address"]).await;
    let multi_address = multi_address_["address"].as_str().unwrap().to_owned();

    to_save.insert("multi_address", &multi_address);

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

pub async fn get_address_from_db(State(state): State<AppState>) -> Json<String> {
    let address = new_multisig_address(&state).await;

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
