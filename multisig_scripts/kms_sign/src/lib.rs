use std::ffi::OsStr;
use std::fs::read_to_string;

use aws_sdk_kms as kms;
use base64::prelude::*;
use gcloud_sdk::google::cloud::kms::v1::key_management_service_client::KeyManagementServiceClient;
use gcloud_sdk::google::cloud::kms::v1::{AsymmetricSignRequest, GetPublicKeyRequest};
use gcloud_sdk::{GoogleApi, GoogleAuthMiddleware};
use kms::{primitives::Blob, types::SigningAlgorithmSpec};

pub fn load_dotenv() -> Option<()> {
    env_file_reader::read_file(".env")
        .ok()?
        .into_iter()
        .for_each(|(k, v)| std::env::set_var(k, v));
    Some(())
}

fn load_aws_config(file_name: &str, key_prefix: Option<&str>) -> Option<()> {
    let credentials_path = home::home_dir()?.join(".aws").join(file_name);
    let credentials = read_to_string(credentials_path).ok()?;

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

fn get_env<K: AsRef<OsStr> + std::fmt::Debug>(key: K) -> String {
    std::env::var(&key)
        .map_err(|err| (err, key))
        .expect("Env var is not present")
}

pub async fn sign(digest: Vec<u8>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if std::env::var("KEY_ARN").is_ok() {
        sign_aws(digest).await.map_err(|e| e.into())
    } else {
        sign_google(digest).await
    }
}

fn debug_sign_big_numbers(signature: &[u8]) {
    let result: asn1::ParseResult<_> = asn1::parse(signature, |d| {
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

pub async fn sign_aws(digest: Vec<u8>) -> Result<Vec<u8>, kms::Error> {
    let key_arn = get_env("KEY_ARN");

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
        debug_sign_big_numbers(&signature);
    }

    Ok(signature)
}

pub fn parse_asn_pubkey<'a>(pk: &'a [u8]) -> asn1::ParseResult<&'a [u8]> {
    asn1::parse(&pk, |d| {
        return d.read_element::<asn1::Sequence>()?.parse(|d| {
            d.read_element::<asn1::Sequence>()?
                .parse(|d| {
                    let _obj_id = d.read_element::<asn1::ObjectIdentifier>().unwrap();
                    let _obj_id = d.read_element::<asn1::ObjectIdentifier>().unwrap();
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

pub async fn get_pubkey() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if std::env::var("KEY_ARN").is_ok() {
        get_pubkey_aws().await.map_err(|e| e.into())
    } else {
        get_pubkey_google().await
    }
}

pub async fn get_pubkey_aws() -> Result<Vec<u8>, kms::Error> {
    let key_arn = get_env("KEY_ARN");

    let config = aws_config::load_from_env().await;
    let client = aws_sdk_kms::Client::new(&config);

    let res = client
        .get_public_key()
        .set_key_id(Some(key_arn))
        .send()
        .await?;
    let pk_pem = res.public_key.unwrap().into_inner();

    let pk = parse_asn_pubkey(&pk_pem).unwrap().to_vec();
    Ok(pk)
}

pub async fn google_kms_client(
) -> Result<GoogleApi<KeyManagementServiceClient<GoogleAuthMiddleware>>, Box<dyn std::error::Error>>
{
    let kms_client: GoogleApi<KeyManagementServiceClient<GoogleAuthMiddleware>> =
        GoogleApi::from_function(
            KeyManagementServiceClient::new,
            "https://cloudkms.googleapis.com",
            // cloud resource prefix: used only for some of the APIs (such as Firestore)
            None,
        )
        .await?;
    Ok(kms_client)
}

// Source: https://github.com/abdolence/gcloud-sdk-rs/blob/master/examples/secrets-manager-client/src/main.rs
pub async fn sign_google(digest: Vec<u8>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let key_name = get_env("KEY_NAME");
    let kms_client = google_kms_client().await?;

    let response = kms_client
        .get()
        .asymmetric_sign(AsymmetricSignRequest {
            name: key_name,
            data: digest,
            ..Default::default()
        })
        .await?;
    let signature = response.into_inner().signature;

    let debug_big_numbers = false;
    if debug_big_numbers {
        debug_sign_big_numbers(&signature);
    }

    // println!("Response: {:?}", response);
    Ok(signature)
}

pub async fn get_pubkey_google() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let key_name = get_env("KEY_NAME");

    let kms_client = google_kms_client().await?;

    let response = kms_client
        .get()
        .get_public_key(GetPublicKeyRequest { name: key_name })
        .await?;

    let pk_base64 = response.into_inner().pem;
    let pk_base64 = pk_base64
        .strip_prefix("-----BEGIN PUBLIC KEY-----\n")
        .unwrap()
        .strip_suffix("\n-----END PUBLIC KEY-----\n")
        .unwrap()
        .replace('\n', "");
    let pk_pem = BASE64_STANDARD.decode(&pk_base64).unwrap();
    let pk = parse_asn_pubkey(&pk_pem).unwrap().to_vec();

    Ok(pk)
}

pub fn init() {
    // discard errors
    let _ = load_aws_config("credentials", None);
    let _ = load_aws_config("config", Some("AWS"));
    let _ = load_dotenv();
}
