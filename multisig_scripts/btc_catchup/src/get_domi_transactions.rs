use std::time::Instant;

use domichain_sdk::pubkey::Pubkey;
use serde_json::json;
use tokio::task::JoinSet;

use crate::{
    domichain::{self, get_transaction, ConfirmationStatus},
    DomiMint, DomiTransaction,
};

pub async fn get_domi_transactions(
    spl_token_program_id: Pubkey,
    service_address: Pubkey,
) -> Vec<DomiTransaction> {
    let mut signatures = domichain::get_signatures_for_address(service_address)
        .await
        .unwrap();

    signatures.retain(|tx| {
        tx.err.is_none() && matches!(tx.confirmation_status, Some(ConfirmationStatus::Finalized))
    });

    dbg!(signatures.len());

    let start = Instant::now();
    let mut join_set = JoinSet::new();
    for sig in signatures {
        join_set.spawn(async move { get_transaction(sig.signature).await });
    }
    let mut tx_infos = Vec::new();
    while let Some(res) = join_set.join_next().await {
        let tx_info = res.unwrap().unwrap();
        assert!(tx_info.meta.as_ref().unwrap().err.is_none());
        assert!(tx_info.meta.as_ref().unwrap().status == json!({"Ok": null}));
        tx_infos.push(tx_info);
    }
    dbg!(start.elapsed());

    // let programs: Vec<Vec<_>> = tx_infos
    //     .iter()
    //     .map(|tx_info| {
    //         tx_info
    //             .transaction
    //             .message
    //             .instructions
    //             .iter()
    //             .map(|ix| (ix.program.clone(), ix.parsed.instruction_type.clone()))
    //             .collect()
    //     })
    //     .collect();
    // dbg!(&programs);

    let mints: Vec<_> = tx_infos
        .iter()
        .filter(|tx_info| {
            let ixs: Vec<_> = tx_info
                .transaction
                .message
                .instructions
                .iter()
                .filter(|ix| {
                    ix.program_id == spl_token_program_id
                        && ix.parsed.instruction_type == "mintToChecked"
                })
                .map(|ix| (ix.program.clone(), ix.parsed.instruction_type.clone()))
                .collect();
            // TODO: handle `MintTo`
            ixs.contains(&("spl-token".to_string(), "mintToChecked".to_string()))
        })
        .collect();

    let mut all_txs = Vec::new();
    for tx_info in mints {
        let ixs: Vec<_> = tx_info
            .transaction
            .message
            .instructions
            .iter()
            .filter(|ix| {
                ix.program_id == spl_token_program_id
                    && ix.parsed.instruction_type == "mintToChecked"
            })
            .collect();
        assert_eq!(ixs.len(), 1, "ixs: {:#?}\n, tx_info: {tx_info:#?}", &ixs);
        let mint_ix = &ixs[0];

        let token_mint_address = mint_ix.parsed.info["mint"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        let amount = mint_ix.parsed.info["tokenAmount"]["amount"]
            .as_str()
            .unwrap()
            .to_string();
        let block = tx_info.slot;

        // FIXME: find destination of mint
        // let to_address = "UNDEFINED".to_string();

        all_txs.push(DomiTransaction::Mint(DomiMint {
            token_mint_address,
            // to_address,
            amount,
            block,
        }));
    }

    all_txs
}

#[tokio::test]
async fn test_get_domi_transactions() {
    use std::str::FromStr;
    let spl_token_program_id =
        Pubkey::from_str("BTCi9FUjBVY3BSaqjzfhEPKVExuvarj8Gtfn4rJ5soLC").unwrap();
    let service_address = Pubkey::from_str("4qovDeQM5kG2z9EZJQ6s93f8yak6VKrHyxWMjZva2daE").unwrap();
    dbg!(get_domi_transactions(spl_token_program_id, service_address).await);
}
