use std::str::FromStr;

use axum::{extract::State, Json};
use domichain_program::pubkey::Pubkey;
use serde::Deserialize;
use serde_json::json;
use tokio::fs::remove_dir_all;

use crate::{
    bdk_cli::{
        bdk_cli, bdk_cli_wallet, bdk_cli_wallet_patched, bdk_cli_wallet_temp, WALLET_DIR_PERMIT,
    },
    mint_token::get_account_address,
    serde_convert,
    spl_token::spl_token,
    AppState,
};

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct SignMultisigTxRequest {
    mint_address: String,
    withdraw_address: String,
    withdraw_amount: String,
}

pub async fn sign_multisig_tx(
    State(state): State<AppState>,
    Json(request): Json<SignMultisigTxRequest>,
) -> Json<serde_json::Value> {
    dbg!("POST sign_multisig_tx");

    let SignMultisigTxRequest {
        mint_address,
        withdraw_address,
        withdraw_amount,
    } = request;

    let meta = if let Some(meta) = state.db.find_by_mint_address(&mint_address).await.unwrap() {
        meta
    } else {
        // Document not found
        return Json(json!({
            "status": "error",
            "message": format!("Mint address not found: {mint_address}"),
        }));
    };

    dbg!(&meta);
    let meta: serde_json::Value = serde_convert(&meta);

    // TODO: get fields from meta

    // export DESCRIPTOR_00="$XPRV_00/84h/1h/0h/0/*"
    // export XPUB_01=$(cat pub_kms_01.json)
    // export XPUB_02=$(bdk-cli key derive --xprv $XPRV_02 --path "m/84'/1'/0'/0" | jq -r ".xpub")

    let private_key_00: serde_json::Value =
        serde_json::from_str(meta["private_key_00"].as_str().unwrap()).unwrap();
    let xprv_00 = &private_key_00["xprv"].as_str().unwrap();
    let descriptor_00 = format!("{xprv_00}/84h/1h/0h/0/*");

    let xpub_00 = meta["public_key_00"].as_str().unwrap();
    let xpub_01 = meta["public_key_01"].as_str().unwrap();
    let xpub_02 = meta["public_key_02"].as_str().unwrap();

    let key_arn = meta["public_key_arn_01"].as_str().unwrap();

    // TODO: check constrains, check burn

    let to_address = &withdraw_address;
    let amount = &withdraw_amount;
    // let to_address = "tb1qjk7wqccmetsngh9e0zff73rhsqny568g5fs758";
    // let amount = "400";

    let onesig_psbt = onesig(&descriptor_00, xpub_01, xpub_02, to_address, amount).await;
    dbg!(&onesig_psbt);

    let (secondsig_psbt, multi_descriptor_01) =
        secondsig(xpub_00, xpub_01, xpub_02, &onesig_psbt, key_arn).await;

    let account_address = get_account_address(Pubkey::from_str(&mint_address).unwrap());
    let burn_output = spl_token(&["burn", &account_address.to_string(), &withdraw_amount]);
    dbg!(&burn_output);

    let tx_id = send(&multi_descriptor_01, &secondsig_psbt).await;

    dbg!("POST sign_multisig_tx finish");

    Json(json!({
        "status": "ok",
        "secondsig_psbt": secondsig_psbt,
        "tx_id": tx_id,
        "tx_link": format!("https://mempool.space/testnet/tx/{tx_id}"),
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
    dbg!(sync_output);

    // bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_balance | jq
    let get_balance_result = bdk_cli_wallet_temp(multi_descriptor_00, &["get_balance"]).await;
    dbg!(get_balance_result);

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
