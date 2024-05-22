// mod spl_token_cli_lib;

mod balance_by_addresses;
mod bdk_cli;
mod bdk_cli_struct;
mod db;
mod deprecated;
mod get_address;
mod get_mint_info;
mod log_progress;
mod mempool;
mod mint_token;
mod sign_multisig_tx;
mod spl_token;
mod watch_addresses;
mod watch_tx;

use std::sync::Arc;
use std::time::Duration;

use axum::http::{self, Method};
use axum::routing::post;
use axum::Json;
use axum::Router;
use kms_sign::load_dotenv;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::time::sleep;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::db::DB;
use crate::deprecated::{
    burn_token, check_balance, check_destination_balance, get_new_service_address, send_btc_to_user,
};
use crate::get_address::get_address_from_db;
use crate::get_mint_info::get_mint_info;
use crate::mint_token::mint_token;
use crate::sign_multisig_tx::sign_multisig_tx;
use crate::spl_token::spl_token;
use crate::watch_tx::watch_tx;

#[derive(Clone)]
struct AppState {
    db: Arc<DB>,
}

impl AppState {
    fn new(db: Arc<DB>) -> Self {
        Self { db }
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
                "bitcoin_transfer=debug,tower_http=debug,axum::rejection=trace".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // let allow_origin = std::env::var("ALLOW_ORIGIN")
    //     .unwrap_or_else(|_| "http://devnet.domichain.io:3000".to_string());

    // DB::test().await.unwrap();
    let db = Arc::new(DB::new().await);
    let db_clone = Arc::clone(&db);

    let ws_handle = tokio::spawn(async move {
        let all_multisig_addresses = db_clone.get_all_multisig_addresses().await;
        dbg!(&all_multisig_addresses);
        dbg!(all_multisig_addresses.len());
        // vec!["tb1qalaejg4ve63htr8pxfr9l76cq8qqq52pgrevwy2vdqywsxlxegesh0mh6n"]

        for (_i, chunk) in all_multisig_addresses.chunks(10).enumerate() {
            let _chunk: Vec<_> = chunk.into_iter().cloned().collect();
            tokio::spawn(async move {
                // watch_addresses(i, chunk, todo!(), todo!(), todo!()).await;
            });
            sleep(Duration::from_secs(2)).await;
        }
    });

    let app = Router::new()
        .route("/get_address_from_db", post(get_address_from_db))
        .route("/watch_tx", post(watch_tx))
        .route("/get_mint_info", post(get_mint_info))
        .route("/sign_multisig_tx", post(sign_multisig_tx))
        // Deprecated
        .route(
            "/get_address",
            post(|| async { Json(get_new_service_address().await.to_string()) }),
        )
        .route("/check_balance", post(check_balance))
        .route("/mint_token", post(mint_token))
        .route("/burn_token", post(burn_token))
        .route("/send_btc_to_user", post(send_btc_to_user))
        .route(
            "/check_destination_balance",
            post(check_destination_balance),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                // .allow_origin(allow_origin.parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(vec![http::header::CONTENT_TYPE]),
        )
        .with_state(AppState::new(db.into()));

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
