use std::{collections::HashSet, sync::Arc, time::Duration};

use futures::{SinkExt, StreamExt, TryStreamExt};
use mongodb::bson::doc;
use serde_json::json;
use tokio::{
    select,
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
    time::{interval, sleep},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info};

use crate::{db::DB, mempool::get_mempool_ws_url, mint_token::mint_token_inner};

const MEMPOOL_CHANNEL_LIMIT: usize = 10;
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

struct Channel {
    id: String,
    addresses: Vec<String>,
}

impl Channel {
    fn new(id: String, addresses: Vec<String>) -> Self {
        assert!(!addresses.is_empty());
        assert!(addresses.len() <= MEMPOOL_CHANNEL_LIMIT);
        // TODO: start connection

        Self { id, addresses }
    }

    fn push(&mut self, address: String) -> Result<(), String> {
        if !self.is_full() {
            todo!()
        } else {
            Err(address)
        }
    }

    fn is_full(&self) -> bool {
        self.addresses.len() >= MEMPOOL_CHANNEL_LIMIT
    }
}

pub struct MempoolWatcher {
    channels: Vec<Channel>,
}

impl MempoolWatcher {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
        }
    }

    pub fn push(&mut self, address: String) {
        if self.channels.is_empty() || self.channels.last().unwrap().is_full() {
            self.channels.push(Channel::new(
                format!("{}", self.channels.len()),
                vec![address],
            ));
        } else {
            self.channels.last_mut().unwrap().push(address).unwrap();
        }
    }
}

pub struct Update {
    address: String,
    new_tx_id: String,
}

pub async fn watch_addresses(
    watch_id: usize,
    addresses: Vec<String>,
    mut subscribe: UnboundedReceiver<String>,
    unsubscribe: UnboundedReceiver<String>,
    on_update: UnboundedSender<Update>,
    btc_network: bdk::bitcoin::Network,
) -> (JoinHandle<()>, JoinHandle<()>) {
    let (ws_stream, _) = connect_async(get_mempool_ws_url(btc_network))
        .await
        .expect("Failed to connect");

    let (mut ws_write, ws_read) = ws_stream.split();

    let init = json!({"action": "init"}).to_string();
    ws_write.send(Message::text(init)).await.unwrap();
    let track_addresses = json!({"track-addresses": addresses}).to_string();
    ws_write.send(Message::text(track_addresses)).await.unwrap();

    let write_handle = tokio::spawn(async move {
        let mut ping_interval = interval(PING_INTERVAL);
        ping_interval.tick().await;

        let handle_sub = |address| async {};
        // let mut handle_interval = |ws_write_mut| async move {
        //     ws_write_mut
        //         .send(Message::text(json!({"action": "ping"}).to_string()))
        //         .await
        //         .unwrap();
        // };

        loop {
            select! {
                address = subscribe.recv() => handle_sub(address).await,
                // _ = ping_interval.tick() => handle_interval(&mut ws_write).await,
                _ = ping_interval.tick() => {
                    ws_write
                        .send(Message::text(json!({"action": "ping"}).to_string()))
                        .await
                        .unwrap();
                },
            };

            // match subscribe.try_recv() {
            //     Ok(address) => {
            //         ws_write
            //             .send(Message::text(json!({"track-address": address}).to_string()))
            //             .await
            //             .unwrap();
            //     }
            //     Err(TryRecvError::Empty) => {}
            //     Err(TryRecvError::Disconnected) => panic!("Disconnected"),
            // }
        }
    });

    let read_handle = tokio::spawn(async move {
        ws_read
            .try_for_each(|msg| async {
                match msg {
                    Message::Text(msg) => {
                        let mut msg_json: serde_json::Value = serde_json::from_str(&msg).unwrap();
                        let msg_object = msg_json.as_object_mut().unwrap();
                        let keys: HashSet<_> = msg_object.keys().map(|s| s.to_string()).collect();
                        msg_object.retain(|k, _| !IGNORED_KEYS.contains(&k.as_str()));
                        // disable auto messages
                        println!(
                            "{watch_id} got: {:?} {}",
                            keys,
                            serde_json::to_string_pretty(&msg_json).unwrap(),
                        );
                    }
                    other => panic!("expected a text message but got {other:?}"),
                }
                Ok(())
            })
            .await
            .unwrap();
    });
    // write_handle.await.unwrap();
    // read_handle.await.unwrap();
    (read_handle, write_handle)
}

pub async fn watch_address(address: String, db: Arc<DB>, btc_network: bdk::bitcoin::Network) {
    loop {
        let db = db.clone();
        let address = address.clone();
        info!("Subscribing on {address}");
        let (ws_stream, _) = connect_async(get_mempool_ws_url(btc_network))
            .await
            .expect("Failed to connect");

        let (mut ws_write, ws_read) = ws_stream.split();

        let init = json!({"action": "init"}).to_string();
        ws_write.send(Message::text(init)).await.unwrap();
        let track_addresses = json!({"track-addresses": [&address]}).to_string();
        ws_write.send(Message::text(track_addresses)).await.unwrap();

        let write_handle = tokio::spawn(async move {
            let mut ping_interval = interval(PING_INTERVAL);
            ping_interval.tick().await;

            loop {
                ping_interval.tick().await;
                ws_write
                    .send(Message::text(json!({"action": "ping"}).to_string()))
                    .await
                    .unwrap();
            }
        });

        let read_handle = tokio::spawn(async move {
            ws_read
                .try_for_each(|msg| async {
                    match msg {
                        Message::Text(msg) => {
                            let mut msg_json: serde_json::Value =
                                serde_json::from_str(&msg).unwrap();
                            let msg_object = msg_json.as_object_mut().unwrap();
                            let keys: HashSet<_> =
                                msg_object.keys().map(|s| s.to_string()).collect();
                            msg_object.retain(|k, _| !IGNORED_KEYS.contains(&k.as_str()));
                            // disable auto messages
                            info!(
                                "got: {:?} {}",
                                keys,
                                serde_json::to_string_pretty(&msg_json).unwrap(),
                            );
                            // Get amount from TX
                            // Get multisig address by TX info
                            // Get domi address by multisig address
                            let msg_object = msg_json.as_object_mut().unwrap();
                            if msg_object.contains_key("multi-address-transactions") {
                                let addresses = msg_object["multi-address-transactions"]
                                    .as_object()
                                    .unwrap();
                                let address_state = &addresses[&address];
                                let confirmed = address_state["confirmed"].as_array().unwrap();
                                if !confirmed.is_empty() {
                                    let data = db
                                        .find_by_deposit_address(&address)
                                        .await
                                        .unwrap()
                                        .expect("multisig address doesn't found");
                                    let domi_address = data.get_str("domi_address").unwrap();

                                    let vout = confirmed[0]["vout"].as_array().unwrap();
                                    assert_eq!(
                                        vout.iter()
                                            .filter(|dest| dest["scriptpubkey_address"]
                                                .as_str()
                                                .unwrap()
                                                == &address)
                                            .count(),
                                        1
                                    );
                                    let address_vout = vout
                                        .iter()
                                        .filter(|dest| {
                                            dest["scriptpubkey_address"].as_str().unwrap()
                                                == &address
                                        })
                                        .next()
                                        .unwrap();
                                    let value = address_vout["value"].as_number().unwrap();
                                    if !value.is_u64() {
                                        todo!("value is wrong type: {value:?}");
                                    }
                                    let value = value.as_u64().unwrap();

                                    // TODO: save in DB
                                    let tx_hash = confirmed[0]["txid"].as_str().unwrap();
                                    // FIXME: insert instead
                                    let insert_result = db
                                        .insert_tx(doc! {
                                            "tx_hash": tx_hash,
                                            "confirmed": true,
                                            "multi_address": &address,
                                            "value": value.to_string(),
                                        })
                                        .await
                                        .unwrap();
                                    info!("Inserted TX. DB ID: {}", insert_result.inserted_id);

                                    // TODO: mint token
                                    // Fail if:
                                    // - network issue
                                    // - insufficient balance
                                    // - address already exists
                                    let mint_result =
                                        mint_token_inner(&value.to_string(), domi_address).await;
                                    info!("mint_result: {mint_result:#?}");
                                    let mint_result = mint_result.unwrap();

                                    // TODO: save mint result
                                    let res = db
                                        .update_tx(
                                            insert_result.inserted_id,
                                            doc! {
                                                "minted": true,
                                                "mint_address": mint_result.mint_address,
                                                "account_address": mint_result.account_address,
                                                "domi_address": domi_address
                                            },
                                        )
                                        .await
                                        .unwrap();
                                    assert_eq!(res.matched_count, 1);
                                    assert_eq!(res.modified_count, 1);
                                    assert_eq!(res.upserted_id, None);

                                    if confirmed.len() > 1 {
                                        todo!();
                                    }
                                }
                            }
                        }
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

        sleep(Duration::from_secs(1)).await;
    }
}
