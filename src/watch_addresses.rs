use std::{collections::HashSet, time::Duration};

use futures::{SinkExt, StreamExt, TryStreamExt};
use serde_json::json;
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    time::sleep,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const MEMPOOL_MAINNET_WS_URL: &str = "wss://mempool.space/api/v1/ws";
const MEMPOOL_TESTNET_WS_URL: &str = "wss://mempool.space/testnet/api/v1/ws";

pub struct Update {
    address: String,
    new_tx_id: String,
}

pub async fn watch_addresses(
    addresses: Vec<String>,
    // subscribe: UnboundedReceiver<String>,
    // on_update: UnboundedSender<Update>,
    watch_id: usize,
) {
    let (ws_stream, _) = connect_async(MEMPOOL_TESTNET_WS_URL)
        .await
        .expect("Failed to connect");

    let (mut ws_write, ws_read) = ws_stream.split();

    let init = json!({"action": "init"}).to_string();
    dbg!(&init);
    ws_write.send(Message::text(init)).await.unwrap();
    let track_addresses = json!({"track-addresses": addresses}).to_string();
    dbg!(&track_addresses);
    ws_write.send(Message::text(track_addresses)).await.unwrap();

    let write_handle = tokio::spawn(async move {
        loop {
            ws_write
                .send(Message::text(json!({"action": "ping"}).to_string()))
                .await
                .unwrap();
            sleep(Duration::from_secs(10)).await;
        }
    });

    ws_read
        .try_for_each(|msg| async {
            match msg {
                Message::Text(msg) => {
                    let mut msg_json: serde_json::Value = serde_json::from_str(&msg).unwrap();
                    let msg_object = msg_json.as_object_mut().unwrap();
                    let keys: HashSet<_> = msg_object.keys().map(|s| s.to_string()).collect();
                    let remove_keys = [
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
                    ];
                    msg_object.retain(|k, _| !remove_keys.contains(&k.as_str()));
                    msg_object.remove("pong");
                    // disable auto messages
                    println!(
                        "{watch_id} got: {:?} {}",
                        keys,
                        serde_json::to_string_pretty(&msg_json).unwrap(),
                    );

                    // if msg_json.get("pong").is_some() {
                    //     println!("Got: pong");
                    // } else if msg_json.get("mempoolInfo").is_some() {
                    //     println!(
                    //         "Got: mempoolInfo {} len",
                    //         serde_json::to_string_pretty(&msg_json).unwrap().len()
                    //     );
                    // } else if msg_json.get("conversions").is_some() {
                    //     println!(
                    //         "Got: conversions {} len",
                    //         serde_json::to_string_pretty(&msg_json).unwrap().len()
                    //     );
                    // } else {
                    //     println!("Got: {}", serde_json::to_string_pretty(&msg_json).unwrap());
                    // }
                }
                other => panic!("expected a text message but got {other:?}"),
            }
            Ok(())
        })
        .await
        .unwrap();

    write_handle.await.unwrap();
}
