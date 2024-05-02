use std::{collections::HashSet, time::Duration};

use aws_sdk_kms::config::IntoShared;
use futures::{SinkExt, StreamExt, TryStreamExt};
use serde_json::json;
use tokio::{
    pin, select,
    sync::mpsc::{error::TryRecvError, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
    time::{interval, sleep},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const MEMPOOL_MAINNET_WS_URL: &str = "wss://mempool.space/api/v1/ws";
const MEMPOOL_TESTNET_WS_URL: &str = "wss://mempool.space/testnet/api/v1/ws";

const MEMPOOL_CHANNEL_LIMIT: usize = 10;
const PING_INTERVAL: Duration = Duration::from_secs(60);

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
) -> (JoinHandle<()>, JoinHandle<()>) {
    let (ws_stream, _) = connect_async(MEMPOOL_TESTNET_WS_URL)
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
        // let handle_interval = || async {
        //     ws_write
        //         .send(Message::text(json!({"action": "ping"}).to_string()))
        //         .await
        //         .unwrap();
        // };

        loop {
            select! {
                address = subscribe.recv() => handle_sub(address).await,
                // _ = ping_interval.tick() => handle_interval().await,
            };

            match subscribe.try_recv() {
                Ok(address) => {
                    ws_write
                        .send(Message::text(json!({"track-address": address}).to_string()))
                        .await
                        .unwrap();
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => panic!("Disconnected"),
            }
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
