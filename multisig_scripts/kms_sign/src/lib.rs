use std::fs::read_to_string;

use aws_sdk_kms as kms;
use kms::{primitives::Blob, types::SigningAlgorithmSpec};

pub fn load_dotenv() -> Option<()> {
    env_file_reader::read_file(".env")
        .ok()?
        .into_iter()
        .for_each(|(k, v)| std::env::set_var(k, v));
    Some(())
}

fn load_aws_config(file_name: &str, key_prefix: Option<&str>) -> Option<()> {
    let credentials = read_to_string(home::home_dir()?.join(format!(".aws/{file_name}"))).ok()?;
    let value: toml::Table = credentials.parse().ok()?;
    let default = value.get("default")?.as_table()?;
    default.into_iter().for_each(|(k, v)| {
        if let Some(v) = v.as_str() {
            let k = if let Some(key_prefix) = key_prefix {
                format!("{key_prefix}_{k}")
            } else {
                k.to_owned()
            }
            .to_uppercase();
            std::env::set_var(k, v);
        }
    });
    Some(())
}

pub async fn sign(digest: Vec<u8>) -> Result<Vec<u8>, kms::Error> {
    let key_arn = std::env::var("KEY_ARN").unwrap();

    let config = aws_config::load_from_env().await;
    let client = aws_sdk_kms::Client::new(&config);

    let sign = client
        .sign()
        .set_key_id(Some(key_arn))
        .set_signing_algorithm(Some(SigningAlgorithmSpec::EcdsaSha256))
        .set_message(Some(Blob::new(digest)))
        .set_message_type(Some(kms::types::MessageType::Digest))
        .send()
        .await?;
    let signature = sign.signature.unwrap().into_inner();

    let debug_big_numbers = false;
    if debug_big_numbers {
        let result: asn1::ParseResult<_> = asn1::parse(&signature, |d| {
            return d.read_element::<asn1::Sequence>()?.parse(|d| {
                let a = d.read_element::<asn1::BigInt>().unwrap();
                let b = d.read_element::<asn1::BigInt>().unwrap();
                assert!(d.is_empty());
                return Ok((a, b));
            });
        });
        let (a, b) = result.unwrap();
        eprintln!(
            "[{file}:{line}] a={:?} b={:?}",
            a,
            b,
            file = file!(),
            line = line!()
        );
    }

    Ok(signature)
}

pub fn parse_asn_pubkey<'a>(pk: &'a [u8]) -> asn1::ParseResult<&'a [u8]> {
    asn1::parse(&pk, |d| {
        return d.read_element::<asn1::Sequence>()?.parse(|d| {
            d.read_element::<asn1::Sequence>()?
                .parse(|d| {
                    d.read_element::<asn1::ObjectIdentifier>().unwrap();
                    d.read_element::<asn1::ObjectIdentifier>().unwrap();
                    asn1::ParseResult::Ok(())
                })
                .unwrap();
            let s = d.read_element::<asn1::BitString>()?;
            assert!(d.is_empty());
            return Ok(s);
        });
    })
    .map(|bit_string| bit_string.as_bytes())
}

pub async fn get_pubkey() -> Result<Vec<u8>, kms::Error> {
    let key_arn = std::env::var("KEY_ARN").unwrap();

    let config = aws_config::load_from_env().await;
    let client = aws_sdk_kms::Client::new(&config);

    let res = client
        .get_public_key()
        .set_key_id(Some(key_arn))
        .send()
        .await?;
    let pk = res.public_key.unwrap().into_inner();

    let pk = parse_asn_pubkey(&pk).unwrap().to_vec();
    Ok(pk)
}

pub fn init() {
    // discard errors
    let _ = load_aws_config("credentials", None);
    let _ = load_aws_config("config", Some("AWS"));
    let _ = load_dotenv();
}
