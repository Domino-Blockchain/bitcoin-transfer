use std::{num::Saturating, time::Duration};

use anyhow::{anyhow, bail};
use axum::{extract::State, Json};
use mongodb::bson::{doc, Document};
use serde::Deserialize;
use serde_json::json;
use tokio::time::sleep;

use crate::{
    db::DB,
    mint_token::{mint_token_inner, MintTokenResult},
    AppState,
};

const MEMPOOL_TESTNET_URL: &str = "https://mempool.space/testnet/api";

#[derive(Deserialize)]
pub struct WatchTxRequest {
    tx_hash: String,
    btc_deposit_address: String,
    domi_address: String,
}

pub async fn watch_tx(
    State(state): State<AppState>,
    Json(request): Json<WatchTxRequest>,
) -> Json<serde_json::Value> {
    let WatchTxRequest {
        tx_hash,
        btc_deposit_address,
        domi_address,
    } = request;
    let url = format!("{MEMPOOL_TESTNET_URL}/tx/{tx_hash}");
    let body = match get_tx_data(&url).await {
        Err(error) => {
            return Json(json!({
                "status": "error",
                "error": format!("Get TX data error: {error}"),
            }))
        }
        Ok(body) => body,
    };
    if let Err(error) = get_value(body, &btc_deposit_address).await {
        return Json(json!({
            "status": "error",
            "error": format!("Verify TX error: {error}"),
        }));
    }
    if let Err(error) = get_address_metadata(&state.db, &btc_deposit_address).await {
        return Json(json!({
            "status": "error",
            "error": format!("Check TX error: {error}"),
        }));
    }

    tokio::spawn(async move {
        loop {
            match poll_is_tx_confirmed(&tx_hash).await {
                Ok(true) => {
                    dbg!("TX is confirmed", &tx_hash);
                    process_confirmed(&state, &url, &tx_hash, &btc_deposit_address, &domi_address)
                        .await
                        .unwrap();
                    break;
                }
                Ok(false) => {
                    dbg!("TX is not confirmed. Polling", &tx_hash);
                }
                Err(error) => {
                    eprintln!(
                        "[{}:{}] Error on polling confirmation: {error}",
                        file!(),
                        line!(),
                    );
                }
            }
            sleep(Duration::from_secs(3)).await
        }
    });
    Json(json!({"status": "ok"}))
}

async fn process_confirmed(
    state: &AppState,
    url: &str,
    tx_hash: &str,
    deposit_address: &str,
    domi_address: &str,
) -> anyhow::Result<()> {
    let body: serde_json::Value = reqwest::get(url).await?.json().await?;
    let value = get_value(body, &deposit_address).await?;
    let meta = get_address_metadata(&state.db, &deposit_address).await?;
    dbg!("Confirmed", &value, &meta);

    // TODO: update metadata
    let res = state
        .db
        .update_by_deposit_address(
            deposit_address,
            doc! {
                "confirmed": true,
                "tx_hash": tx_hash,
                "value": &value,
            },
        )
        .await
        .unwrap();
    assert_eq!(res.matched_count, 1);
    assert_eq!(res.modified_count, 1);
    assert_eq!(res.upserted_id, None);

    // TODO: mint token
    let mint_result = mint_token_inner(&value, domi_address).await.unwrap();
    dbg!(&mint_result);
    let MintTokenResult {
        mint_address,
        account_address,
        output: _,
    } = mint_result;
    let res = state
        .db
        .update_by_deposit_address(
            deposit_address,
            doc! {
                "minted": true,
                "mint_address": mint_address,
                "account_address": account_address,
                "domi_address": domi_address,
            },
        )
        .await
        .unwrap();
    assert_eq!(res.matched_count, 1);
    assert_eq!(res.modified_count, 1);
    assert_eq!(res.upserted_id, None);
    Ok(())
}

async fn get_tx_data(url: &str) -> anyhow::Result<serde_json::Value> {
    let sleep_duration = Duration::from_secs(2);
    let mut attempts: Saturating<u8> = Saturating(5);
    loop {
        let result = reqwest::get(url).await?;
        if result.status().is_success() {
            return Ok(result.json().await?);
        }
        if result.status().as_u16() == 404 {
            attempts -= 1;
            if attempts.0 == 0 {
                let if_json: Result<serde_json::Value, _> = result.json().await;
                bail!("Unable to request TX data. Got 404: {if_json:?}");
            }
            sleep(sleep_duration).await;
            continue;
        }
        let status = result.status();
        let if_json: Result<serde_json::Value, _> = result.json().await;
        bail!("Unable to request TX data. Got {status}: {if_json:?}");
    }
}

async fn get_address_metadata(db: &DB, deposit_address: &str) -> anyhow::Result<Document> {
    let meta = db.find_by_deposit_address(deposit_address).await?;
    meta.ok_or_else(|| anyhow!("Deposit address data not found"))
}

// https://mempool.space/docs/api/rest#get-transaction
async fn get_value(body: serde_json::Value, deposit_address: &str) -> anyhow::Result<String> {
    let vout = body["vout"].as_array().ok_or(anyhow!("Not array"))?;
    if let Some(destination) = vout.iter().find(|&destination| {
        let address = destination["scriptpubkey_address"].as_str().unwrap();
        address == deposit_address
    }) {
        let value = destination["value"].as_number().unwrap();
        Ok(value.to_string())
    } else {
        Err(anyhow!(
            "Address don't match: {deposit_address} is not in vout"
        ))
    }
}

// https://mempool.space/docs/api/rest#get-transaction-status
async fn poll_is_tx_confirmed(tx_hash: &str) -> anyhow::Result<bool> {
    let body: serde_json::Value =
        reqwest::get(format!("{MEMPOOL_TESTNET_URL}/tx/{tx_hash}/status"))
            .await?
            .json()
            .await?;
    let confirmed = body["confirmed"].as_bool().ok_or(anyhow!("Not bool"))?;
    Ok(confirmed)
}
