use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use serde::Deserialize;
use tokio::time::{sleep, timeout};

use crate::{get_json, REQUEST_TIMEOUT};

/*
{
    "txid": "3a3774433c15d8c1791806d25043335c2a53e5c0ed19517defa4dba9d0b2019f",
    "version": 1,
    "locktime": 0,
    "vin": [
      {
        "txid": "0cd6631c09e7b33d031f8d327b98154dd5829e0c9143ffa5e4860c9a4953e8f3",
        "vout": 1,
        "prevout": {
          "scriptpubkey": "00148d097c52d7d807a051c69e74de35518b859492b1",
          "scriptpubkey_asm": "OP_0 OP_PUSHBYTES_20 8d097c52d7d807a051c69e74de35518b859492b1",
          "scriptpubkey_type": "v0_p2wpkh",
          "scriptpubkey_address": "bc1q35yhc5khmqr6q5wxne6dud233wzefy43k4w9sv",
          "value": 51290
        },
        "scriptsig": "",
        "scriptsig_asm": "",
        "witness": [
          "3043021f34c3c27dcd10346921424c1a7ca35bea87e1ac85d7ab8351525d623369d4990220370108a2e1a8253d2768d53e6df799b4cdb37d4ff77590b297217b08d535de1901",
          "02dcea820c0e9bfa38dcf158b59f6d2ad44e43baf81cbdcf476d4521d81a56d0c1"
        ],
        "is_coinbase": false,
        "sequence": 4294967295
      }
    ],
    "vout": [
      {
        "scriptpubkey": "76a9140a5981e5cc40050ba3f8974afc96ba2d3469a8b288ac",
        "scriptpubkey_asm": "OP_DUP OP_HASH160 OP_PUSHBYTES_20 0a5981e5cc40050ba3f8974afc96ba2d3469a8b2 OP_EQUALVERIFY OP_CHECKSIG",
        "scriptpubkey_type": "p2pkh",
        "scriptpubkey_address": "1wiz18xYmhRX6xStj2b9t1rwWX4GKUgpv",
        "value": 30236
      },
    ],
    "size": 224,
    "weight": 572,
    "sigops": 5,
    "fee": 7200,
    "status": {
      "confirmed": true,
      "block_height": 840719,
      "block_hash": "0000000000000000000170deaa4ccf2de2f1c94346dfef40318d0a7c5178ffd3",
      "block_time": 1713994081
    }
}
*/

#[derive(Deserialize, Debug)]
pub struct VinPrevout {
    pub scriptpubkey: String,
    pub scriptpubkey_asm: String,
    pub scriptpubkey_type: String,
    pub scriptpubkey_address: String,
    pub value: u64,
}

#[derive(Deserialize, Debug)]
pub struct Vin {
    pub txid: String,
    pub vout: usize,
    pub prevout: VinPrevout,
    pub scriptsig: String,
    pub scriptsig_asm: String,
    pub witness: Option<Vec<String>>,
    pub is_coinbase: bool,
    pub sequence: u64,
}

#[derive(Deserialize, Debug)]
pub struct Vout {
    pub scriptpubkey: String,
    pub scriptpubkey_asm: String,
    pub scriptpubkey_type: String,
    pub scriptpubkey_address: Option<String>,
    pub value: u64,
}

#[derive(Deserialize, Debug)]
pub struct Status {
    pub confirmed: bool,
    pub block_height: u64,
    pub block_hash: String,
    pub block_time: u64,
}

#[derive(Deserialize, Debug)]
pub struct Transaction {
    pub txid: String,
    pub version: usize,
    pub locktime: u64,
    pub vin: Vec<Vin>,
    pub vout: Vec<Vout>,
    pub size: usize,
    pub weight: u64,
    pub sigops: usize,
    pub fee: u64,
    pub status: Status,
}

#[derive(Deserialize, Debug)]
pub struct AddressStats {
    funded_txo_count: usize,
    funded_txo_sum: u64,
    spent_txo_count: usize,
    spent_txo_sum: u64,
    tx_count: usize,
}

#[derive(Deserialize, Debug)]
pub struct Address {
    address: String,
    chain_stats: AddressStats,
    mempool_stats: AddressStats,
}

/// See: https://mempool.space/docs/api/rest#get-address-transactions
pub async fn get_address(address: &str) -> Result<Address, reqwest::Error> {
    timeout(
        REQUEST_TIMEOUT,
        get_json(format!("https://mempool.space/api/address/{address}")),
    )
    .await
    .unwrap()
}

/// See: https://mempool.space/docs/api/rest#get-address-transactions
pub async fn get_address_txs_chain(address: &str) -> Result<Vec<Transaction>, reqwest::Error> {
    let use_timeout = true;
    let use_sleep = false;

    // let mut sleep_duration = Duration::from_secs(1);

    let address_data = get_address(address).await.unwrap();
    if use_sleep {
        sleep(Duration::from_millis(500)).await;
    }

    dbg!(&address_data);

    let mut all_txs: Vec<Transaction> = Vec::with_capacity(address_data.chain_stats.tx_count);
    loop {
        let after_txid = all_txs
            .last()
            .map(|last_tx| format!("?after_txid={}", &last_tx.txid))
            .unwrap_or_default();
        let start = Instant::now();

        let mut new_page: Vec<Transaction>;
        if use_timeout {
            new_page = timeout(
                REQUEST_TIMEOUT,
                get_json(format!(
                    "https://mempool.space/api/address/{address}/txs{after_txid}"
                )),
            )
            .await
            .unwrap()?;
        } else {
            new_page = get_json(format!(
                "https://mempool.space/api/address/{address}/txs{after_txid}"
            ))
            .await?;
        }

        // Only confirmed
        new_page.retain(|tx| tx.status.confirmed);

        dbg!(start.elapsed());
        let is_last_page = new_page.len() < 25;

        all_txs.extend(new_page);
        // Assert all unique
        let unique_txids: HashSet<_> = all_txs.iter().map(|tx| tx.txid.as_str()).collect();
        assert_eq!(all_txs.len(), unique_txids.len());

        if is_last_page {
            break;
        }

        if use_sleep {
            sleep(Duration::from_millis(1000)).await;
        }
    }

    dbg!(all_txs.len());

    Ok(all_txs)
}

#[tokio::test]
async fn test_address_txs_chain() {
    // dbg!(address_txs_chain("1wiz18xYmhRX6xStj2b9t1rwWX4GKUgpv")
    //     .await
    //     .unwrap());

    // dbg!(
    get_address_txs_chain("bc1q35yhc5khmqr6q5wxne6dud233wzefy43k4w9sv")
        .await
        .unwrap();
    // );
}

#[tokio::test]
async fn test_address_txs_chain_verbose() {
    use serde_json::Value;
    let data: Vec<Value> = reqwest::get(format!(
        "https://mempool.space/api/address/bc1q35yhc5khmqr6q5wxne6dud233wzefy43k4w9sv/txs/chain"
    ))
    .await
    .unwrap()
    .json()
    .await
    .unwrap();

    for row in data {
        let res: Result<Transaction, serde_json::Error> = serde_json::from_value(row.clone());
        if res.is_err() {
            let _ = dbg!(res);
            dbg!(row);
            panic!();
        }
    }
}
