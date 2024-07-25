mod balance_by_addresses;
mod bdk_cli;
mod bdk_cli_struct;
mod catchup;
mod db;
mod deprecated;
mod domichain;
mod estimate_fee;
mod get_address;
mod get_mint_info;
mod log_progress;
mod mempool;
mod mint_token;
mod sign_multisig_tx;
mod spl_token;
mod utils;
mod watch_addresses;
mod watch_tx;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::http::{self, HeaderValue, Method};
use axum::routing::post;
use axum::Router;
use catchup::process_catchup;
use clap::Parser;
use db::DB;
use domichain_program::pubkey::Pubkey;
use kms_sign::load_dotenv;
use reqwest::Url;
use tower_http::cors::CorsLayer;
use tracing::{debug, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use utils::ArcPathValueParser;

/// BTC Transfer service
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Domichain RPC URL
    #[arg(short = 'u', long, env = "DOMICHAIN_RPC_URL")]
    domichain_rpc_url: Url,

    /// Domichain service wallet address
    #[arg(long, env = "DOMICHAIN_SERVICE_ADDRESS")]
    domichain_service_address: Pubkey,

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

    /// Do not catchup missed BTC transactions
    #[arg(long, default_value_t = false)]
    skip_catchup: bool,

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
        domichain_rpc_url: _,
        domichain_service_address,
        mongodb_uri,
        mongodb_master_key_path,
        service_bind_address,
        service_allow_origin,
        dry_run: _,
        skip_catchup,
        spl_token_cli_path,
        spl_token_program_id,
        bdk_cli_path_default,
        bdk_cli_path_patched,
        btc_network: _,
        ledger_keys_path,
        aws_access_key_id: _,
        aws_secret_access_key: _,
        aws_region: _,
    } = &args;

    let service_allow_origin = service_allow_origin.clone();
    let service_bind_address = service_bind_address.clone();

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

    if !skip_catchup {
        debug!("starting catchup");
        process_catchup(
            db.clone(),
            *spl_token_program_id,
            *domichain_service_address,
            &all_multisig_addresses,
        )
        .await;
        debug!("catchup finished");
    } else {
        debug!("catchup skipped");
    }

    let app_state = AppState::new(db, args);

    let app = Router::new()
        .route(
            "/get_address_from_db",
            post(get_address::get_address_from_db),
        )
        .route("/estimate_fee", post(estimate_fee::estimate_fee))
        .route(
            "/sign_multisig_tx",
            post(sign_multisig_tx::sign_multisig_tx),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(service_allow_origin)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(vec![http::header::CONTENT_TYPE]),
        )
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(service_bind_address)
        .await
        .unwrap();
    info!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
