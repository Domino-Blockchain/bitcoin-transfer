use std::{path::PathBuf, str::FromStr};

use axum::{extract::State, Json};
use bdk::{bitcoin::Network, FeeRate};
use serde::{Deserialize, Serialize};
use serde_json::Number;
use tracing::{debug, info};

use crate::{
    bdk_cli_struct::BdkCli,
    mempool::{get_mempool_url, get_recommended_fee_rates, RecommendedFeesResp},
    serde_convert, AppState, Args,
};

#[derive(Deserialize)]
pub struct EstimateFeeRequest {
    mint_address: String,
    withdraw_address: String, // BTC
    withdraw_amount: String,
}

#[derive(Serialize)]
pub struct RecommendedFeeRates {
    fastest_fee: Number,
    half_hour_fee: Number,
    hour_fee: Number,
    economy_fee: Number,
    minimum_fee: Number,
}

#[derive(Serialize)]
pub struct EstimateFeeResponse {
    status: String,
    vbytes: u64,
    recommended_fee_rates: RecommendedFeeRates,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum EstimateFeeResult {
    Ok(EstimateFeeResponse),
    Error { status: String, message: String },
}

pub async fn estimate_fee(
    State(state): State<AppState>,
    Json(request): Json<EstimateFeeRequest>,
) -> Json<EstimateFeeResult> {
    let Args {
        bdk_cli_path_default,
        bdk_cli_path_patched,
        btc_network,
        ..
    } = state.config;

    let temp_wallet_dir = None;
    let descriptor = None;
    let cli = BdkCli::new(
        btc_network,
        bdk_cli_path_default,
        bdk_cli_path_patched,
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
            return Json(EstimateFeeResult::Error {
                status: "error".to_string(),
                message: format!("Mint address not found: {mint_address}"),
            });
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

    let recommended_fee_rates = get_recommended_fee_rates(get_mempool_url(btc_network)).await;
    debug!(
        "get_recommended_fee_rates was_cached: {:?}",
        recommended_fee_rates.was_cached
    );
    let recommended_fee_rates = recommended_fee_rates.value;
    let recommended_fee = &recommended_fee_rates.fastest_fee;
    let fee_rate = FeeRate::from_sat_per_vb(recommended_fee.as_f64().unwrap() as f32);

    let (fee, vbytes) = cli
        .estimate_fee(&multi_descriptor_00, to_address, amount, fee_rate)
        .await;
    info!("fee: {fee}");
    info!("fee_rate: {fee_rate:?}");
    info!("vbytes: {vbytes}");

    let RecommendedFeesResp {
        fastest_fee,
        half_hour_fee,
        hour_fee,
        economy_fee,
        minimum_fee,
    } = recommended_fee_rates;
    return Json(EstimateFeeResult::Ok(EstimateFeeResponse {
        status: "ok".to_string(),
        vbytes,
        recommended_fee_rates: RecommendedFeeRates {
            fastest_fee,
            half_hour_fee,
            hour_fee,
            economy_fee,
            minimum_fee,
        },
    }));
}

pub fn get_vbytes(fee: u64, fee_rate: FeeRate) -> u64 {
    let vbytes = fee as f32 / fee_rate.as_sat_per_vb();
    assert!(vbytes.fract().abs() <= f32::EPSILON);
    let vbytes = vbytes.round() as u64;
    vbytes
}
