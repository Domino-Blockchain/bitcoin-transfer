use aws_sdk_kms as kms;
use clap::{arg, command, Command};
use kms_sign::{get_pubkey_aws, get_pubkey_google, init, sign_aws, sign_google};

#[tokio::main]
async fn main() -> Result<(), kms::Error> {
    let matches = command!()
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("get_pubkey").about("Get KMS pubkey"))
        .subcommand(
            Command::new("sign")
                .about("Sign with KMS key")
                .arg(arg!(<BASE64_STR>)),
        )
        .subcommand(Command::new("get_pubkey_google").about("Get KMS pubkey"))
        .subcommand(
            Command::new("sign_google")
                .about("Sign with KMS key")
                .arg(arg!(<BASE64_STR>)),
        )
        .get_matches();

    init();

    match matches.subcommand() {
        Some(("get_pubkey_aws", _sub_matches)) => {
            let pk = get_pubkey_aws().await.unwrap();
            println!("{}", hex::encode(&pk));
        }
        Some(("sign_aws", sub_matches)) => {
            let hex_str = sub_matches.get_one::<String>("BASE64_STR").unwrap();
            let signature = sign_aws(hex::decode(hex_str).unwrap()).await.unwrap();
            println!("{}", hex::encode(&signature));
        }
        Some(("get_pubkey_google", _sub_matches)) => {
            let pk = get_pubkey_google().await.unwrap();
            println!("{}", hex::encode(&pk));
        }
        Some(("sign_google", sub_matches)) => {
            let hex_str = sub_matches.get_one::<String>("BASE64_STR").unwrap();
            let signature = sign_google(hex::decode(hex_str).unwrap()).await.unwrap();
            println!("{}", hex::encode(&signature));
        }
        _ => unreachable!("Exhausted list of subcommands and subcommand_required prevents `None`"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_pubkey_aws() {
        init();
        let pk = get_pubkey_aws().await.unwrap();
        println!("{}", hex::encode(&pk));
    }

    #[tokio::test]
    async fn test_sign_aws() {
        init();
        let data: Vec<u8> = vec![0; 32];
        let hex_str = hex::encode(data);
        let signature = sign_aws(hex::decode(hex_str).unwrap()).await.unwrap();
        println!("{}", hex::encode(&signature));
    }

    #[tokio::test]
    async fn test_get_pubkey_google() {
        init();
        std::env::set_var("KEY_NAME", "projects/domichain-archive/locations/global/keyRings/TestKeyring/cryptoKeys/TestKey1/cryptoKeyVersions/1");
        let pk = get_pubkey_google().await.unwrap();
        println!("{}", hex::encode(&pk));
    }

    #[tokio::test]
    async fn test_sign_google() {
        init();
        std::env::set_var("KEY_NAME", "projects/domichain-archive/locations/global/keyRings/TestKeyring/cryptoKeys/TestKey1/cryptoKeyVersions/1");
        let data: Vec<u8> = vec![0; 32];
        let hex_str = hex::encode(data);
        let signature = sign_google(hex::decode(hex_str).unwrap()).await.unwrap();
        println!("{}", hex::encode(&signature));
    }
}