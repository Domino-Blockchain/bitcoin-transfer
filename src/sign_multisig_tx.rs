use std::{path::PathBuf, str::FromStr};

use axum::{extract::State, Json};
use bdk::bitcoin::Network;
use serde::Deserialize;
use serde_json::json;
use tokio::fs::remove_dir_all;
use tracing::info;

use crate::{
    bdk_cli::{
        bdk_cli, bdk_cli_wallet, bdk_cli_wallet_patched, bdk_cli_wallet_temp, WALLET_DIR_PERMIT,
    },
    bdk_cli_struct::BdkCli,
    mempool::get_mempool_url,
    serde_convert, AppState,
};

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct SignMultisigTxRequest {
    mint_address: String,
    withdraw_address: String, // BTC
    withdraw_amount: String,
}

pub async fn sign_multisig_tx(
    State(state): State<AppState>,
    Json(request): Json<SignMultisigTxRequest>,
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

    let SignMultisigTxRequest {
        mint_address,
        withdraw_address,
        withdraw_amount,
    } = request;

    let (transaction, key) = state.db.find_by_mint_address(&mint_address).await.unwrap();
    // let meta = if let Some(meta) =  {
    //     meta
    // } else {
    //     // Document not found
    //     return Json(json!({
    //         "status": "error",
    //         "message": format!("Mint address not found: {mint_address}"),
    //     }));
    // };

    info!("transaction: {:#?}", &transaction);
    info!("key: {:#?}", &key);
    let _transaction: serde_json::Value = serde_convert(&transaction);
    let key: serde_json::Value = serde_convert(&key);

    // TODO: get fields from meta

    // export DESCRIPTOR_00="$XPRV_00/84h/1h/0h/0/*"
    // export XPUB_01=$(cat pub_kms_01.json)
    // export XPUB_02=$(bdk-cli key derive --xprv $XPRV_02 --path "m/84'/1'/0'/0" | jq -r ".xpub")

    let private_key_00: serde_json::Value =
        serde_json::from_str(key["private_key_00"].as_str().unwrap()).unwrap();
    let xprv_00 = &private_key_00["xprv"].as_str().unwrap();
    // let descriptor_00 = format!("{xprv_00}/84h/1h/0h/0/*");

    let xpub_00 = key["public_key_00"].as_str().unwrap();
    let xpub_01 = key["public_key_01"].as_str().unwrap();
    let xpub_02 = key["public_key_02"].as_str().unwrap();
    let xpub_03 = key["public_key_03"].as_str().unwrap();

    let key_arn = key["public_key_arn_01"].as_str().unwrap();
    let key_name = key["public_key_name_03"].as_str().unwrap();

    // TODO: check constrains, check burn

    let to_address = &withdraw_address;
    let amount = &withdraw_amount;
    // let to_address = "tb1qjk7wqccmetsngh9e0zff73rhsqny568g5fs758";
    // let amount = "400";

    let onesig_psbt = cli
        .onesig(xprv_00, xpub_01, xpub_02, xpub_03, to_address, amount)
        .await;
    // let onesig_psbt = onesig(&descriptor_00, xpub_01, xpub_02, to_address, amount).await;
    info!("onesig_psbt: {:#?}", &onesig_psbt);

    let secondsig_psbt = cli
        .secondsig(xpub_00, xpub_01, xpub_02, xpub_03, &onesig_psbt, key_arn)
        .await;
    info!("secondsig_psbt: {:#?}", &secondsig_psbt);
    // let (secondsig_psbt, multi_descriptor_01) =
    //     secondsig(xpub_00, xpub_01, xpub_02, &onesig_psbt, key_arn).await;

    let thirdsig_psbt = cli
        .thirdsig(
            xpub_00,
            xpub_01,
            xpub_02,
            xpub_03,
            &secondsig_psbt,
            key_name,
        )
        .await;
    info!("thirdsig_psbt: {:#?}", &thirdsig_psbt);

    // let account_address = get_account_address(Pubkey::from_str(&mint_address).unwrap());
    // info!("Burn system account_address: {account_address:?}");
    // let burn_output = spl_token(&["burn", &account_address.to_string(), &withdraw_amount]);
    // info!("burn_output: {:#?}", &burn_output);

    let tx_id = cli
        .send(xpub_00, xpub_01, xpub_02, xpub_03, &thirdsig_psbt)
        .await;
    // let tx_id = send(&multi_descriptor_01, &secondsig_psbt).await;

    let mempool_url = get_mempool_url();

    let tx_link = format!("{mempool_url}/tx/{tx_id}");
    info!("transaction sent: {tx_link}");
    Json(json!({
        "status": "ok",
        "thirdsig_psbt": thirdsig_psbt,
        "tx_id": tx_id,
        "tx_link": tx_link,
    }))
}

pub async fn onesig(
    descriptor_00: &str,
    xpub_01: &str,
    xpub_02: &str,
    to_address: &str,
    amount: &str,
) -> String {
    let wallet_dir_permit = WALLET_DIR_PERMIT.acquire().await.unwrap();
    let _ = remove_dir_all("/home/domi/.bdk-bitcoin/wallet_name_temp").await;

    // export MULTI_DESCRIPTOR_00=$(bdk-cli compile "thresh(2,pk($DESCRIPTOR_00),pk($XPUB_01),pk($XPUB_02))" | jq -r '.descriptor')
    let multi_descriptor_00_ = format!("thresh(2,pk({descriptor_00}),pk({xpub_01}),pk({xpub_02}))");
    let multi_descriptor_00_result = bdk_cli(&["compile", &multi_descriptor_00_]).await;
    let multi_descriptor_00 = multi_descriptor_00_result["descriptor"].as_str().unwrap();

    // bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sync
    let sync_output = bdk_cli_wallet_temp(multi_descriptor_00, &["sync"]).await;
    info!("sync_output: {:#?}", sync_output);

    // bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_balance | jq
    let get_balance_result = bdk_cli_wallet_temp(multi_descriptor_00, &["get_balance"]).await;
    info!("get_balance_result: {:#?}", get_balance_result);

    // export CHANGE_ID=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 policies | jq -r ".external.id")
    let change_id_ = bdk_cli_wallet_temp(multi_descriptor_00, &["policies"]).await;
    let change_id = change_id_["external"]["id"].as_str().unwrap();

    // export UNSIGNED_PSBT=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 create_tx --to $TO_ADDRESS:$AMOUNT --external_policy "{\"$CHANGE_ID\": [0,1]}" | jq -r '.psbt')
    let unsigned_psbt_ = bdk_cli_wallet_temp(
        multi_descriptor_00,
        &[
            "create_tx",
            "--to",
            &format!("{to_address}:{amount}"),
            "--external_policy",
            &format!("{{\"{change_id}\": [0,1]}}"),
        ],
    )
    .await;
    let unsigned_psbt = unsigned_psbt_["psbt"].as_str().unwrap();

    // export ONESIG_PSBT=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sign --psbt $UNSIGNED_PSBT | jq -r '.psbt')
    let onesig_psbt_ =
        bdk_cli_wallet_temp(multi_descriptor_00, &["sign", "--psbt", unsigned_psbt]).await;
    let onesig_psbt = onesig_psbt_["psbt"].as_str().unwrap().to_string();

    let _ = remove_dir_all("/home/domi/.bdk-bitcoin/wallet_name_temp").await;
    drop(wallet_dir_permit);

    onesig_psbt
}

pub async fn secondsig(
    xpub_00: &str,
    xpub_01: &str,
    xpub_02: &str,
    onesig_psbt: &str,
    key_arn: &str,
) -> (String, String) {
    // export MULTI_DESCRIPTOR_01=$(cat multi_descriptor_01.json)
    // export ONESIG_PSBT=$(cat onesig_psbt.json)
    // export KEY_ARN="arn:aws:kms:us-east-2:571922870935:key/17be5d9e-d752-4350-bbc1-68993fa25a4f"

    // export MULTI_DESCRIPTOR_01=$(bdk-cli compile "thresh(2,pk($XPUB_00),pk($XPUB_01),pk($XPUB_02))" | jq -r '.descriptor')
    let multi_descriptor_01_ = format!("thresh(2,pk({xpub_00}),pk({xpub_01}),pk({xpub_02}))");
    let multi_descriptor_01_result = bdk_cli(&["compile", &multi_descriptor_01_]).await;
    let multi_descriptor_01 = multi_descriptor_01_result["descriptor"]
        .as_str()
        .unwrap()
        .to_string();

    // export SECONDSIG_PSBT=$(./bdk-cli/target/release/bdk-cli wallet --aws_kms $KEY_ARN --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 sign --psbt $ONESIG_PSBT | jq -r '.psbt')
    let secondsig_psbt_ = bdk_cli_wallet_patched(
        &multi_descriptor_01,
        &["--aws_kms", key_arn, "sign", "--psbt", onesig_psbt],
    )
    .await;
    let secondsig_psbt = secondsig_psbt_["psbt"].as_str().unwrap();

    // if [ "$ONESIG_PSBT" = "$SECONDSIG_PSBT" ]; then
    //     echo "ERROR: Secondsig don't change PSBT"
    //     exit 1
    // fi
    assert_ne!(onesig_psbt, secondsig_psbt);

    (secondsig_psbt.to_string(), multi_descriptor_01)
}

pub async fn send(multi_descriptor_01: &str, secondsig_psbt: &str) -> String {
    // # broadcast
    // export TX_ID=$(bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 broadcast --psbt $SECONDSIG_PSBT)
    // echo $TX_ID
    let tx_id_ = bdk_cli_wallet(
        multi_descriptor_01,
        &["broadcast", "--psbt", secondsig_psbt],
    )
    .await;
    let tx_id = tx_id_["txid"].as_str().unwrap().to_string();

    // echo "Check: https://mempool.space/testnet/tx/$(echo $TX_ID | jq -r ".txid")"
    tx_id
}
