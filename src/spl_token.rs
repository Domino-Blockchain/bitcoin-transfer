use std::path::Path;
use std::process::Stdio;
use std::str::FromStr;

use domichain_sdk::pubkey::Pubkey;
use domichain_sdk::signature::Signature;
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::utils::serde_as_str;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct CombinedMintOutput {
    pub status: String,
    #[serde(with = "serde_as_str")]
    pub mint: Pubkey,
    #[serde(with = "serde_as_str")]
    pub destination_account: Pubkey,
    pub amount: u64,
    #[serde(with = "serde_as_str")]
    pub signature: Signature,
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
            "mint",
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

#[derive(Debug, Serialize, Deserialize)]
pub struct CombinedBurnOutput {
    pub status: String,
    #[serde(with = "serde_as_str")]
    pub mint: Pubkey,
    #[serde(with = "serde_as_str")]
    pub token_account: Pubkey,
    pub amount: u64,
    #[serde(with = "serde_as_str")]
    pub signature: Signature,
}

pub async fn combined_burn_cli(
    spl_token_combined_mint_cli_path: &Path,
    amount: u64,
    mint_address: Pubkey,
    token_account_address: Pubkey,
    token_program: Pubkey,
    decimals: u8,
    url: Url,
    keypair: &Path,
) -> CombinedBurnOutput {
    let output = tokio::process::Command::new(spl_token_combined_mint_cli_path)
        .args(&[
            "burn",
            "--amount",
            &amount.to_string(),
            "--token-program",
            &token_program.to_string(),
            "--decimals",
            &decimals.to_string(),
            "--url",
            &url.to_string(),
            "--keypair",
            keypair.to_str().unwrap(),
            "--mint-address",
            &mint_address.to_string(),
            "--token-account-address",
            &token_account_address.to_string(),
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

#[derive(Debug, Serialize, Deserialize)]
pub struct CombinedTransferOutput {
    pub status: String,
    #[serde(with = "serde_as_str")]
    pub mint: Pubkey,
    #[serde(with = "serde_as_str")]
    pub token_account: Pubkey,
    #[serde(with = "serde_as_str")]
    pub destination_token_account: Pubkey,
    pub amount: u64,
    #[serde(with = "serde_as_str")]
    pub signature: Signature,
}

pub async fn combined_transfer_cli(
    spl_token_combined_mint_cli_path: &Path,
    amount: u64,
    mint_address: Pubkey,
    token_account_address: Pubkey,
    destination_token_account_address: Pubkey,
    token_program: Pubkey,
    decimals: u8,
    url: Url,
    keypair: &Path,
) -> CombinedTransferOutput {
    let output = tokio::process::Command::new(spl_token_combined_mint_cli_path)
        .args(&[
            "transfer",
            "--amount",
            &amount.to_string(),
            "--token-program",
            &token_program.to_string(),
            "--decimals",
            &decimals.to_string(),
            "--url",
            &url.to_string(),
            "--keypair",
            keypair.to_str().unwrap(),
            "--mint-address",
            &mint_address.to_string(),
            "--token-account-address",
            &token_account_address.to_string(),
            "--destination-token-account-address",
            &destination_token_account_address.to_string(),
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

#[tokio::test]
async fn test_combined_mint_cli() {
    use clap::Parser;

    kms_sign::load_dotenv();
    let v: Vec<String> = vec![];
    let args = crate::Args::parse_from(v);
    assert!(args.spl_token_combined_mint_cli_path.exists());

    let amount = 1000;
    let destination_address = "6pANXPdfVhnuAax5aZD9rRbgG2qhhjJqG1Dighrd3Vrv"
        .parse()
        .unwrap();
    let decimals = 8;

    dbg!(
        combined_mint_cli(
            &args.spl_token_combined_mint_cli_path,
            amount,
            destination_address,
            args.spl_token_program_id,
            decimals,
            args.domichain_rpc_url,
            &args.domichain_service_keypair_path,
        )
        .await
    );
}
