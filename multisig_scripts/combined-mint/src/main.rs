use std::{path::PathBuf, sync::Arc};

use clap::{Parser, Subcommand};
use domichain_client::nonblocking::rpc_client::RpcClient;
use domichain_sdk::{
    commitment_config::CommitmentConfig, program_pack::Pack, pubkey::Pubkey, signature::Keypair,
    signer::Signer,
};
use spl_token::state::Account;
use spl_token_client::{
    client::{ProgramRpcClient, ProgramRpcClientSendTransaction, RpcClientResponse},
    token::Token,
};

/// Simple program to mint and burn tokens in a single transaction
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Mint and transfer tokens in a single transaction
    Mint(MintArgs),
    Burn(BurnArgs),
    Transfer(TransferArgs),
}

#[derive(clap::Args, Debug)]
struct MintArgs {
    /// Amount of tokens to mint
    #[arg(long)]
    amount: u64,

    /// Destination user wallet address
    #[arg(long)]
    destination_address: Pubkey,

    /// Program ID: {spl_token, spl_token_btci, spl_token_usdt}::id()
    #[arg(long)]
    token_program: Pubkey,

    /// SPL Token decimals
    #[arg(long)]
    decimals: u8,

    /// URL for Domichain's JSON RPC
    #[arg(long)]
    url: String,

    /// Filepath to a keypair
    #[arg(long)]
    keypair: PathBuf,
}

#[derive(clap::Args, Debug)]
struct BurnArgs {
    /// Amount of tokens to mint
    #[arg(long)]
    amount: u64,

    /// Program ID: {spl_token, spl_token_btci, spl_token_usdt}::id()
    #[arg(long)]
    token_program: Pubkey,

    /// SPL Token decimals
    #[arg(long)]
    decimals: u8,

    /// URL for Domichain's JSON RPC
    #[arg(long)]
    url: String,

    /// Filepath to a keypair
    #[arg(long)]
    keypair: PathBuf,

    #[arg(long)]
    mint_address: Pubkey,

    #[arg(long)]
    token_account_address: Pubkey,
}

#[derive(clap::Args, Debug)]
struct TransferArgs {
    /// Amount of tokens to mint
    #[arg(long)]
    amount: u64,

    /// Program ID: {spl_token, spl_token_btci, spl_token_usdt}::id()
    #[arg(long)]
    token_program: Pubkey,

    /// SPL Token decimals
    #[arg(long)]
    decimals: u8,

    /// URL for Domichain's JSON RPC
    #[arg(long)]
    url: String,

    /// Filepath to a keypair
    #[arg(long)]
    keypair: PathBuf,

    #[arg(long)]
    mint_address: Pubkey,

    #[arg(long)]
    token_account_address: Pubkey,

    #[arg(long)]
    destination_token_account_address: Pubkey,
}

/*
cargo run -r -- mint \
    --amount 10 \
    --destination-address $(domichain-keygen pubkey test_key.json) \
    --token-program BTCi9FUjBVY3BSaqjzfhEPKVExuvarj8Gtfn4rJ5soLC \
    --decimals 8 \
    --url https://api.testnet.domichain.io \
    --keypair /home/domi/.config/domichain/id.json

cargo run -r -- burn \
    --amount 1 \
    --token-program BTCi9FUjBVY3BSaqjzfhEPKVExuvarj8Gtfn4rJ5soLC \
    --decimals 8 \
    --url https://api.testnet.domichain.io \
    --keypair test_key.json \
    --mint-address HU8wy2oocPzYvFsJAx6aEX1NWjc5TbSEyWB1cJTLVAsT \
    --token-account-address 6f8umNr1GWsyZ3x71N7TPP5itS827i7pGSbHNWJSEqP2
*/

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match args.command {
        Command::Mint(args) => mint(args).await,
        Command::Burn(args) => burn(args).await,
        Command::Transfer(args) => transfer(args).await,
    };
}

async fn mint(args: MintArgs) {
    let MintArgs {
        amount,
        destination_address,
        token_program,
        decimals,
        url,
        keypair,
    } = args;

    // let token_program = id();
    // let url = "http://127.0.0.1:8899".to_string();
    // let keypair = "/home/domi/.config/domichain/id.json";
    // let decimals = 8;

    let mint_account = Arc::new(Keypair::new());

    let client = Arc::new(RpcClient::new_with_commitment(
        url,
        CommitmentConfig::confirmed(),
    ));
    let rpc_client = Arc::new(ProgramRpcClient::new(
        client.clone(),
        ProgramRpcClientSendTransaction,
    ));

    let id_raw = tokio::fs::read_to_string(keypair).await.unwrap();
    let id_bytes: Vec<u8> = serde_json::from_str(&id_raw).unwrap();
    let payer = Arc::new(Keypair::from_bytes(&id_bytes).unwrap());

    let token_client = Token::new(
        rpc_client,
        &token_program,
        &mint_account.pubkey(),
        Some(decimals),
        payer.clone(),
    );

    let owner = payer.clone();

    combined_flow(
        client,
        token_client,
        owner,
        mint_account,
        destination_address,
        amount,
    )
    .await;
}

async fn combined_flow(
    client: Arc<RpcClient>,
    token_client: Token<ProgramRpcClientSendTransaction>,
    owner: Arc<Keypair>,
    mint_account: Arc<Keypair>,
    destination_address: Pubkey,
    amount: u64,
) {
    let mint_address = &mint_account.pubkey();

    let mint_authority = &owner.pubkey();
    let freeze_authority = None;
    let extension_initialization_params = vec![];

    let system_owner = &owner.pubkey();
    let dest_owner = &destination_address;

    let system_account = &token_client.get_associated_token_address(system_owner);
    let dest_account = &token_client.get_associated_token_address(dest_owner);

    let mut token_instructions = Vec::new();

    let signing_keypairs = &[owner.as_ref()];

    let create_mint_signing_keypairs = &[mint_account.as_ref()];

    let set_authority_signing_keypairs = &[mint_account.as_ref()];

    let combined_signing_keypairs = &[
        create_mint_signing_keypairs[0],
        signing_keypairs[0],
        set_authority_signing_keypairs[0],
    ];

    let create_mint_ix = token_client
        .create_mint_ix(
            mint_authority,
            freeze_authority,
            extension_initialization_params,
            create_mint_signing_keypairs,
        )
        .await
        .unwrap();
    token_instructions.extend_from_slice(&create_mint_ix);

    let create_associated_token_account_ix = token_client
        .create_associated_token_account_ix(system_owner)
        .await
        .unwrap();
    token_instructions.extend_from_slice(&create_associated_token_account_ix);

    let create_associated_token_account_ix = token_client
        .create_associated_token_account_ix(dest_owner)
        .await
        .unwrap();
    token_instructions.extend_from_slice(&create_associated_token_account_ix);

    let mint_to_ix = token_client
        .mint_to_ix(system_account, mint_authority, amount, signing_keypairs)
        .await
        .unwrap();
    token_instructions.extend_from_slice(&mint_to_ix);

    let set_authority_ix = token_client
        .set_authority_ix(
            mint_address,
            mint_authority,
            None,
            spl_token_client::spl_token_2022::instruction::AuthorityType::MintTokens,
            set_authority_signing_keypairs,
        )
        .await
        .unwrap();
    token_instructions.extend_from_slice(&set_authority_ix);

    let transfer_ix = token_client
        .transfer_ix(
            system_account,
            dest_account,
            system_owner,
            amount,
            signing_keypairs,
        )
        .await
        .unwrap();
    token_instructions.extend_from_slice(&transfer_ix);

    let res = token_client
        .process_ixs(&token_instructions, combined_signing_keypairs)
        .await
        .unwrap();
    let signature = match res {
        RpcClientResponse::Signature(signature) => signature,
        RpcClientResponse::Transaction(tx) => unreachable!("{tx:?}"),
    };

    // Inspect account
    let token_account_info = client.get_account_data(dest_account).await.unwrap();
    let account_data = Account::unpack(&token_account_info).unwrap();
    assert_eq!(account_data.amount, amount, "not correct amount");

    let output = serde_json::json!({
        "status": "ok",
        "mint": mint_address.to_string(),
        "destination_account": dest_account.to_string(),
        "amount": amount,
        "signature": signature.to_string(),
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

async fn burn(args: BurnArgs) {
    let BurnArgs {
        amount,
        token_program,
        decimals,
        url,
        keypair,
        mint_address,
        token_account_address,
    } = args;

    let client = Arc::new(RpcClient::new_with_commitment(
        url,
        CommitmentConfig::confirmed(),
    ));
    let rpc_client = Arc::new(ProgramRpcClient::new(
        client.clone(),
        ProgramRpcClientSendTransaction,
    ));

    let id_raw = tokio::fs::read_to_string(keypair).await.unwrap();
    let id_bytes: Vec<u8> = serde_json::from_str(&id_raw).unwrap();
    let payer = Arc::new(Keypair::from_bytes(&id_bytes).unwrap());

    let token_client = Token::new(
        rpc_client,
        &token_program,
        &mint_address,
        Some(decimals),
        payer.clone(),
    );

    let authority = payer.pubkey();

    let signing_keypairs = &[payer.as_ref()];

    let res = token_client
        .burn(&token_account_address, &authority, amount, signing_keypairs)
        .await
        .unwrap();
    let signature = match res {
        RpcClientResponse::Signature(signature) => signature,
        RpcClientResponse::Transaction(tx) => unreachable!("{tx:?}"),
    };

    let output = serde_json::json!({
        "status": "ok",
        "mint": mint_address.to_string(),
        "token_account": token_account_address.to_string(),
        "amount": amount,
        "signature": signature.to_string(),
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

async fn transfer(args: TransferArgs) {
    let TransferArgs {
        amount,
        token_program,
        decimals,
        url,
        keypair,
        mint_address,
        token_account_address,
        destination_token_account_address,
    } = args;

    let client = Arc::new(RpcClient::new_with_commitment(
        url,
        CommitmentConfig::confirmed(),
    ));
    let rpc_client = Arc::new(ProgramRpcClient::new(
        client.clone(),
        ProgramRpcClientSendTransaction,
    ));

    let id_raw = tokio::fs::read_to_string(keypair).await.unwrap();
    let id_bytes: Vec<u8> = serde_json::from_str(&id_raw).unwrap();
    let payer = Arc::new(Keypair::from_bytes(&id_bytes).unwrap());

    let token_client = Token::new(
        rpc_client,
        &token_program,
        &mint_address,
        Some(decimals),
        payer.clone(),
    );

    let authority = payer.pubkey();

    let signing_keypairs = &[payer.as_ref()];

    let source = token_account_address;
    let destination = destination_token_account_address;

    let res = token_client
        .transfer(&source, &destination, &authority, amount, signing_keypairs)
        .await
        .unwrap();
    let signature = match res {
        RpcClientResponse::Signature(signature) => signature,
        RpcClientResponse::Transaction(tx) => unreachable!("{tx:?}"),
    };

    let output = serde_json::json!({
        "status": "ok",
        "mint": mint_address.to_string(),
        "token_account": token_account_address.to_string(),
        "destination_token_account": destination_token_account_address.to_string(),
        "amount": amount,
        "signature": signature.to_string(),
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

#[tokio::test]
async fn test_burn() {
    burn(BurnArgs {
        amount: 1,
        token_program: "BTCi9FUjBVY3BSaqjzfhEPKVExuvarj8Gtfn4rJ5soLC"
            .parse()
            .unwrap(),
        decimals: 8,
        url: "https://api.testnet.domichain.io/".to_string(),
        keypair: "/home/btc-transfer/bitcoin-transfer/multisig_scripts/combined-mint/test_key.json"
            .into(),
        mint_address: "HU8wy2oocPzYvFsJAx6aEX1NWjc5TbSEyWB1cJTLVAsT"
            .parse()
            .unwrap(),
        token_account_address: "6f8umNr1GWsyZ3x71N7TPP5itS827i7pGSbHNWJSEqP2"
            .parse()
            .unwrap(),
    })
    .await;
}
