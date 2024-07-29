use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};

use futures::{SinkExt, StreamExt, TryStreamExt};
use mongodb::{bson::doc, results::InsertOneResult};
use serde::Deserialize;
use serde_json::json;
use tokio::time::{interval, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info};

use crate::{db::DB, mempool::get_mempool_ws_url, mint_token::mint_token_inner};

const PING_INTERVAL: Duration = Duration::from_secs(30);

const IGNORED_KEYS: &[&str] = &[
    "mempool-blocks",
    "transactions",
    "vBytesPerSecond",
    "rbfSummary",
    "da",
    "backend",
    "blocks",
    "mempoolInfo",
    "loadingIndicators",
    "conversions",
    "backendInfo",
    "fees",
    "pong",
];

pub async fn watch_address(address: String, db: Arc<DB>, btc_network: bdk::bitcoin::Network) {
    let mut sleep_duration = Duration::from_secs(1);
    let mut last_sleep = Instant::now();
    let sleep_reset_interval = Duration::from_secs(10 * 60); // 10min
    let mut sleep_duration = std::iter::from_fn(|| {
        if last_sleep.elapsed() > sleep_reset_interval {
            // Reset sleep
            sleep_duration = Duration::from_secs(1);
        } else {
            sleep_duration = sleep_duration
                .checked_mul(2)
                .unwrap()
                .checked_add(Duration::from_millis(500))
                .unwrap();
        }
        last_sleep = Instant::now();
        Some(sleep_duration)
    });

    // Infinite loop to retry subscription on errors
    loop {
        let db = db.clone();
        let address_clone = address.clone();
        let address_clone_2 = address.clone();
        info!("Subscribing on {address}");

        let connect_handle = tokio::spawn(async move {
            let address = address_clone;
            let (ws_stream, _) = connect_async(get_mempool_ws_url(btc_network))
                .await
                .expect("Failed to connect");

            let (mut ws_write, ws_read) = ws_stream.split();

            let init = json!({"action": "init"}).to_string();
            ws_write.send(Message::text(init)).await.unwrap();
            let track_addresses = json!({"track-addresses": [&address]}).to_string();
            ws_write.send(Message::text(track_addresses)).await.unwrap();

            (ws_write, ws_read)
        });

        let (mut ws_write, ws_read) = match connect_handle.await {
            Ok((ws_write, ws_read)) => (ws_write, ws_read),
            Err(connect_error) => {
                error!("WebSocket connect_error: {connect_error:?}");
                sleep(sleep_duration.next().unwrap()).await;
                continue;
            }
        };

        let write_handle = tokio::spawn(async move {
            let mut ping_interval = interval(PING_INTERVAL);

            loop {
                ping_interval.tick().await;
                ws_write
                    .send(Message::text(json!({"action": "ping"}).to_string()))
                    .await
                    .unwrap();
            }
        });

        let read_handle = tokio::spawn(async move {
            let address = address_clone_2;
            ws_read
                .try_for_each(|msg| async {
                    match msg {
                        Message::Text(msg) => handle_text_message(&db, &address, &msg).await,
                        other => panic!("expected a text message but got {other:?}"),
                    }
                    Ok(())
                })
                .await
                .unwrap();
        });

        if let Err(write_error) = write_handle.await {
            error!("WebSocket write_error: {write_error:?}");
        }
        if let Err(read_error) = read_handle.await {
            error!("WebSocket read_error: {read_error:?}");
        }

        sleep(sleep_duration.next().unwrap()).await;
    }
}

pub async fn handle_text_message(db: &DB, address: &str, msg: &str) {
    let mut msg_json: serde_json::Value = serde_json::from_str(msg).unwrap();
    let msg_object = msg_json.as_object_mut().unwrap();
    let keys: HashSet<_> = msg_object.keys().map(|s| s.to_string()).collect();
    // disable auto messages
    msg_object.retain(|k, _| !IGNORED_KEYS.contains(&k.as_str()));
    if !msg_object.is_empty() {
        info!(
            "got: {keys:?} {}",
            serde_json::to_string_pretty(msg_object).unwrap(),
        );
    }
    // Get amount from TX
    // Get multisig address by TX info
    // Get domi address by multisig address
    if let Some(addresses) = msg_object.get("multi-address-transactions") {
        let confirmed = addresses[&address]["confirmed"].as_array().unwrap();
        if !confirmed.is_empty() {
            let confirmed_tx = serde_json::from_value(confirmed[0].clone()).unwrap();
            process_confirmed_transaction(&db, &address, confirmed_tx).await;

            if confirmed.len() > 1 {
                todo!("Confirmed TXs array have multiple entries");
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct VinPrevout {
    pub scriptpubkey_address: String,
}

#[derive(Debug, Deserialize)]
pub struct Vin {
    pub prevout: VinPrevout,
}

#[derive(Debug, Deserialize)]
pub struct Vout {
    pub scriptpubkey_address: String,
    pub value: u64,
}

#[derive(Debug, Deserialize)]
pub struct Confirmed {
    pub txid: String,
    pub vin: Vec<Vin>,
    pub vout: Vec<Vout>,
}

/// Find deposit amount and do the BTCi mint
pub async fn process_confirmed_transaction(db: &DB, multi_address: &str, confirmed: Confirmed) {
    // Find corresponding DOMI address
    let data = db
        .find_by_deposit_address(multi_address)
        .await
        .unwrap()
        .expect("multisig address doesn't found");
    let domi_address = data.get_str("domi_address").unwrap();

    // Check that this deposit is from external address
    let vin = confirmed.vin;
    let known_multisig_addresses: HashSet<String> =
        HashSet::from_iter(db.get_all_multisig_addresses().await);
    assert!(!vin
        .iter()
        .any(|vin| known_multisig_addresses.contains(&vin.prevout.scriptpubkey_address)));

    // Get TX output and value in sat
    let vout = confirmed.vout;
    // Check that transaction have only one our multisig address in output
    let mut vouts = vout
        .iter()
        .filter(|dest| dest.scriptpubkey_address == multi_address);
    let address_vout = vouts.next().unwrap();
    assert!(vouts.next().is_none());

    let value = address_vout.value;

    let tx_hash = confirmed.txid;
    let InsertOneResult { inserted_id, .. } = db
        .insert_tx(doc! {
            "tx_hash": tx_hash,
            "confirmed": true,
            "multi_address": multi_address,
            "value": value.to_string(),
        })
        .await
        .unwrap();
    info!("Inserted TX. DB ID: {inserted_id}");

    // Fail if:
    // - network issue
    // - insufficient balance
    // - address already exists
    let mint_result = mint_token_inner(&value.to_string(), domi_address).await;
    info!("mint_result: {mint_result:#?}");
    let mint_result = mint_result.unwrap();

    let res = db
        .update_tx(
            inserted_id,
            doc! {
                "minted": true,
                "mint_address": mint_result.mint_address,
                "account_address": mint_result.account_address,
                "domi_address": domi_address,
            },
        )
        .await
        .unwrap();
    assert_eq!(res.matched_count, 1);
    assert_eq!(res.modified_count, 1);
    assert_eq!(res.upserted_id, None);
}
