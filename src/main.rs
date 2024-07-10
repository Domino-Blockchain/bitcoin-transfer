mod balance_by_addresses;
mod bdk_cli;
mod bdk_cli_struct;
mod db;
mod deprecated;
mod estimate_fee;
mod get_address;
mod get_mint_info;
mod log_progress;
mod mempool;
mod mint_token;
mod sign_multisig_tx;
mod spl_token;
mod watch_addresses;
mod watch_tx;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::http::{self, HeaderValue, Method};
use axum::routing::post;
use axum::Json;
use axum::Router;
use clap::Parser;
use domichain_program::pubkey::Pubkey;
use kms_sign::load_dotenv;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::time::sleep;
use tower_http::cors::CorsLayer;
use tracing::{debug, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::db::DB;
use crate::deprecated::{
    burn_token, check_balance, check_destination_balance, get_new_service_address, send_btc_to_user,
};
use crate::estimate_fee::estimate_fee;
use crate::get_address::get_address_from_db;
use crate::get_mint_info::get_mint_info;
use crate::mint_token::mint_token;
use crate::sign_multisig_tx::sign_multisig_tx;
use crate::spl_token::spl_token;
use crate::watch_tx::watch_tx;

#[derive(Clone)]
struct ArcPathValueParser;

impl clap::builder::TypedValueParser for ArcPathValueParser {
    type Value = Arc<Path>;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let path = PathBuf::from(value);
        Ok(path.into())
    }
}

/// BTC Transfer service
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Domichain RPC URL
    #[arg(short = 'u', long, env = "DOMICHAIN_RPC_URL")]
    domichain_rpc_url: Url,

    /// MongoDB URI
    #[arg(long, env = "MONGODB_URI")]
    mongodb_uri: String,

    /// Path to master key file for MongoDB encryption
    #[arg(long, env = "MONGODB_MASTER_KEY_PATH", value_parser=ArcPathValueParser)]
    mongodb_master_key_path: Arc<Path>,

    /// Start service bind to the address
    #[arg(long, env = "SERVICE_BIND_ADDRESS")]
    service_bind_address: SocketAddr,

    /// Configure HTTP server allow origin header for CORS
    #[arg(long, env = "SERVICE_ALLOW_ORIGIN")]
    service_allow_origin: HeaderValue,

    /// Dry run, don't send BTC TX
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// Path to spl-token-cli
    #[arg(long, env = "SPL_TOKEN_CLI_PATH", value_parser=ArcPathValueParser)]
    spl_token_cli_path: Arc<Path>,

    /// Domichain program ID of SPL token
    #[arg(long, env = "SPL_TOKEN_PROGRAM_ID")]
    spl_token_program_id: Pubkey,

    /// Path to bdk-cli
    #[arg(long, env = "BDK_CLI_PATH_DEFAULT", value_parser=ArcPathValueParser)]
    bdk_cli_path_default: Arc<Path>,

    /// Path to bdk-cli with AWS KMS support
    #[arg(long, env = "BDK_CLI_PATH_PATCHED", value_parser=ArcPathValueParser)]
    bdk_cli_path_patched: Arc<Path>,

    /// Bitcoin network
    #[arg(long, env = "BTC_NETWORK")]
    btc_network: bdk::bitcoin::Network,

    /// Path to ledger keys JSON file with hardware ledger pubkey
    #[arg(long, env = "LEDGER_KEYS_PATH", value_parser=ArcPathValueParser)]
    ledger_keys_path: Arc<Path>,

    /// AWS Access key ID
    #[arg(long, env = "AWS_ACCESS_KEY_ID")]
    aws_access_key_id: String, // TODO: Arc<str> or remove from AppState

    /// AWS Secret access key
    #[arg(long, env = "AWS_SECRET_ACCESS_KEY")]
    aws_secret_access_key: String,

    /// AWS Region
    #[arg(long, env = "AWS_REGION")]
    aws_region: String,
}

#[derive(Clone)]
struct AppState {
    db: Arc<DB>,
    config: Args,
}

impl AppState {
    fn new(db: Arc<DB>, config: Args) -> Self {
        Self { db, config }
    }
}

#[tokio::main]
async fn main() {
    load_dotenv();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // axum logs rejections from built-in extractors with the `axum::rejection`
                // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
                concat!(
                    env!("CARGO_PKG_NAME"),
                    "=debug",
                    ",tower_http=debug",
                    ",axum::rejection=trace"
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    let Args {
        domichain_rpc_url,
        mongodb_uri,
        mongodb_master_key_path,
        service_bind_address,
        service_allow_origin,
        dry_run,
        spl_token_cli_path,
        spl_token_program_id,
        bdk_cli_path_default,
        bdk_cli_path_patched,
        btc_network,
        ledger_keys_path,
        aws_access_key_id,
        aws_secret_access_key,
        aws_region,
    } = &args;

    let service_allow_origin = service_allow_origin.clone();

    assert!(mongodb_master_key_path.exists());
    assert!(spl_token_cli_path.exists());
    assert!(bdk_cli_path_default.exists());
    assert!(bdk_cli_path_patched.exists());
    assert!(ledger_keys_path.exists());

    debug!("starting");

    let db = Arc::new(DB::new(mongodb_uri, mongodb_master_key_path).await);

    let all_multisig_addresses = db.get_all_multisig_addresses().await;
    info!("all_multisig_addresses = {:#?}", &all_multisig_addresses);
    info!(
        "all_multisig_addresses.len() = {:#?}",
        all_multisig_addresses.len()
    );

    let db_clone = Arc::clone(&db);
    let ws_handle = tokio::spawn(async move {
        for (_i, chunk) in all_multisig_addresses.chunks(10).enumerate() {
            let _chunk: Vec<_> = chunk.into_iter().cloned().collect();
            tokio::spawn(async move {
                // watch_addresses(i, chunk, todo!(), todo!(), todo!()).await;
            });
            sleep(Duration::from_secs(2)).await;
        }
    });

    let app_state = AppState::new(db.into(), args);

    let app = Router::new()
        .route("/get_address_from_db", post(get_address_from_db))
        .route("/estimate_fee", post(estimate_fee))
        .route("/sign_multisig_tx", post(sign_multisig_tx))
        // // Unused
        // .route("/watch_tx", post(watch_tx))
        // .route("/get_mint_info", post(get_mint_info))
        // // Deprecated
        // .route(
        //     "/get_address",
        //     post(|| async { Json(get_new_service_address().await.to_string()) }),
        // )
        // .route("/check_balance", post(check_balance))
        // .route("/mint_token", post(mint_token))
        // .route("/burn_token", post(burn_token))
        // .route("/send_btc_to_user", post(send_btc_to_user))
        // .route(
        //     "/check_destination_balance",
        //     post(check_destination_balance),
        // )
        .layer(
            CorsLayer::new()
                .allow_origin(service_allow_origin)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(vec![http::header::CONTENT_TYPE]),
        )
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await.unwrap();
    info!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    ws_handle.await.unwrap();
}

pub fn serde_convert<F, T>(a: F) -> T
where
    F: Serialize,
    T: DeserializeOwned,
{
    let string = serde_json::to_string(&a).unwrap();
    serde_json::from_str(&string).unwrap()
}
