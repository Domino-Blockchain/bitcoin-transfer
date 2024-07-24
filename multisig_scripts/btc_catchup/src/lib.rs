mod domichain;
mod get_btc_transactions;
mod get_domi_transactions;
mod mempool;

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use cached::{proc_macro::io_cached, Return};
use domichain_sdk::pubkey::Pubkey;
use mempool::{Vin, Vout};
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::time::{sleep, timeout};

type DomiAddress = Pubkey;
type DomiBlock = u64;

type BtcTxHash = String;
type BtcAddress = String;
type BtcBlock = u64;

type Amount = String;

pub struct CatchupData {
    all_btc_transactions: Vec<BtcTransaction>,
    all_domi_transactions: Vec<DomiTransaction>,
    btc_address_to_domi_mints: HashMap<String, Vec<Pubkey>>,
}

pub async fn get_catchup_data(
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

    let btc_address_to_domi_mints = get_btc_addresses_to_domi_token_mints_mapping(btc_addresses);

    CatchupData {
        all_btc_transactions,
        all_domi_transactions,
        btc_address_to_domi_mints,
    }
}

#[allow(dead_code)]
pub async fn do_catchup(
    all_btc_transactions: Vec<BtcTransaction>,
    mut all_domi_transactions: Vec<DomiTransaction>,
    btc_address_to_domi_mints: HashMap<String, Vec<Pubkey>>,
) -> (Vec<BtcTransaction>, Vec<(BtcTransaction, DomiMint)>) {
    // Now tracks only deposits and mints

    let mut missed_mints = Vec::new();
    let mut amount_mismatch = Vec::new();

    for btc_tx in all_btc_transactions {
        match btc_tx.tx_type {
            BtcTransactionType::Deposit => {
                let btc_address = &btc_tx.to_address;
                let domi_token_mints = &btc_address_to_domi_mints[btc_address];
                let domi_tx_index =
                    all_domi_transactions
                        .iter()
                        .position(|domi_tx| match domi_tx {
                            DomiTransaction::Mint(domi_tx) => {
                                domi_token_mints.contains(&domi_tx.token_mint_address)
                            }
                            DomiTransaction::Burn(_) => false,
                        });
                if domi_tx_index.is_none() {
                    missed_mints.push(btc_tx);
                    continue;
                }
                let domi_tx = all_domi_transactions.remove(domi_tx_index.unwrap());
                assert!(matches!(domi_tx, DomiTransaction::Mint(..)));
                match domi_tx {
                    DomiTransaction::Mint(domi_tx) => {
                        if btc_tx.amount != domi_tx.amount {
                            amount_mismatch.push((btc_tx, domi_tx));
                        }
                    }
                    DomiTransaction::Burn(_) => unreachable!(),
                }
            }
            BtcTransactionType::Withdraw => {
                let btc_address = &btc_tx.from_address;
                let domi_token_mints = &btc_address_to_domi_mints[btc_address];
                let domi_tx_index =
                    all_domi_transactions
                        .iter()
                        .position(|domi_tx| match domi_tx {
                            DomiTransaction::Mint(_) => false,
                            DomiTransaction::Burn(domi_tx) => {
                                domi_token_mints.contains(&domi_tx.token_mint_address)
                            }
                        });
                if domi_tx_index.is_none() {
                    // TODO: do burn
                    panic!("No DOMI burn found for BTC withdraw. {btc_tx:?}");
                }
                let domi_tx = all_domi_transactions.remove(domi_tx_index.unwrap());
                assert!(matches!(domi_tx, DomiTransaction::Burn(..)));
                match domi_tx {
                    DomiTransaction::Mint(_) => unreachable!(),
                    DomiTransaction::Burn(domi_tx) => {
                        assert_eq!(btc_tx.amount, domi_tx.amount);
                    }
                }
            }
        }
    }
    assert!(
        all_domi_transactions.is_empty(),
        "No BTC transactions for these DOMI transactions: {all_domi_transactions:?}",
    );
    (missed_mints, amount_mismatch)
}

#[tokio::test]
async fn test_do_catchup() {
    use std::str::FromStr;
    let spl_token_program_id =
        Pubkey::from_str("BTCi9FUjBVY3BSaqjzfhEPKVExuvarj8Gtfn4rJ5soLC").unwrap();
    let service_address = Pubkey::from_str("4qovDeQM5kG2z9EZJQ6s93f8yak6VKrHyxWMjZva2daE").unwrap();
    do_catchup(
        spl_token_program_id,
        service_address,
        vec![
            "bc1qj80eg85adqgcj78ydplmrnlfs7a7gnefzt7ztwzh6cnus3sj7arqfpnpvk".to_string(),
            "bc1qzqqg6fy048cfa2zr9wm3nggdps0vtt3cwpj8uapkw6gtdgj3hu8qdqjzwn".to_string(),
            "bc1qrqd3f0k9a6fcyvxnvpathv0mj59paqrpge84zw0fmuuz2r0956eq24nzlv".to_string(),
            "bc1q2erg27wzdrdk4r0zgrz45g0x538lwu38anl4c693jms3k5fkurgqfqvskj".to_string(),
            "bc1q2w797kwlnsr0x598krnhqx9p48a736nw8qssev3wenpeee6udnhsxsdzgx".to_string(),
            "bc1q0wsa9fskg0j7yauv5m6qgveld7c9j42w48x05pvwes5ks752jsgqrmezy4".to_string(),
            "bc1qwhkuu7uw6gcudl5kcwmarwp0rzfrw2yrs4ttz5vm08zua06d25sq6es6ha".to_string(),
            "bc1q7nymhhyev263q33t6hu94uncn2mkg3vf8ml0f5un4tafsnsem6ksvc7jlt".to_string(),
            "bc1qdrkyndwvde4drersh9vppe04nr2rqhepaa55z9pfkspcrz3qeymq5d0fck".to_string(),
            "bc1qh6tkze3j25x9dyrxv8vhhavkkg4m7ynjuleg5azanc2tr5r4wt3swx6ldp".to_string(),
            "bc1q82jsgl9zdy3qxn2ey7a0yw9q6k0pz5zjwmkjz5emmszsafupl8wqnlpw3w".to_string(),
            "bc1qy2p5ar57q852qahwstyhs4wp7ut8zr45yvv3ahaaj9vtcyu2m7usyfmc70".to_string(),
            "bc1qk6qxaa53r0wfjj2lg68vgw2d35end50k37mynpzx8htau0mh3h8swd80ls".to_string(),
            "bc1qjalzzqjkydsmftl62hkxy5r56dlga9aykhlm0gstnhudx8jrdvcsfavgpv".to_string(),
            "bc1qrcxtjeyvgz50t25eq7yl3hngk4l4cq25zmpfz0dlx37ttpqjemzqjtxuc0".to_string(),
            "bc1qkyat094k65ff3m0w2mtq6zltcsqyw2kjpu8gha8kafhk40zpxh3snydsjw".to_string(),
            "bc1q5x6mwk6mrj8757u6pmhlwf0fqt5j6mpapdhx86rnayd6rmda026szr96ay".to_string(),
        ],
    )
    .await;
}

#[derive(Debug, Clone, Copy)]
pub enum BtcTransactionType {
    Deposit,
    Withdraw,
}

#[derive(Debug)]
pub struct BtcTransaction {
    pub tx_id: BtcTxHash,
    pub vin: Vec<Vin>,
    pub vout: Vec<Vout>,
    pub from_address: BtcAddress,
    pub to_address: BtcAddress,
    pub tx_type: BtcTransactionType,
    pub amount: Amount,
    pub block: BtcBlock,
}

/// Incoming and outgoing transactions of BTC address
pub async fn get_btc_transactions(address: &str) -> Vec<BtcTransaction> {
    get_btc_transactions::get_btc_transactions(address).await
}

#[derive(Debug)]
pub struct DomiMint {
    pub token_mint_address: DomiAddress,
    // to_address: DomiAddress,
    pub amount: Amount,
    pub block: DomiBlock,
}

#[derive(Debug)]
pub struct DomiBurn {
    pub token_mint_address: DomiAddress,
    pub from_address: DomiAddress,
    pub amount: Amount,
    pub block: DomiBlock,
}

#[derive(Debug)]
pub enum DomiTransaction {
    Mint(DomiMint),
    Burn(DomiBurn),
}

pub async fn get_domi_transactions(
    spl_token_program_id: Pubkey,
    service_address: Pubkey,
) -> Vec<DomiTransaction> {
    get_domi_transactions::get_domi_transactions(spl_token_program_id, service_address).await
}

/// Get one to many mapping:
///
/// btc_address -1-to-N- domi_token_mint
fn get_btc_addresses_to_domi_token_mints_mapping(
    btc_addresses: Vec<BtcAddress>,
) -> HashMap<BtcAddress, Vec<DomiAddress>> {
    #[cfg_attr(rustfmt, rustfmt_skip)]
    [
        ("bc1qj80eg85adqgcj78ydplmrnlfs7a7gnefzt7ztwzh6cnus3sj7arqfpnpvk".to_string(), vec![]),
        ("bc1qzqqg6fy048cfa2zr9wm3nggdps0vtt3cwpj8uapkw6gtdgj3hu8qdqjzwn".to_string(), vec![]),
        ("bc1qrqd3f0k9a6fcyvxnvpathv0mj59paqrpge84zw0fmuuz2r0956eq24nzlv".to_string(), vec![
            "GEPmYVQQiCCRGtuNEi4PczMSTRwKbwySkeESXKmpMR3z".parse().unwrap(),
        ]),
        ("bc1q2erg27wzdrdk4r0zgrz45g0x538lwu38anl4c693jms3k5fkurgqfqvskj".to_string(), vec![]),
        ("bc1q2w797kwlnsr0x598krnhqx9p48a736nw8qssev3wenpeee6udnhsxsdzgx".to_string(), vec![]),
        ("bc1q0wsa9fskg0j7yauv5m6qgveld7c9j42w48x05pvwes5ks752jsgqrmezy4".to_string(), vec![]),
        ("bc1qwhkuu7uw6gcudl5kcwmarwp0rzfrw2yrs4ttz5vm08zua06d25sq6es6ha".to_string(), vec![
            "Dm6phGa5eh7ihFtvbqM2cjxYrpvvzg5h5y3CnrXHEb2x".parse().unwrap(),
        ]),
        ("bc1q7nymhhyev263q33t6hu94uncn2mkg3vf8ml0f5un4tafsnsem6ksvc7jlt".to_string(), vec![]),
        ("bc1qdrkyndwvde4drersh9vppe04nr2rqhepaa55z9pfkspcrz3qeymq5d0fck".to_string(), vec![]),
        ("bc1qh6tkze3j25x9dyrxv8vhhavkkg4m7ynjuleg5azanc2tr5r4wt3swx6ldp".to_string(), vec![]),
        ("bc1q82jsgl9zdy3qxn2ey7a0yw9q6k0pz5zjwmkjz5emmszsafupl8wqnlpw3w".to_string(), vec![
            "DcJWn7tkiC5dNAsyUAu9Q64dzCZ964xBD5bemnGsJ1Mf".parse().unwrap(),
        ]),
        ("bc1qy2p5ar57q852qahwstyhs4wp7ut8zr45yvv3ahaaj9vtcyu2m7usyfmc70".to_string(), vec![]),
        ("bc1qk6qxaa53r0wfjj2lg68vgw2d35end50k37mynpzx8htau0mh3h8swd80ls".to_string(), vec![]),
        ("bc1qjalzzqjkydsmftl62hkxy5r56dlga9aykhlm0gstnhudx8jrdvcsfavgpv".to_string(), vec![]),
        ("bc1qrcxtjeyvgz50t25eq7yl3hngk4l4cq25zmpfz0dlx37ttpqjemzqjtxuc0".to_string(), vec![]),
        ("bc1qkyat094k65ff3m0w2mtq6zltcsqyw2kjpu8gha8kafhk40zpxh3snydsjw".to_string(), vec![]),
        ("bc1q5x6mwk6mrj8757u6pmhlwf0fqt5j6mpapdhx86rnayd6rmda026szr96ay".to_string(), vec![]),
    ].into_iter().collect()
}

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub enum GetJsonError {
    IoError(String),
    ReqwestError(reqwest::Error),
}

#[io_cached(
    map_error = r##"|e| GetJsonError::IoError(format!("{e:?}"))"##,
    disk = true,
    with_cached_flag = true
)]
pub async fn get_json_cached(url: String) -> Result<Return<serde_json::Value>, GetJsonError> {
    dbg!(&url);
    let start = Instant::now();

    let default_sleep = Duration::from_millis(500);
    let mut retry_sleep = Duration::from_secs(2);

    let response = loop {
        sleep(default_sleep).await;
        let response = timeout(REQUEST_TIMEOUT, reqwest::get(&url))
            .await
            .unwrap()
            .map_err(GetJsonError::ReqwestError)?;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            dbg!(retry_sleep);
            sleep(retry_sleep).await;
            retry_sleep *= 2;
            continue;
        }
        break response;
    };

    let res: serde_json::Value = timeout(REQUEST_TIMEOUT, response.json())
        .await
        .unwrap()
        .map_err(GetJsonError::ReqwestError)?;
    dbg!(start.elapsed());
    Ok(Return::new(res))
}

pub async fn get_json<T: for<'a> Deserialize<'a>>(url: String) -> Result<T, reqwest::Error> {
    let start = Instant::now();
    let res = get_json_cached(url).await;
    dbg!(start.elapsed());
    if let Ok(ret) = &res {
        dbg!(ret.was_cached);
    }
    res.map(|v| serde_json::from_value(v.value).unwrap())
        .map_err(|err| match err {
            GetJsonError::IoError(err) => panic!("IO: Error: {err}"),
            GetJsonError::ReqwestError(err) => err,
        })
}

#[tokio::test]
async fn test_get_json() {
    for _ in 0..3 {
        let start = Instant::now();
        let res = get_json_cached(format!(
            "https://mempool.space/api/address/bc1q35yhc5khmqr6q5wxne6dud233wzefy43k4w9sv/txs"
        ))
        .await;
        dbg!(start.elapsed());
        dbg!(res.is_ok());
        if let Ok(ret) = &res {
            dbg!(ret.was_cached);
        }
        dbg!();
    }
}
