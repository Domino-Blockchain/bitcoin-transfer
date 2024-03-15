use aws_sdk_kms as kms;
use base64::prelude::*;
use crypto_hash::{digest, hex_digest, Algorithm};
use kms::{primitives::Blob, types::SigningAlgorithmSpec};

const KEY_ARN: &str = "arn:aws:kms:us-east-2:571922870935:key/17be5d9e-d752-4350-bbc1-68993fa25a4f";

// export $(cat .env | xargs)
#[tokio::main]
async fn main() -> Result<(), kms::Error> {
    let data = BASE64_STANDARD
        .decode(std::env::args().nth(1).unwrap().as_bytes())
        .unwrap();
    let digest = digest(Algorithm::SHA256, &data);
    dbg!(hex_digest(Algorithm::SHA256, &data));

    let config = aws_config::load_from_env().await;
    let client = aws_sdk_kms::Client::new(&config);

    let sign = client
        .sign()
        .set_key_id(Some(KEY_ARN.to_string()))
        .set_signing_algorithm(Some(SigningAlgorithmSpec::EcdsaSha256))
        .set_message(Some(Blob::new(digest)))
        .set_message_type(Some(kms::types::MessageType::Digest))
        .send()
        .await?;
    println!("SIGNATURE: {:?}", sign.signature);

    Ok(())
}
