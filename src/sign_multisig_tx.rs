use std::str::FromStr;

use axum::{extract::State, Json};
use bdk::FeeRate;
use domichain_account_decoder::parse_token::token_amount_to_ui_amount;
use domichain_program::pubkey::Pubkey;
use domichain_sdk::signature::Signature;
use reqwest::Url;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::fs::remove_dir_all;
use tracing::{debug, info};

use crate::{
    bdk_cli::{
        bdk_cli, bdk_cli_wallet, bdk_cli_wallet_patched, bdk_cli_wallet_temp, WALLET_DIR_PERMIT,
    },
    bdk_cli_struct::BdkCli,
    domichain::{get_block_height, get_transaction_poll, DomiTransactionInstructionInfo},
    estimate_fee::get_vbytes,
    mempool::{get_mempool_url, get_recommended_fee_rate},
    mint_token::{burn_token_inner, get_account_address},
    utils::{serde_as_str, serde_convert},
    AppState, Args,
};

#[derive(Deserialize)]
pub struct SignMultisigTxRequest {
    #[serde(with = "serde_as_str")]
    mint_address: Pubkey,
    /// BTC withdraw destination address
    withdraw_address: String,
    withdraw_amount: String,
    fee_rate: Option<serde_json::Number>,
    vbytes: Option<u64>,
    #[serde(with = "serde_as_str")]
    domi_address: Pubkey,
    block_height: u64,
    #[serde(with = "serde_as_str")]
    btci_tx_signature: Signature,
    #[serde(with = "serde_as_str")]
    signature: Signature,
}

pub async fn sign_multisig_tx(
    State(state): State<AppState>,
    Json(request): Json<SignMultisigTxRequest>,
) -> Json<serde_json::Value> {
    let Args {
        domichain_rpc_url,
        spl_token_program_id,
        bdk_cli_path_default,
        bdk_cli_path_patched,
        btc_network,
        ..
    } = state.config.clone();

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

    if let Err(verify_error) =
        verify_request_signature(&domichain_rpc_url, spl_token_program_id, &request).await
    {
        return Json(json!({
            "status": "error",
            "message": format!("verification is failed: {verify_error}"),
        }));
    }

    let SignMultisigTxRequest {
        mint_address,
        withdraw_address,
        withdraw_amount,
        fee_rate,
        vbytes,
        domi_address: _,
        block_height,
        btci_tx_signature: _,
        signature: _,
    } = request;

    // Validate withdraw_address
    match bdk::bitcoin::Address::from_str(&withdraw_address) {
        Err(error) => {
            // withdraw_address is invalid
            return Json(json!({
                "status": "error",
                "message": format!("withdraw_address is invalid: {error}"),
            }));
        }
        Ok(address) => {
            if !address.is_valid_for_network(btc_network) {
                // withdraw_address is invalid for currnet network
                return Json(json!({
                    "status": "error",
                    "message": format!("withdraw_address is invalid for '{btc_network}' network"),
                }));
            }
        }
    }

    let actual_block_height = get_block_height(domichain_rpc_url).await;
    let block_height_diff = actual_block_height.checked_sub(block_height);
    if !matches!(block_height_diff, Some(0..=20)) {
        // block_height is invalid
        return Json(json!({
            "status": "error",
            "message": format!("block_height is invalid"),
        }));
    }

    let (transaction, key) = if let Some(data) = state
        .db
        .find_by_mint_address(&mint_address.to_string())
        .await
        .unwrap()
    {
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

    // Check that witdraw destination is not one of ours BTC multisig addresses
    let known_multisig_addresses = state.db.get_all_multisig_addresses().await;
    assert!(!known_multisig_addresses.contains(&withdraw_address));

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

    let fee_rate = if let Some(sat_per_vb) = fee_rate {
        FeeRate::from_sat_per_vb(sat_per_vb.as_f64().unwrap() as f32)
    } else {
        get_recommended_fee_rate(btc_network).await
    };

    let (onesig_psbt, fee) = cli
        .onesig(
            xprv_00, xpub_01, xpub_02, xpub_03, to_address, amount, fee_rate,
        )
        .await;
    // let onesig_psbt = onesig(&descriptor_00, xpub_01, xpub_02, to_address, amount).await;
    info!("onesig_psbt: {:#?}", &onesig_psbt);

    if let Some(expected_vbytes) = vbytes {
        let actual_vbytes = get_vbytes(fee, fee_rate);
        debug!(
            "fee: {fee}, fee_rate: {fee_rate}, actual_vbytes: {actual_vbytes}, expected_vbytes: {expected_vbytes}",
            fee_rate=fee_rate.as_sat_per_vb(),
        );

        if actual_vbytes != expected_vbytes {
            return Json(json!({
                "status": "error",
                "message": format!("vbytes is different from expected: expected {expected_vbytes}, found {actual_vbytes}"),
            }));
        }
    }

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

    let account_address = get_account_address(mint_address);
    info!("Burn system account_address: {account_address:?}");
    let amount_tokens: u64 = withdraw_amount.parse().unwrap();
    burn_token_inner(&state.config, mint_address, amount_tokens).await;

    let tx_id = cli
        .send(xpub_00, xpub_01, xpub_02, xpub_03, &thirdsig_psbt)
        .await;
    // let tx_id = send(&multi_descriptor_01, &secondsig_psbt).await;

    let mempool_url = get_mempool_url(btc_network);

    let tx_link = format!("{mempool_url}/tx/{tx_id}");
    info!("transaction sent: {tx_link}");
    Json(json!({
        "status": "ok",
        "thirdsig_psbt": thirdsig_psbt,
        "tx_id": tx_id,
        "tx_link": tx_link,
    }))
}

async fn verify_request_signature(
    domichain_rpc_url: &Url,
    spl_token_program_id: Pubkey,
    request: &SignMultisigTxRequest,
) -> Result<(), String> {
    let SignMultisigTxRequest {
        mint_address,
        withdraw_address,
        withdraw_amount,
        fee_rate,
        vbytes,
        domi_address,
        block_height,
        btci_tx_signature,
        signature,
    } = request;

    let btci_tx = get_transaction_poll(domichain_rpc_url.clone(), *btci_tx_signature).await;

    // Verify is success transaction
    assert!(btci_tx.meta.err.is_null());
    assert!(btci_tx.meta.status == json!({"Ok": null}));

    // TODO: check slot is recent

    // Verify only one signer
    assert_eq!(btci_tx.transaction.signatures.len(), 1);
    assert_eq!(btci_tx.transaction.signatures[0].0, *btci_tx_signature);
    // Get token transfer instruction
    let mut ixs = btci_tx
        .transaction
        .message
        .instructions
        .into_iter()
        .filter(|ix| &ix.program == "spl-token" && ix.program_id == spl_token_program_id);
    let ix = ixs.next().unwrap();
    assert!(ixs.next().is_none());

    assert_eq!(&ix.parsed.instruction_type, "transferChecked");
    let info: DomiTransactionInstructionInfo = serde_json::from_value(ix.parsed.info).unwrap();
    // Verify transfer authority is request sender
    assert_eq!(&info.authority, domi_address);
    // Verify transfer destination is service account
    let service_token_account = get_account_address(*mint_address);
    assert_eq!(info.destination, service_token_account);
    // Verify mint address
    assert_eq!(info.mint, *mint_address);
    // Verify BTCi token amount is same as in request
    assert_eq!(
        info.token_amount["amount"].as_str().unwrap(),
        withdraw_amount
    );

    let request_body = json!({
        "mint_address": mint_address.to_string(),
        "withdraw_address": withdraw_address,
        "withdraw_amount": withdraw_amount,
        "fee_rate": fee_rate,
        "vbytes": vbytes,
        "domi_address": domi_address.to_string(),
        "block_height": block_height,
        "btci_tx_signature": btci_tx_signature.to_string(),
    });
    let request_body_str = serde_json::to_string(&request_body).unwrap();

    // Verify `signature` is for `request_body` and `domi_address`
    if !signature.verify(domi_address.as_ref(), request_body_str.as_bytes()) {
        #[allow(dead_code)]
        #[derive(Debug)]
        struct DebugLog {
            signature: Signature,
            domi_address: Pubkey,
            request_body_str: String,
            request_body: Value,
        }
        debug!(
            "Failed to verify: {:#?}",
            DebugLog {
                signature: *signature,
                domi_address: *domi_address,
                request_body_str,
                request_body,
            }
        );
        return Err("Failed to verify".to_string());
    }

    Ok(())
}

#[test]
fn test_verify_request_signature() {
    // await temp1.createSignature(new TextEncoder().encode(JSON.stringify({a:5,b:6})))
    // 4LaPQGRQHbjNyv4ET9CtDiPCBoZSBgXLSesmXAwtNKB7n1HYdbRNTiKg6T3YTitHPsuW31gAyiyJ4i3PHgZSGkZK
    // temp1.provider.publicKey.toString()
    // 2832BAbZJ1WoZ56DzgpWX7dhvwnqRdKdNpC2oUpYyDjH

    let signature: Signature =
        "4LaPQGRQHbjNyv4ET9CtDiPCBoZSBgXLSesmXAwtNKB7n1HYdbRNTiKg6T3YTitHPsuW31gAyiyJ4i3PHgZSGkZK"
            .parse()
            .unwrap();
    let domi_address: Pubkey = "2832BAbZJ1WoZ56DzgpWX7dhvwnqRdKdNpC2oUpYyDjH"
        .parse()
        .unwrap();

    let request_body = json!({"a":5,"b":6});
    let request_body_str = serde_json::to_string(&request_body).unwrap();

    // let message = domichain_sdk::offchain_message::OffchainMessage::new(0, request_body_str.as_bytes()).unwrap();

    // Verify `signature` is for `request_body` and `domi_address`
    assert!(signature.verify(domi_address.as_ref(), request_body_str.as_bytes()));

    let domi_address_wrong: Pubkey = "2832BAbZJ1WoZ56DzgpWX7dhvwqqRdKdNpC2oUpYyDjH"
        .parse()
        .unwrap();
    let signature_wrong: Signature =
        "4LaPQGRQHbjNyv4ET9CtDiPCBoZSBgXLSesmXAwtNKB7n1HYdbRhTiKg6T3YTitHPsuW31gAyiyJ4i3PHgZSGkZK"
            .parse()
            .unwrap();
    let request_body_wrong = json!({"a":5,"b":7});
    let request_body_str_wrong = serde_json::to_string(&request_body_wrong).unwrap();
    assert!(!signature.verify(domi_address_wrong.as_ref(), request_body_str.as_bytes()));
    assert!(!signature_wrong.verify(domi_address.as_ref(), request_body_str.as_bytes()));
    assert!(!signature.verify(domi_address.as_ref(), request_body_str_wrong.as_bytes()));
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
