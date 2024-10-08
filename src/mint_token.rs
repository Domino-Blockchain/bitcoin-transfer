use std::str::FromStr;

use axum::{extract::State, Json};
use domichain_account_decoder::parse_token::token_amount_to_ui_amount;
use domichain_program::pubkey::Pubkey;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    spl_token::{combined_burn_cli, combined_mint_cli, combined_transfer_cli, spl_token},
    AppState, Args,
};

/// Get service token account address
pub fn get_account_address(token_address: Pubkey) -> Pubkey {
    // DWallet: 5PCWRXtMhen9ipbq4QeeAuDgFymGachUf7ozA3NJwHDJ

    // let token_program_id = Pubkey::from_str("TokenAAGbeQq5tGW2r5RoR3oauzN2EkNFiHNPw9q34s").unwrap();
    let token_program_id_string = std::env::var("SPL_TOKEN_PROGRAM_ID").unwrap();
    let token_program_id = Pubkey::from_str(&token_program_id_string).unwrap();

    let associated_token_program_id =
        Pubkey::from_str("Dt8fRCpjeV6JDemhPmtcTKijgKdPxXHn9Wo9cXY5agtG").unwrap();
    // owner == Fk2HRYuDw9h29yKs1tNDjvjdvYMqQ2dGg9sS4JhUzQ6w
    let owner =
        Pubkey::from_str(spl_token(&["address"])["walletAddress"].as_str().unwrap()).unwrap();
    let mint = token_address;
    /*
    const TOKEN_PROGRAM_ID = new PublicKey('TokenAAGbeQq5tGW2r5RoR3oauzN2EkNFiHNPw9q34s');
    const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('Dt8fRCpjeV6JDemhPmtcTKijgKdPxXHn9Wo9cXY5agtG');
    const owner = new PublicKey('Fk2HRYuDw9h29yKs1tNDjvjdvYMqQ2dGg9sS4JhUzQ6w');
    const mint = new PublicKey('9bLgyijGKGrKHfT72JUQZNgEH4GVTuHosk3VcdpHYo19');

    const [address] = await PublicKey.findProgramAddress(
        [owner.toBuffer(), TOKEN_PROGRAM_ID.toBuffer(), mint.toBuffer()],
        ASSOCIATED_TOKEN_PROGRAM_ID
    );
    */

    let (pubkey, _bump_seed) = Pubkey::find_program_address(
        &[owner.as_ref(), token_program_id.as_ref(), mint.as_ref()],
        &associated_token_program_id,
    );
    pubkey
}

/// Get user token account address
pub fn get_user_account_address(token_address: Pubkey, user: Pubkey) -> Pubkey {
    let token_program_id_string = std::env::var("SPL_TOKEN_PROGRAM_ID").unwrap();
    let token_program_id = Pubkey::from_str(&token_program_id_string).unwrap();

    let associated_token_program_id =
        Pubkey::from_str("Dt8fRCpjeV6JDemhPmtcTKijgKdPxXHn9Wo9cXY5agtG").unwrap();

    let (pubkey, _bump_seed) = Pubkey::find_program_address(
        &[
            user.as_ref(),
            token_program_id.as_ref(),
            token_address.as_ref(),
        ],
        &associated_token_program_id,
    );
    pubkey
}

#[test]
fn test_get_account_address() {
    std::env::set_var(
        "SPL_TOKEN_PROGRAM_ID",
        "BTCi9FUjBVY3BSaqjzfhEPKVExuvarj8Gtfn4rJ5soLC",
    );
    std::env::set_var(
        "SPL_TOKEN_CLI_PATH",
        "/home/btc-transfer/spl-token_from_domi",
    );
    std::env::set_var("DOMICHAIN_RPC_URL", "https://api.testnet.domichain.io/");
    dbg!(get_account_address(
        "Dm6phGa5eh7ihFtvbqM2cjxYrpvvzg5h5y3CnrXHEb2x"
            .parse()
            .unwrap()
    ));
}

#[derive(Deserialize)]
pub struct MintTokenRequest {
    pub amount: String,
    pub address: String,
}

#[allow(dead_code)]
pub async fn mint_token(
    State(state): State<AppState>,
    Json(request): Json<MintTokenRequest>,
) -> Json<MintTokenResult> {
    let MintTokenRequest { amount, address } = request;
    Json(
        mint_token_inner(&state.config, &amount, &address)
            .await
            .unwrap(),
    )
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MintTokenResult {
    pub mint_address: String,
    pub account_address: String,
    pub output: serde_json::Value,
}

pub async fn mint_token_inner(
    args: &Args,
    amount: &str,
    address: &str,
) -> anyhow::Result<MintTokenResult> {
    let use_combined_mint = true;
    if use_combined_mint {
        combined_mint_token_inner(args, amount, address).await
    } else {
        separate_mint_token_inner(args, amount, address).await
    }
}

pub async fn separate_mint_token_inner(
    _args: &Args,
    amount: &str,
    address: &str,
) -> anyhow::Result<MintTokenResult> {
    let amount_satomis: u64 = amount.parse().unwrap();
    let ui_amount = token_amount_to_ui_amount(amount_satomis, 8);
    let amount_domis = ui_amount.ui_amount_string;
    info!("amount_domis: {amount_domis}");

    let mut out = Vec::new();
    let create_token_result = spl_token(&["create-token", "--decimals", "8"]);
    let token_address = create_token_result["commandOutput"]["address"]
        .as_str()
        .unwrap()
        .to_string();
    out.push(create_token_result);

    let account_address = get_account_address(Pubkey::from_str(&token_address).unwrap());

    out.push(spl_token(&["create-account", &token_address]));
    out.push(spl_token(&["mint", &token_address, &amount_domis]));
    // Disable mint
    out.push(spl_token(&[
        "authorize",
        &token_address,
        "mint",
        "--disable",
    ]));
    // Send BTCi token
    out.push(spl_token(&[
        "transfer",
        &token_address,
        &amount_domis,
        address,
        "--allow-unfunded-recipient",
        "--fund-recipient",
    ]));

    Ok(MintTokenResult {
        mint_address: token_address.clone(),
        account_address: account_address.to_string(),
        output: serde_json::to_value(out).unwrap(),
    })
}

pub async fn combined_mint_token_inner(
    args: &Args,
    amount: &str,
    address: &str,
) -> anyhow::Result<MintTokenResult> {
    let amount = amount.parse().unwrap();
    let destination_address = address.parse().unwrap();
    let decimals = 8;

    let output = combined_mint_cli(
        &args.spl_token_combined_mint_cli_path,
        amount,
        destination_address,
        args.spl_token_program_id,
        decimals,
        args.domichain_rpc_url.clone(),
        &args.domichain_service_keypair_path,
    )
    .await;

    let account_address = get_account_address(output.mint);

    Ok(MintTokenResult {
        mint_address: output.mint.to_string(),
        account_address: account_address.to_string(),
        output: serde_json::to_value(output).unwrap(),
    })
}

pub async fn burn_token_inner(args: &Args, mint_address: Pubkey, amount: u64) {
    let decimals = 8;
    let token_account_address = get_account_address(mint_address);

    let use_combined_burn = true;
    if use_combined_burn {
        info!("Burn amount integer: {amount}");
        let burn_output = combined_burn_cli(
            &args.spl_token_combined_mint_cli_path,
            amount,
            mint_address,
            token_account_address,
            args.spl_token_program_id,
            decimals,
            args.domichain_rpc_url.clone(),
            &args.domichain_service_keypair_path,
        )
        .await;
        info!("burn_output: {burn_output:#?}");
        assert_eq!(&burn_output.status, "ok");
    } else {
        let ui_amount = token_amount_to_ui_amount(amount, 8);
        let burn_amount = ui_amount.ui_amount_string;
        info!("Burn amount: {burn_amount}");
        let burn_output = spl_token(&["burn", &token_account_address.to_string(), &burn_amount]);
        info!("burn_output: {burn_output:#?}");
    }
}

pub async fn transfer_token_inner(
    args: &Args,
    mint_address: Pubkey,
    amount: u64,
    destination: Pubkey,
) {
    let decimals = 8;
    let token_account_address = get_account_address(mint_address);
    info!("Transfer amount integer: {amount}");
    let transfer_output = combined_transfer_cli(
        &args.spl_token_combined_mint_cli_path,
        amount,
        mint_address,
        token_account_address,
        destination,
        args.spl_token_program_id,
        decimals,
        args.domichain_rpc_url.clone(),
        &args.domichain_service_keypair_path,
    )
    .await;
    info!("transfer_output: {transfer_output:#?}");
    assert_eq!(&transfer_output.status, "ok");
}
