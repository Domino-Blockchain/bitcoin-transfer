use std::{
    ffi::OsStr,
    process::{Output, Stdio},
    str::FromStr,
};

use tokio::{fs::remove_dir_all, process::Command, sync::Semaphore};
use tracing::error;

pub async fn exec_with_json_output(
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    program_path: impl AsRef<OsStr>,
) -> serde_json::Value {
    let args: Vec<_> = args.into_iter().map(|a| a.as_ref().to_owned()).collect();
    // dbg!(&args);
    println!();
    args.iter()
        .for_each(|arg| println!("\t{}", arg.to_str().unwrap()));
    println!();
    let output = Command::new(program_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(args)
        .spawn()
        .unwrap()
        .wait_with_output()
        .await
        .unwrap();
    if !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("exec_with_json_output: {}", stderr);
    }
    serde_json::Value::from_str(std::str::from_utf8(&output.stdout).unwrap()).unwrap()
}

pub async fn try_exec_with_json_output(
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    program_path: impl AsRef<OsStr>,
) -> Result<serde_json::Value, Output> {
    let args: Vec<_> = args.into_iter().map(|a| a.as_ref().to_owned()).collect();
    // dbg!(&args);
    println!();
    args.iter()
        .for_each(|arg| println!("\t{}", arg.to_str().unwrap()));
    println!();
    let output = Command::new(program_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(args)
        .spawn()
        .unwrap()
        .wait_with_output()
        .await
        .unwrap();
    if !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("try_exec_with_json_output: {}", stderr);
    }
    let result = serde_json::Value::from_str(std::str::from_utf8(&output.stdout).unwrap());
    result.map_err(|_| output)
}

pub async fn bdk_cli_inner<S: AsRef<OsStr>>(args: &[&str], cli_path: S) -> serde_json::Value {
    exec_with_json_output(args, cli_path).await
}

pub async fn bdk_cli(args: &[&str]) -> serde_json::Value {
    let cli_path = std::env::var("BDK_CLI_PATH_DEFAULT").unwrap();
    bdk_cli_inner(args, &cli_path).await
}

pub async fn bdk_cli_wallet_inner<S: AsRef<OsStr>>(
    descriptor: &str,
    args: &[&str],
    cli_path: S,
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

pub async fn bdk_cli_wallet_temp_inner<S: AsRef<OsStr>>(
    descriptor: &str,
    args: &[&str],
    cli_path: S,
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
