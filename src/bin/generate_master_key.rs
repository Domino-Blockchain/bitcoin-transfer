use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use base64::prelude::*;
use clap::Parser;
use kms_sign::load_dotenv;
use rand::Rng;

/// Program to generate MongoDB master key
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to master key file for MongoDB encryption
    #[arg(long, env = "MONGODB_MASTER_KEY_PATH")]
    mongodb_master_key_path: PathBuf,
}

fn main() {
    load_dotenv();
    let Args {
        mongodb_master_key_path,
    } = Args::parse();

    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..96).map(|_| rng.gen()).collect();
    let out = BASE64_STANDARD.encode(random_bytes);

    eprintln!("Writing to: {}", mongodb_master_key_path.display());
    let mongodb_master_key_path = mongodb_master_key_path.display().to_string();
    let mongodb_master_key_path = shellexpand::tilde(&mongodb_master_key_path);
    let mongodb_master_key_path = Path::new(mongodb_master_key_path.as_ref());
    // Fail if already exists
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(mongodb_master_key_path)
        .unwrap();
    file.write_all(out.as_bytes()).unwrap();
}
