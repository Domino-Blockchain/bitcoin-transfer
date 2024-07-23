use std::{collections::HashSet, str::FromStr, time::Instant};

use domichain_sdk::{pubkey::Pubkey, signature::Signature};
use serde::{de, Deserialize, Deserializer};
use serde_json::{json, Value};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ConfirmationStatus {
    Processed,
    Confirmed,
    Finalized,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SignatureInfo {
    pub block_time: Option<i64>,
    pub confirmation_status: Option<ConfirmationStatus>,
    pub err: Option<serde_json::Map<String, Value>>,
    pub memo: Option<String>,
    #[serde(deserialize_with = "from_str")]
    pub signature: Signature,
    pub slot: u64,
}

#[derive(Deserialize, Debug)]
pub struct RpcResponse<T> {
    jsonrpc: String,
    result: T,
    id: usize,
}

/*
curl -s https://api.testnet.domichain.io -X POST -H "Content-Type: application/json" -d '
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getSignaturesForAddress",
    "params": [
      "4qovDeQM5kG2z9EZJQ6s93f8yak6VKrHyxWMjZva2daE",
      {
        "limit": 1000
      }
    ]
  }
' | jq '.result'

[
  {
    "blockTime": 1720614310,
    "confirmationStatus": "finalized",
    "err": null,
    "memo": null,
    "signature": "2Au9K7imhGZ3ehdKmvU3DJhKJpxr6j7fz411cR4rSiA23Axi35xAmd6AHBPpLioQL5expCwDSXorfxpRgXyhAXz6",
    "slot": 59695587
  }, ...
]


curl -s https://api.testnet.domichain.io -X POST -H "Content-Type: application/json" -d '
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getSignaturesForAddress",
    "params": [
      "4qovDeQM5kG2z9EZJQ6s93f8yak6VKrHyxWMjZva2daE",
      {
        "limit": 3,
        "before": "3qXcn6SUGiGKjPEHZQwBmYfdA9v5B9WP4q5X7ckHcVrFQ6aixdv47g3AQPocvW5BM7ky81G4zwXwP1eFcEmL1CEV",
      }
    ]
  }
' | jq '.result[] | .signature'
*/

pub async fn get_signatures_for_address(
    address: Pubkey,
) -> Result<Vec<SignatureInfo>, reqwest::Error> {
    let address = address.to_string();
    let limit = 1000;

    let client = reqwest::Client::new();
    let mut rpc_id: usize = 1;

    let mut signatures: Vec<SignatureInfo> = Vec::new();

    loop {
        let mut params = json!({ "limit": limit });
        if let Some(before_signature) = signatures.last() {
            params.as_object_mut().unwrap().insert(
                "before".to_string(),
                Value::String(before_signature.signature.to_string()),
            );
        }
        let start = Instant::now();
        let res: RpcResponse<_> = client
            .post("https://api.testnet.domichain.io")
            .json(&json!({
                "jsonrpc": "2.0",
                "id": rpc_id,
                "method": "getSignaturesForAddress",
                "params": [
                    &address,
                    params,
                ],
            }))
            .send()
            .await?
            .json()
            .await?;
        dbg!(start.elapsed(), signatures.len());
        rpc_id += 1;
        let new_page: Vec<SignatureInfo> = res.result;
        let is_last_page = new_page.len() < limit;

        signatures.extend(new_page);
        // Assert all unique
        let unique_signatures: HashSet<_> = signatures.iter().map(|sig| sig.signature).collect();
        assert_eq!(signatures.len(), unique_signatures.len());

        if is_last_page {
            break;
        }
    }

    Ok(signatures)
}

#[tokio::test]
async fn test_get_signatures_for_address() {
    dbg!(get_signatures_for_address(
        "4qovDeQM5kG2z9EZJQ6s93f8yak6VKrHyxWMjZva2daE"
            .parse()
            .unwrap()
    )
    .await
    .unwrap());
    // dbg!(
    //     get_signatures_for_address("5G1WG8CSCoWsBX8E8oPsLgRpK1n5uE6v4KQY7k4rbefM")
    //         .await
    //         .unwrap()
    // );
}

pub fn from_str<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    let s = String::deserialize(deserializer)?;
    T::from_str(&s).map_err(serde::de::Error::custom)
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
    pub err: Option<Value>,
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
    pub block_time: Option<i64>,
    pub meta: Option<DomiTransactionMeta>,
    pub slot: u64,
    pub transaction: DomiTransactionInner,
}

/*
curl https://api.testnet.domichain.io -X POST -H "Content-Type: application/json" -d '
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getTransaction",
    "params": [
      "2Au9K7imhGZ3ehdKmvU3DJhKJpxr6j7fz411cR4rSiA23Axi35xAmd6AHBPpLioQL5expCwDSXorfxpRgXyhAXz6",
      "jsonParsed"
    ]
  }
'
*/

/*
{
  "jsonrpc": "2.0",
  "result": {
    "blockTime": 1720614310,
    "meta": {
      "computeUnitsConsumed": 0,
      "err": null,
      "fee": 1000000,
      "innerInstructions": [],
      "logMessages": [
        "Program 11111111111111111111111111111111 invoke [1]",
        "Program 11111111111111111111111111111111 success"
      ],
      "postBalances": [
        0,
        9999000000,
        1
      ],
      "postTokenBalances": [],
      "preBalances": [
        10000000000,
        0,
        1
      ],
      "preTokenBalances": [],
      "rewards": [],
      "status": {
        "Ok": null
      }
    },
    "slot": 59695587,
    "transaction": {
      "message": {
        "accountKeys": [
          {
            "pubkey": "AHVhj6a5XVKKB3Es6gyWFd4ZqAS5V4LZZzoGqs182f9c",
            "signer": true,
            "source": "transaction",
            "writable": true
          },
          {
            "pubkey": "4qovDeQM5kG2z9EZJQ6s93f8yak6VKrHyxWMjZva2daE",
            "signer": false,
            "source": "transaction",
            "writable": true
          },
          {
            "pubkey": "11111111111111111111111111111111",
            "signer": false,
            "source": "transaction",
            "writable": false
          }
        ],
        "instructions": [
          {
            "parsed": {
              "info": {
                "destination": "4qovDeQM5kG2z9EZJQ6s93f8yak6VKrHyxWMjZva2daE",
                "lamports": 9999000000,
                "source": "AHVhj6a5XVKKB3Es6gyWFd4ZqAS5V4LZZzoGqs182f9c"
              },
              "type": "transfer"
            },
            "program": "system",
            "programId": "11111111111111111111111111111111",
            "stackHeight": null
          }
        ],
        "recentBlockhash": "ACGWBsKvYnCcSX6o2tNufgDMWHzVxeZoKwtfoS94EF7"
      },
      "signatures": [
        "2Au9K7imhGZ3ehdKmvU3DJhKJpxr6j7fz411cR4rSiA23Axi35xAmd6AHBPpLioQL5expCwDSXorfxpRgXyhAXz6"
      ]
    }
  },
  "id": 1
}
*/

pub async fn get_transaction(signature: Signature) -> Result<DomiTransaction, reqwest::Error> {
    let client = reqwest::Client::new();
    let res: RpcResponse<Option<_>> = client
        .post("https://api.testnet.domichain.io")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTransaction",
            "params": [
                signature.to_string(),
                "jsonParsed"
            ],
        }))
        .send()
        .await?
        .json()
        .await?;
    if res.result.is_none() {
        panic!("Not found TX signature: {signature}");
    }
    Ok(res.result.unwrap())
}

#[tokio::test]
async fn test_get_transaction() {
    dbg!(get_transaction(
        "2Au9K7imhGZ3ehdKmvU3DJhKJpxr6j7fz411cR4rSiA23Axi35xAmd6AHBPpLioQL5expCwDSXorfxpRgXyhAXz6"
            .parse()
            .unwrap()
    )
    .await
    .unwrap());

    dbg!(get_transaction(
        "2Au9K7imhGZ3ehdKmvU3DJhKJpxr6j7fz411cR4rSiA23Axi35xAmd6AHBPpLioQL5expCwDSXorfxpRgXyhAXz6"
            .parse()
            .unwrap()
    )
    .await
    .unwrap());
}
