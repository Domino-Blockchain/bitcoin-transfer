use std::{process::Stdio, str::FromStr};

use tokio::{fs::remove_dir_all, process::Command, sync::Semaphore};

pub async fn bdk_cli_inner(args: &[&str], cli_path: &str) -> serde_json::Value {
    let mut c = Command::new(cli_path);
    let command = c.stdout(Stdio::piped()).stderr(Stdio::piped()).args(args);
    let o = command.spawn().unwrap().wait_with_output().await.unwrap();
    if !o.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&o.stderr);
        dbg!(stderr);
    }
    serde_json::Value::from_str(std::str::from_utf8(&o.stdout).unwrap()).unwrap()
}

pub async fn bdk_cli(args: &[&str]) -> serde_json::Value {
    let cli_path = std::env::var("BDK_CLI_PATH_DEFAULT").unwrap();
    bdk_cli_inner(args, &cli_path).await
}

pub async fn bdk_cli_wallet_inner(
    descriptor: &str,
    args: &[&str],
    cli_path: &str,
) -> serde_json::Value {
    let mut cli_args = vec![
        "wallet",
        "--wallet",
        "wallet_name_temp",
        "--descriptor",
        descriptor,
    ];
    cli_args.extend_from_slice(args);

    bdk_cli_inner(&cli_args, cli_path).await
}

pub static WALLET_DIR_PERMIT: Semaphore = Semaphore::const_new(1);

pub async fn bdk_cli_wallet_temp_inner(
    descriptor: &str,
    args: &[&str],
    cli_path: &str,
) -> serde_json::Value {
    let wallet_dir_permit = WALLET_DIR_PERMIT.acquire().await.unwrap();
    let _ = remove_dir_all("/home/domi/.bdk-bitcoin/wallet_name_temp").await;
    let result = bdk_cli_wallet_inner(descriptor, args, cli_path).await;
    let _ = remove_dir_all("/home/domi/.bdk-bitcoin/wallet_name_temp").await;
    drop(wallet_dir_permit);
    result
}

pub async fn bdk_cli_wallet(multi_descriptor: &str, args: &[&str]) -> serde_json::Value {
    let cli_path = std::env::var("BDK_CLI_PATH_DEFAULT").unwrap();
    bdk_cli_wallet_temp_inner(multi_descriptor, args, &cli_path).await
}

pub async fn bdk_cli_wallet_temp(multi_descriptor: &str, args: &[&str]) -> serde_json::Value {
    let cli_path = std::env::var("BDK_CLI_PATH_DEFAULT").unwrap();
    bdk_cli_wallet_inner(multi_descriptor, args, &cli_path).await
}

pub async fn bdk_cli_wallet_patched(multi_descriptor: &str, args: &[&str]) -> serde_json::Value {
    let cli_path = std::env::var("BDK_CLI_PATH_PATCHED").unwrap();
    bdk_cli_wallet_temp_inner(multi_descriptor, args, &cli_path).await
}
