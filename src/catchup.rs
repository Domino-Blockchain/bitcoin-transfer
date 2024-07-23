use std::{collections::HashMap, sync::Arc};

use btc_catchup::{get_btc_transactions, get_domi_transactions, BtcTransaction, DomiTransaction};
use domichain_sdk::pubkey::Pubkey;

use crate::db::DB;

type BtcAddress = String;

pub struct CatchupData {
    pub all_btc_transactions: Vec<BtcTransaction>,
    pub all_domi_transactions: Vec<DomiTransaction>,
    pub btc_address_to_domi_mints: HashMap<String, Vec<Pubkey>>,
}

pub async fn get_catchup_data(
    db: Arc<DB>,
    spl_token_program_id: Pubkey,
    service_address: Pubkey,
    btc_addresses: Vec<BtcAddress>,
) -> CatchupData {
    // Might be duplicates: in case of withdraw to deposit address.
    let mut all_btc_transactions = Vec::new();
    for a in &btc_addresses {
        let txs = get_btc_transactions(a).await;
        all_btc_transactions.extend(txs);
    }
    all_btc_transactions.sort_by_key(|tx| tx.block);

    let mut all_domi_transactions =
        get_domi_transactions(spl_token_program_id, service_address).await;
    all_domi_transactions.sort_by_key(|tx| match tx {
        DomiTransaction::Mint(m) => m.block,
        DomiTransaction::Burn(b) => b.block,
    });

    let btc_address_to_domi_mints = db.get_multisig_address_to_mint_addresses_mapping().await;

    CatchupData {
        all_btc_transactions,
        all_domi_transactions,
        btc_address_to_domi_mints,
    }
}
