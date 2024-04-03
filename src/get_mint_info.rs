use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;

#[derive(Deserialize)]
pub struct GetMintInfoRequest {
    btc_deposit_address: String,
}

pub async fn get_mint_info(
    State(state): State<AppState>,
    Json(request): Json<GetMintInfoRequest>,
) -> Json<serde_json::Value> {
    let btc_deposit_address = request.btc_deposit_address;

    let meta = state
        .db
        .find_by_deposit_address(&btc_deposit_address)
        .await
        .unwrap();
    let meta = match meta {
        None => {
            return Json(json!({
                "status": "error",
                "error": format!("Deposit address not found: {btc_deposit_address}"),
            }));
        }
        Some(meta) => meta,
    };
    Json(match meta.get("minted").map(|b| b.as_bool().unwrap()) {
        Some(true) => {
            json!({
                "status": "ok",
                "mint_address": meta.get("mint_address").unwrap(),
            })
        }
        Some(false) | None => {
            json!({
                "status": "error",
                "error": format!("Token is not minted: {btc_deposit_address}"),
            })
        }
    })
}
