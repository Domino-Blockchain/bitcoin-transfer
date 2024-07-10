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
    use std::str::FromStr;

    use bitcoin::secp256k1::{ecdsa::Signature, Message, PublicKey};

    use super::*;

    #[tokio::test]
    async fn test_get_pubkey_aws() {
        init();
        std::env::set_var(
            "KEY_ARN",
            "arn:aws:kms:us-east-2:571922870935:key/17be5d9e-d752-4350-bbc1-68993fa25a4f",
        );
        let pk = get_pubkey_aws().await.unwrap();
        println!("{}", hex::encode(&pk));
    }

    #[tokio::test]
    async fn test_sign_aws() {
        init();
        std::env::set_var(
            "KEY_ARN",
            "arn:aws:kms:us-east-2:571922870935:key/17be5d9e-d752-4350-bbc1-68993fa25a4f",
        );
        let data: Vec<u8> = vec![0; 32];
        let hex_str = hex::encode(data);
        let signature = sign_aws(hex::decode(hex_str).unwrap()).await.unwrap();
        println!("{}", hex::encode(&signature));
    }

    #[tokio::test]
    async fn test_sign_aws_verify() {
        init();
        std::env::set_var(
            "KEY_ARN",
            "arn:aws:kms:us-east-2:571922870935:key/17be5d9e-d752-4350-bbc1-68993fa25a4f",
        );

        let pk = get_pubkey_aws().await.unwrap();
        let pk = PublicKey::from_slice(&pk).unwrap();

        let data: Vec<u8> = vec![1; 32];
        let msg = &Message::from_slice(&data[..]).unwrap();
        let hex_str = hex::encode(data);

        let mut i = 0;
        loop {
            i += 1;
            let signature = sign_aws(hex::decode(&hex_str).unwrap()).await.unwrap();
            println!("{}", hex::encode(&signature));
            let sig = Signature::from_der(&signature).unwrap();

            let secp = bitcoin::secp256k1::Secp256k1::new();
            let res = secp.verify_ecdsa(msg, &sig, &pk);
            if let Ok(()) = res {
                dbg!(msg, sig, pk.to_string());
                break;
            }
            dbg!(res.unwrap_err());
            if i > 10 {
                dbg!(msg, sig, pk.to_string());
                res.unwrap()
            }
        }
    }

    #[tokio::test]
    async fn test_sign_aws_verify_result() {
        let data: Vec<u8> = vec![1; 32];
        let msg = &Message::from_slice(&data[..]).unwrap();

        let sig = Signature::from_str("3044022013a101e5b408707fbc82b4068c97de99b1902079dd226a678c9ec01563ee02a20220196e17e05d5a0e983589553061d1c1dc4ee91a8db6f9cebe575233e0e66bcc9c").unwrap();

        let pk = PublicKey::from_str(
            "02002c5c77d7951eaa1818a7b409181b2e4a81e93e6eb44c6fe92c637c492725bb",
        )
        .unwrap();

        let secp = bitcoin::secp256k1::Secp256k1::new();
        secp.verify_ecdsa(msg, &sig, &pk).unwrap();
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

    #[tokio::test]
    async fn test_sign_google_verify() {
        init();
        std::env::set_var(
            "KEY_NAME",
            "projects/domichain-archive/locations/global/keyRings/TestKeyring/cryptoKeys/TestKey1/cryptoKeyVersions/1",
        );

        let pk = get_pubkey_google().await.unwrap();
        let pk = PublicKey::from_slice(&pk).unwrap();

        let data: Vec<u8> = vec![1; 32];
        let msg = &Message::from_slice(&data[..]).unwrap();
        let hex_str = hex::encode(data);

        let mut i = 0;
        loop {
            i += 1;
            let signature = sign_google(hex::decode(&hex_str).unwrap()).await.unwrap();
            println!("{}", hex::encode(&signature));
            let sig = Signature::from_der(&signature).unwrap();

            let secp = bitcoin::secp256k1::Secp256k1::new();
            let res = secp.verify_ecdsa(msg, &sig, &pk);
            if let Ok(()) = res {
                dbg!(msg, sig, pk.to_string());
                break;
            }
            dbg!(res.unwrap_err());
            if i > 10 {
                dbg!(msg, sig, pk.to_string());
                res.unwrap()
            }
        }
    }

    #[tokio::test]
    async fn test_sign_google_verify_result() {
        let data: Vec<u8> = vec![1; 32];
        let msg = &Message::from_slice(&data[..]).unwrap();

        let sig = Signature::from_str("304402200c10350cf7f0ff0463cd52b476e0f1e37c9d164ec0630d648973ed89d58b8b8002207980566cd7ba8a59f3d55a13d15e71bf66a83fc1b9cec38875fb7fb02a08d01f").unwrap();

        let pk = PublicKey::from_str(
            "036f0694a43f05fd642f1fe0b3bd023b1322df39080c5624a5ba8bede20fcd9dc2",
        )
        .unwrap();

        let secp = bitcoin::secp256k1::Secp256k1::new();
        secp.verify_ecdsa(msg, &sig, &pk).unwrap();
    }
}
