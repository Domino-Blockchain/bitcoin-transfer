use std::process::Stdio;
use std::str::FromStr;

pub fn spl_token(args: &[&str]) -> serde_json::Value {
    // TODO: use spl-token library to create token
    let cli_path = std::env::var("SPL_TOKEN_CLI_PATH").unwrap();
    let mut c = std::process::Command::new(&cli_path);
    let command = c
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--output")
        .arg("json")
        .args(args);
    let o = command.spawn().unwrap().wait_with_output().unwrap();
    let stdout = o.stdout;
    let stderr = o.stderr;
    // if !stdout.is_empty() {
    //     println!("stdout = {}", String::from_utf8_lossy(&stdout));
    // }
    if !stderr.is_empty() {
        eprintln!("args = {args:?}");
        let stderr = String::from_utf8_lossy(&stderr);
        eprintln!("stderr = {stderr}");
        // let stderr = stderr
        //     .trim()
        //     .strip_prefix("Error: Client(Error ")
        //     .unwrap()
        //     .strip_suffix(")")
        //     .unwrap();
        // println!("stderr = {stderr:?}");
        // let options = Options::default().with_default_extension(Extensions::EXPLICIT_STRUCT_NAMES);
        // let stderr: ron::Value = match options.from_str(&stderr) {
        //     Ok(val) => val,
        //     Err(err) => {
        //         stderr.lines().for_each(|line| {
        //             dbg!(line.get(err.position.col - 10..err.position.col + 10));
        //         });
        //         dbg!(&err);
        //         panic!("ERR: {err:?}");
        //     }
        // };
        // println!(
        //     "stderr = {}",
        //     ron::ser::to_string_pretty(&stderr, ron::ser::PrettyConfig::default()).unwrap(),
        // );
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
