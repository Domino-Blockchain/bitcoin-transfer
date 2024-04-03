use axum::{extract::State, Json};
use serde::Deserialize;

use crate::{
    bdk_cli::{bdk_cli, bdk_cli_wallet},
    AppState,
};

#[derive(Deserialize)]
pub struct SignMultisigTxRequest {
    mint_address: String,
}

pub async fn sign_multisig_tx(
    State(state): State<AppState>,
    Json(request): Json<SignMultisigTxRequest>,
) -> Json<serde_json::Value> {
    let mint_address = request.mint_address;

    let meta = state.db.find_by_mint_address(&mint_address).await.unwrap();

    let meta = meta.unwrap();
    dbg!(&meta);

    // TODO: get fields from meta

    // export DESCRIPTOR_00="$XPRV_00/84h/1h/0h/0/*"
    // export XPUB_01=$(cat pub_kms_01.json)
    // export XPUB_02=$(bdk-cli key derive --xprv $XPRV_02 --path "m/84'/1'/0'/0" | jq -r ".xpub")

    let private_key_00 = meta.get("private_key_00").unwrap();

    let descriptor_00 = todo!();
    let xpub_01 = todo!();
    let xpub_02 = todo!();

    // TODO: check constrains

    let unsigned_psbt = onesig(descriptor_00, xpub_01, xpub_02).await;
    dbg!(&unsigned_psbt);

    Json(todo!())
}

pub async fn onesig(descriptor_00: &str, xpub_01: &str, xpub_02: &str) -> String {
    // export MULTI_DESCRIPTOR_00=$(bdk-cli compile "thresh(2,pk($DESCRIPTOR_00),pk($XPUB_01),pk($XPUB_02))" | jq -r '.descriptor')
    let multi_descriptor_00_ = format!("thresh(2,pk({descriptor_00}),pk({xpub_01}),pk({xpub_02}))");
    let multi_descriptor_00_result = bdk_cli(&["compile", &multi_descriptor_00_]).await;
    let multi_descriptor_00 = multi_descriptor_00_result["descriptor"].as_str().unwrap();

    // bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sync
    let sync_output = bdk_cli(&[
        "wallet",
        "--wallet",
        "wallet_name_msd00",
        "--descriptor",
        multi_descriptor_00,
        "sync",
    ])
    .await;
    dbg!(sync_output);

    // bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_balance | jq
    let get_balance_result = bdk_cli_wallet(multi_descriptor_00, &["get_balance"]).await;
    dbg!(get_balance_result);

    // export CHANGE_ID=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 policies | jq -r ".external.id")
    let change_id_ = bdk_cli_wallet(multi_descriptor_00, &["policies"]).await;
    let change_id = change_id_["external"]["id"].as_str().unwrap();

    let to_address = "tb1qjk7wqccmetsngh9e0zff73rhsqny568g5fs758";
    let amount = "550";

    // export UNSIGNED_PSBT=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 create_tx --to $TO_ADDRESS:$AMOUNT --external_policy "{\"$CHANGE_ID\": [0,1]}" | jq -r '.psbt')
    let unsigned_psbt_ = bdk_cli_wallet(
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
    unsigned_psbt.to_string()
}
