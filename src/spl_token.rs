use std::path::Path;
use std::process::Stdio;
use std::str::FromStr;

use domichain_sdk::pubkey::Pubkey;
use domichain_sdk::signature::Signature;
use reqwest::Url;
use serde::Deserialize;

use crate::utils::from_str;

pub fn spl_token(args: &[&str]) -> serde_json::Value {
    // TODO: use spl-token library to create token
    let cli_path = std::env::var("SPL_TOKEN_CLI_PATH").unwrap();
    let domichain_rpc_url = std::env::var("DOMICHAIN_RPC_URL").unwrap();
    let spl_token_program_id = std::env::var("SPL_TOKEN_PROGRAM_ID").unwrap();

    let mut full_args = vec![
        "--output",
        "json",
        "--url",
        &domichain_rpc_url,
        "--program-id",
        &spl_token_program_id,
    ];
    full_args.extend_from_slice(args);

    let mut c = std::process::Command::new(&cli_path);
    let command = c
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(&full_args);
    let o = command.spawn().unwrap().wait_with_output().unwrap();
    if !o.status.success() {
        eprintln!("exec: {cli_path}");
        for arg in &full_args {
            eprintln!("    {arg}");
        }

        eprintln!("status = {}", &o.status);
    }
    let stdout = o.stdout;
    let stderr = o.stderr;
    if !stdout.is_empty() {
        eprintln!("stdout = {}", String::from_utf8_lossy(&stdout));
    }
    if !stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&stderr);
        eprintln!("stderr = {stderr}");
    }
    serde_json::Value::from_str(std::str::from_utf8(&stdout).unwrap()).unwrap()
}

#[allow(dead_code)]
pub fn spl_token_plain(args: &[&str]) {
    // TODO: use spl-token library to create token
    let cli_path = std::env::var("SPL_TOKEN_CLI_PATH").unwrap();
    let mut c = std::process::Command::new(cli_path);
    let command = c.stdout(Stdio::piped()).stderr(Stdio::piped()).args(args);
    let o = command.spawn().unwrap().wait_with_output().unwrap();
    let stdout = o.stdout;
    let stderr = o.stderr;
    if !stdout.is_empty() {
        println!("stdout = {}", String::from_utf8_lossy(&stdout));
    }
    if !stderr.is_empty() {
        println!("stderr = {}", String::from_utf8_lossy(&stderr));
    }
}

#[derive(Deserialize)]
pub struct CombinedMintOutput {
    status: String,
    #[serde(deserialize_with = "from_str")]
    mint: Pubkey,
    #[serde(deserialize_with = "from_str")]
    destination_account: Pubkey,
    amount: u64,
    #[serde(deserialize_with = "from_str")]
    signature: Signature,
}

pub async fn combined_mint_cli(
    spl_token_combined_mint_cli_path: &Path,
    amount: u64,
    destination_address: Pubkey,
    token_program: Pubkey,
    decimals: u8,
    url: Url,
    keypair: &Path,
) -> CombinedMintOutput {
    let output = tokio::process::Command::new(spl_token_combined_mint_cli_path)
        .args(&[
            "--amount",
            &amount.to_string(),
            "--destination-address",
            &destination_address.to_string(),
            "--token-program",
            &token_program.to_string(),
            "--decimals",
            &decimals.to_string(),
            "--url",
            &url.to_string(),
            "--keypair",
            keypair.to_str().unwrap(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
        .wait_with_output()
        .await
        .unwrap();
    serde_json::from_slice(&output.stdout).unwrap()
}
