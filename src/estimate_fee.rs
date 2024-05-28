use std::{path::PathBuf, str::FromStr};

use axum::{extract::State, Json};
use bdk::{bitcoin::Network, FeeRate};
use serde::Deserialize;
use serde_json::json;
use tracing::info;

use crate::{bdk_cli_struct::BdkCli, serde_convert, AppState};

#[derive(Deserialize)]
pub struct EstimateFeeRequest {
    mint_address: String,
    withdraw_address: String, // BTC
    withdraw_amount: String,
}

pub async fn estimate_fee(
    State(state): State<AppState>,
    Json(request): Json<EstimateFeeRequest>,
) -> Json<serde_json::Value> {
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

    let EstimateFeeRequest {
        mint_address,
        withdraw_address,
        withdraw_amount,
    } = request;

    let (transaction, key) =
        if let Some(data) = state.db.find_by_mint_address(&mint_address).await.unwrap() {
            data
        } else {
            // Document not found
            return Json(json!({
                "status": "error",
                "message": format!("Mint address not found: {mint_address}"),
            }));
        };

    info!("transaction: {:#?}", &transaction);
    info!("key: {:#?}", &key);
    let _transaction: serde_json::Value = serde_convert(&transaction);
    let key: serde_json::Value = serde_convert(&key);

    let private_key_00: serde_json::Value =
        serde_json::from_str(key["private_key_00"].as_str().unwrap()).unwrap();
    let xprv_00 = &private_key_00["xprv"].as_str().unwrap();
    // let descriptor_00 = format!("{xprv_00}/84h/1h/0h/0/*");

    // let xpub_00 = key["public_key_00"].as_str().unwrap();
    let xpub_01 = key["public_key_01"].as_str().unwrap();
    let xpub_02 = key["public_key_02"].as_str().unwrap();
    let xpub_03 = key["public_key_03"].as_str().unwrap();

    // let key_arn = key["public_key_arn_01"].as_str().unwrap();
    // let key_name = key["public_key_name_03"].as_str().unwrap();

    let to_address = &withdraw_address;
    let amount = &withdraw_amount;

    let multi_descriptor_00 = cli
        .get_multi_descriptor(xprv_00, xpub_01, xpub_02, xpub_03)
        .await;

    let (fee, fee_rate, vbytes) = cli
        .estimate_fee(&multi_descriptor_00, to_address, amount)
        .await;
    info!("fee: {}", &fee);
    info!("fee_rate: {:?}", &fee_rate);

    return Json(json!({
        "status": "ok",
        "fee": fee,
        "fee_rate": fee_rate.as_sat_per_vb(),
        "vbytes": vbytes,
    }));
}

pub fn get_vbytes(fee: u64, fee_rate: FeeRate) -> u64 {
    let vbytes = fee as f32 / fee_rate.as_sat_per_vb();
    assert!(vbytes.fract().abs() <= f32::EPSILON);
    let vbytes = vbytes.round() as u64;
    vbytes
}
