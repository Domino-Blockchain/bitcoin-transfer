use std::{collections::HashMap, sync::Arc};

use btc_catchup::{
    get_btc_transactions, get_domi_transactions, BtcTransaction, BtcTransactionType,
    DomiTransaction,
};
use domichain_sdk::pubkey::Pubkey;
use tracing::warn;

use crate::{
    db::DB,
    watch_addresses::{process_confirmed_transaction, Confirmed, Vin, VinPrevout, Vout},
    AppState,
};

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
    btc_addresses: &[BtcAddress],
) -> CatchupData {
    // Might be duplicates: in case of withdraw to deposit address.
    let mut all_btc_transactions = Vec::new();
    for a in btc_addresses {
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

pub async fn process_catchup(
    app_state: &AppState,
    spl_token_program_id: Pubkey,
    domichain_service_address: Pubkey,
    all_multisig_addresses: &[String],
) {
    let CatchupData {
        all_btc_transactions,
        all_domi_transactions,
        btc_address_to_domi_mints,
    } = get_catchup_data(
        app_state.db.clone(),
        spl_token_program_id,
        domichain_service_address,
        all_multisig_addresses,
    )
    .await;

    let (mut missed_mints, mut amount_mismatch) = btc_catchup::do_catchup(
        all_btc_transactions,
        all_domi_transactions,
        btc_address_to_domi_mints,
    )
    .await;

    missed_mints.iter().for_each(|btc_tx| {
        warn!("No DOMI mint found for BTC deposit. {btc_tx:#?}");
    });

    amount_mismatch.iter().for_each(|(btc_tx, domi_mint)| {
        warn!("Amount mismatch for deposit and mint: {btc_tx:#?}\n{domi_mint:#?}");
    });

    for btc_tx in missed_mints.drain(..) {
        let vout = btc_tx
            .vout
            .into_iter()
            .filter_map(|vout| {
                vout.scriptpubkey_address.map(|scriptpubkey_address| Vout {
                    scriptpubkey_address: scriptpubkey_address,
                    value: vout.value,
                })
            })
            .collect();
        let vin = btc_tx
            .vin
            .into_iter()
            .map(|vin| Vin {
                prevout: VinPrevout {
                    scriptpubkey_address: vin.prevout.scriptpubkey_address,
                },
            })
            .collect();
        let confirmed_tx = Confirmed {
            txid: btc_tx.tx_id,
            vin,
            vout,
        };
        assert!(matches!(btc_tx.tx_type, BtcTransactionType::Deposit));
        let multisig_address = btc_tx.to_address;
        process_confirmed_transaction(app_state, &multisig_address, confirmed_tx).await;
    }

    amount_mismatch.retain(|(btc_tx, _domi_tx)| {
        let skip_txs = ["f697db2d2962b976150aae2c2292fdb3df3938c82fe67327aa5600d29fa0d75f"];
        !skip_txs.contains(&btc_tx.tx_id.as_str())
    });

    assert!(missed_mints.is_empty());
    assert!(amount_mismatch.is_empty());
}
