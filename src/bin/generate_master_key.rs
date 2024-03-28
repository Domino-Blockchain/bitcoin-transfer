use std::{fs::OpenOptions, io::Write};

use base64::prelude::*;
use kms_sign::load_dotenv;
use rand::Rng;

fn main() {
    load_dotenv();

    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..96).map(|_| rng.gen()).collect();
    let out = BASE64_STANDARD.encode(random_bytes);

    let path = std::env::var("MONGODB_MASTER_KEY_PATH").unwrap();
    eprintln!("Writing to: {path}");
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&shellexpand::tilde(&path).as_ref())
        .unwrap();
    file.write_all(out.as_bytes()).unwrap();
}
