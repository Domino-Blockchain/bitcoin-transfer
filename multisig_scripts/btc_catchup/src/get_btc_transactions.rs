use std::collections::{HashMap, HashSet};

use crate::{
    mempool::{self, Vout},
    BtcTransaction, BtcTransactionType,
};

/*
#[derive(Debug)]
pub struct BtcTransaction {
    from_address: BtcAddress,
    to_address: BtcAddress,
    tx_type: BtcTransactionType,
    amount: Amount,
    block: BtcBlock,
}
*/

/// See: https://mempool.space/docs/api/rest#get-address-transactions-chain
///
/// curl -sSL "https://mempool.space/api/address/1wiz18xYmhRX6xStj2b9t1rwWX4GKUgpv/txs/chain"
///
pub async fn get_btc_transactions(address: &str) -> Vec<BtcTransaction> {
    let include_withdraws = false;

    let txs = mempool::get_address_txs_chain(address).await.unwrap();

    let multisig_address = address;

    let mut result = Vec::new();
    for tx in txs {
        assert!(tx.status.confirmed);

        let block = tx.status.block_height;

        let vin_multisig = tx
            .vin
            .iter()
            .any(|vin| vin.prevout.scriptpubkey_address == multisig_address);
        let vout_multisig = tx.vout.iter().any(|vout| {
            vout.scriptpubkey_address.as_ref().map(|a| a.as_str()) == Some(multisig_address)
        });
        let tx_type = match (vin_multisig, vout_multisig) {
            (true, true) => {
                assert!(tx
                    .vin
                    .iter()
                    .all(|vin| &vin.prevout.scriptpubkey_address == multisig_address));

                let source_amount: u64 = tx.vin.iter().map(|vin| vin.prevout.value).sum();
                let destination_amount: u64 = tx
                    .vout
                    .iter()
                    .filter_map(|vout| {
                        (vout.scriptpubkey_address.as_ref().map(|a| a.as_str())
                            == Some(multisig_address))
                        .then_some(vout.value)
                    })
                    .sum();

                assert!(source_amount > destination_amount);

                BtcTransactionType::Withdraw // multisig_address in input and output (change)
            }
            (true, false) => BtcTransactionType::Withdraw, // multisig_address in input
            (false, true) => BtcTransactionType::Deposit,  // multisig_address in output
            (false, false) => panic!("No multisig address in transaction"),
        };

        let from_address;
        let to_address;
        let amount: u64;

        match tx_type {
            BtcTransactionType::Deposit => {
                let input_addresses: HashSet<_> = tx
                    .vin
                    .iter()
                    .map(|vin| vin.prevout.scriptpubkey_address.as_str())
                    .collect();
                if input_addresses.len() > 1 {
                    panic!(
                        "Could not define single `from_address`. TX ID: {}",
                        &tx.txid
                    );
                }
                from_address = input_addresses.into_iter().next().unwrap().to_string();

                // Discard other outgoing BTC
                amount = tx
                    .vout
                    .iter()
                    .filter_map(|vout| {
                        (vout.scriptpubkey_address.as_ref().map(|a| a.as_str())
                            == Some(multisig_address))
                        .then_some(vout.value)
                    })
                    .sum();
                assert_ne!(amount, 0);

                to_address = multisig_address.to_string();
            }
            BtcTransactionType::Withdraw => {
                if !include_withdraws {
                    continue;
                }

                assert!(tx
                    .vout
                    .iter()
                    .all(|vout| vout.scriptpubkey_address.is_some()));

                let input_addresses: HashSet<_> = tx
                    .vin
                    .iter()
                    .map(|vin| vin.prevout.scriptpubkey_address.as_str())
                    .collect();
                if input_addresses.len() > 1 {
                    panic!("Could not define single `from_address`. Should be single `multisig_address`");
                }
                from_address = input_addresses.into_iter().next().unwrap().to_string();

                // Ugnore change addresses
                let destination_amounts: HashMap<_, _> =
                    tx.vout
                        .iter()
                        .filter_map(|vout| {
                            (vout.scriptpubkey_address.as_ref().unwrap() != multisig_address)
                                .then_some((
                                    vout.scriptpubkey_address.as_ref().unwrap().as_str(),
                                    vout.value,
                                ))
                        })
                        .collect();
                assert_eq!(destination_amounts.len(), 1);
                let (destination, destination_amount) =
                    destination_amounts.into_iter().next().unwrap();
                assert_ne!(destination_amount, 0);
                amount = destination_amount + tx.fee;
                to_address = destination.to_string();
            }
        }

        let amount = amount.to_string();

        result.push(BtcTransaction {
            tx_id: tx.txid,
            vout: tx.vout,
            from_address,
            to_address,
            tx_type,
            amount,
            block,
        })
    }

    if !include_withdraws {
        assert!(result
            .iter()
            .all(|tx| matches!(tx.tx_type, BtcTransactionType::Deposit)));
    }

    result
}

#[tokio::test]
async fn test_get_btc_transactions() {
    // Random address
    // dbg!(get_btc_transactions("bc1q35yhc5khmqr6q5wxne6dud233wzefy43k4w9sv").await);

    // Multisig address:
    dbg!(
        get_btc_transactions("bc1qrqd3f0k9a6fcyvxnvpathv0mj59paqrpge84zw0fmuuz2r0956eq24nzlv")
            .await
    );
}
