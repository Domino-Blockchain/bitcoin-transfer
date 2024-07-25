use std::{str::FromStr, time::Duration};

use domichain_sdk::{pubkey::Pubkey, signature::Signature};
use reqwest::Url;
use serde::{de, Deserialize};
use serde_json::{json, Value};
use tokio::time::sleep;

use crate::utils::from_str;

#[allow(dead_code)]
#[derive(Deserialize)]
struct GetBlockHeightResp {
    jsonrpc: String,
    result: u64,
    id: usize,
}

/// See: https://solana.com/docs/rpc/http/getblockheight
pub async fn get_block_height(rpc_url: Url) -> u64 {
    let client = reqwest::Client::new();
    let res = client
        .post(rpc_url)
        .json(&json!({
          "jsonrpc": "2.0",
          "id": 1,
          "method": "getBlockHeight",
        }))
        .send()
        .await
        .unwrap();
    let res: GetBlockHeightResp = res.json().await.unwrap();
    res.result
}

#[derive(Debug)]
pub struct JsonSignature(pub Signature);

impl<'de> Deserialize<'de> for JsonSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Signature::from_str(&s).map(Self).map_err(de::Error::custom)
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DomiTransactionMeta {
    pub err: Value,
    pub status: Value,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DomiTransactionInstructionInfo {
    #[serde(deserialize_with = "from_str")]
    pub authority: Pubkey,
    #[serde(deserialize_with = "from_str")]
    pub destination: Pubkey,
    #[serde(deserialize_with = "from_str")]
    pub mint: Pubkey,
    #[serde(deserialize_with = "from_str")]
    pub source: Pubkey,
    pub token_amount: Value,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DomiTransactionInstructionParsed {
    pub info: Value,
    #[serde(rename = "type")]
    pub instruction_type: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DomiTransactionInstruction {
    pub parsed: DomiTransactionInstructionParsed,
    pub program: String,
    #[serde(deserialize_with = "from_str")]
    pub program_id: Pubkey,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DomiTransactionMessage {
    pub instructions: Vec<DomiTransactionInstruction>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DomiTransactionInner {
    pub message: DomiTransactionMessage,
    pub signatures: Vec<JsonSignature>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DomiTransaction {
    pub block_time: u64,
    pub meta: DomiTransactionMeta,
    pub slot: u64,
    pub transaction: DomiTransactionInner,
}

#[derive(Deserialize, Debug)]
pub struct GetTransactionResponse {
    pub jsonrpc: String,
    pub result: DomiTransaction,
    pub id: usize,
}

/// See: https://solana.com/docs/rpc/http/gettransaction
pub async fn get_transaction(
    rpc_url: Url,
    signature: Signature,
) -> Result<DomiTransaction, reqwest::Error> {
    let client = reqwest::Client::new();
    let res = client
        .post(rpc_url)
        .json(&json!({
          "jsonrpc": "2.0",
          "id": 1,
          "method": "getTransaction",
          "params": [
            signature.to_string(),
            "jsonParsed"
          ]
        }))
        .send()
        .await?;
    let res: GetTransactionResponse = res.json().await?;
    Ok(res.result)
}

pub async fn get_transaction_poll(rpc_url: Url, signature: Signature) -> DomiTransaction {
    let mut duration = Duration::from_millis(500);
    let mut attempts = 8;
    loop {
        if let Ok(tx) = get_transaction(rpc_url.clone(), signature).await {
            return tx;
        }
        sleep(duration).await;
        duration = duration
            .checked_mul(2)
            .unwrap()
            .min(Duration::from_secs(10));
        attempts -= 1;
        if attempts == 0 {
            return get_transaction(rpc_url.clone(), signature).await.unwrap();
        }
    }
}

#[tokio::test]
async fn test_get_transaction() {
    let tx = get_transaction_poll(
        Url::from_str("https://api.testnet.domichain.io").unwrap(),
        Signature::from_str("45LT1XzdoHuNU7v1jXsi2xk5oE5Wgqb8aZpq71iTPqnFWMVsdehHTW56x2PyJJ5BPfGTTAmeeMdY34nazdbRkKsm").unwrap(),
    ).await;
    dbg!(tx);
}
