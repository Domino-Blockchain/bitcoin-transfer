use std::{process::Stdio, str::FromStr};

use tokio::{fs::remove_dir_all, process::Command, sync::Semaphore};

pub async fn bdk_cli(args: &[&str]) -> serde_json::Value {
    let cli_path = std::env::var("BDK_CLI_PATH").unwrap();
    let mut c = Command::new(&cli_path);
    let command = c.stdout(Stdio::piped()).stderr(Stdio::piped()).args(args);
    let o = command.spawn().unwrap().wait_with_output().await.unwrap();
    if !o.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&o.stderr);
        dbg!(stderr);
    }
    serde_json::Value::from_str(std::str::from_utf8(&o.stdout).unwrap()).unwrap()
}

static WALLET_DIR_PERMIT: Semaphore = Semaphore::const_new(1);

pub async fn bdk_cli_wallet(multi_descriptor: &str, args: &[&str]) -> serde_json::Value {
    let mut cli_args = vec![
        "wallet",
        "--wallet",
        "wallet_name_temp",
        "--descriptor",
        multi_descriptor,
    ];
    cli_args.extend_from_slice(args);

    let wallet_dir_permit = WALLET_DIR_PERMIT.acquire().await.unwrap();

    let _ = remove_dir_all("/home/domi/.bdk-bitcoin/wallet_name_temp").await;
    let result = bdk_cli(&cli_args).await;
    let _ = remove_dir_all("/home/domi/.bdk-bitcoin/wallet_name_temp").await;

    drop(wallet_dir_permit);

    result
}
