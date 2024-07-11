use std::process::Stdio;
use std::str::FromStr;

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
