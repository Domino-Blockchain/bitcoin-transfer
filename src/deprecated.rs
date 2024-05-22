use axum::Json;
use bdk::bitcoin::{Address, Network};
use bdk::blockchain::{Blockchain, ElectrumBlockchain};
use bdk::database::MemoryDatabase;
use bdk::electrum_client::Client;
use bdk::keys::{
    bip39::{Language, Mnemonic, WordCount},
    DerivableKey, ExtendedKey, GeneratableKey, GeneratedKey,
};
use bdk::template::Bip84;
use bdk::wallet::AddressIndex::{LastUnused, Peek};
use bdk::{
    miniscript, Balance, KeychainKind, SignOptions, SyncOptions, TransactionDetails, Wallet,
};
use serde::Deserialize;

use crate::balance_by_addresses::get_known_addresses;
use crate::log_progress;
use crate::spl_token::spl_token;

// e:0:tb1q6dsqge320xzu7g64d5arp4qx6ldvz6xd27zvgy:0
// e:1:tb1qsvsqza56mdcmp8d02ttq06grdrcjmtcnxd08pf:779
// e:2:tb1q2kpgx8474rkttkxl9yq6e8e06u9egw7ep2k4vf:0
// e:3:tb1q2p9nlkfkjpx68ex24uv2cau4rdjy0ft7qxwjl0:0
// i:0:tb1q5fyz7lm2xmvlj0808lzytlavu487qhwc4n7m4v:2302
// i:1:tb1q255gg70ev9xhld0uywz2w6knnrvlalkju44td9:0
// i:2:tb1q4ve39l7gn8zyr8zf2luxfn3legtd777j2fcjw4:418
// i:3:tb1qqjahfdv2hlyvfghnmnzddwwsv5ww6uv582cvwc:0
const SERVICE_ADDRESS: [u8; 32] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2,
];

// e:0:tb1q76ksqtcp3ph20ne5rlduz3xvvc0ysv6chj8w3n:841
// e:1:tb1q8y067gys5k6pm8gdcpnq4evf6z477h9fkpcdl3:0
// i:0:tb1qqph07frjg6fng2dwr2hkz9dh66katry8a28hv8:0
const USER_ADDRESS: [u8; 32] = [
    10, 20, 30, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2,
    3,
];

fn mnemonic_from_entropy(entropy: [u8; 32]) -> GeneratedKey<Mnemonic, miniscript::Segwitv0> {
    Mnemonic::generate_with_entropy((WordCount::Words12, Language::English), entropy).unwrap()
}

#[derive(Deserialize)]
pub struct BurnTokenRequest {
    account_address: String,
    amount: u64,
}
pub async fn burn_token(Json(request): Json<BurnTokenRequest>) -> Json<serde_json::Value> {
    Json(spl_token(&[
        "burn",
        &request.account_address,
        &request.amount.to_string(),
    ]))
}

pub async fn check_balance() -> Json<Balance> {
    let mnemonic = mnemonic_from_entropy(SERVICE_ADDRESS);
    check_balance_by_mnemonic(mnemonic).await
}

pub async fn check_destination_balance() -> Json<Balance> {
    let mnemonic = mnemonic_from_entropy(USER_ADDRESS);
    check_balance_by_mnemonic(mnemonic).await
}

async fn get_synched_wallet_from_mnemonic(
    mnemonic: GeneratedKey<Mnemonic, miniscript::Segwitv0>,
) -> (Wallet<MemoryDatabase>, ElectrumBlockchain) {
    let network = Network::Testnet; // Or this can be Network::Bitcoin, Network::Signet or Network::Regtest

    // Convert mnemonic to string
    let mnemonic_words = mnemonic.to_string();
    // Parse a mnemonic
    let mnemonic = Mnemonic::parse(&mnemonic_words).unwrap();
    // Generate the extended key
    let xkey: ExtendedKey = mnemonic.into_extended_key().unwrap();
    // Get xprv from the extended key
    let xprv = xkey.into_xprv(network).unwrap();

    // tb1qedg9fdlf8cnnqfd5mks6uz5w4kgpk2pr6y4qc7
    // let key = bitcoin::util::bip32::ExtendedPubKey::from_str("tpubDC2Qwo2TFsaNC4ju8nrUJ9mqVT3eSgdmy1yPqhgkjwmke3PRXutNGRYAUo6RCHTcVQaDR3ohNU9we59brGHuEKPvH1ags2nevW5opEE9Z5Q").unwrap();
    // let fingerprint = bitcoin::util::bip32::Fingerprint::from_str("c55b303a").unwrap();

    // Create a BDK wallet structure using BIP 84 descriptor ("m/84h/1h/0h/0" and "m/84h/1h/0h/1")
    let wallet = Wallet::new(
        // Bip84Public(key.clone(), fingerprint, KeychainKind::External),
        // Some(Bip84Public(key, fingerprint, KeychainKind::Internal)),
        Bip84(xprv, KeychainKind::External),
        Some(Bip84(xprv, KeychainKind::Internal)),
        network,
        MemoryDatabase::default(),
    )
    .unwrap();

    let client = Client::new("ssl://electrum.blockstream.info:60002").unwrap();
    let blockchain = ElectrumBlockchain::from(client);
    wallet
        .sync(
            &blockchain,
            SyncOptions {
                progress: Some(Box::new(log_progress::log_progress())),
            },
        )
        .unwrap();

    let addresses = get_known_addresses(&wallet, &blockchain);
    println!();
    addresses.iter().for_each(|s| println!("{s}"));
    println!();

    (wallet, blockchain)
}

async fn check_balance_by_mnemonic(
    mnemonic: GeneratedKey<Mnemonic, miniscript::Segwitv0>,
) -> Json<Balance> {
    let (wallet, _blockchain) = get_synched_wallet_from_mnemonic(mnemonic).await;
    let balance = wallet.get_balance().unwrap();
    Json(balance)
}

pub async fn get_new_service_address() -> Address {
    let _network = Network::Testnet; // Or this can be Network::Bitcoin, Network::Signet or Network::Regtest

    // Generate fresh mnemonic
    // tb1q6dsqge320xzu7g64d5arp4qx6ldvz6xd27zvgy
    let mnemonic = mnemonic_from_entropy(SERVICE_ADDRESS);

    let (wallet, _blockchain) = get_synched_wallet_from_mnemonic(mnemonic).await;

    let address = wallet.get_address(LastUnused).unwrap();
    address.address
}

async fn get_destination_address_0() -> Address {
    let _network = Network::Testnet; // Or this can be Network::Bitcoin, Network::Signet or Network::Regtest

    // Generate fresh mnemonic
    // tb1q76ksqtcp3ph20ne5rlduz3xvvc0ysv6chj8w3n
    let mnemonic = mnemonic_from_entropy(USER_ADDRESS);

    let (wallet, _blockchain) = get_synched_wallet_from_mnemonic(mnemonic).await;

    let address = wallet.get_address(Peek(0)).unwrap();
    address.address
}

pub async fn send_btc_to_user() -> Json<TransactionDetails> {
    // OUR: tb1q2p9nlkfkjpx68ex24uv2cau4rdjy0ft7qxwjl0, SEND_TO: tb1q76ksqtcp3ph20ne5rlduz3xvvc0ysv6chj8w3n

    // tb1q2p9nlkfkjpx68ex24uv2cau4rdjy0ft7qxwjl0
    let mnemonic = mnemonic_from_entropy(SERVICE_ADDRESS);

    let (wallet, blockchain) = get_synched_wallet_from_mnemonic(mnemonic).await;

    let (mut psbt, details) = {
        // tb1q76ksqtcp3ph20ne5rlduz3xvvc0ysv6chj8w3n
        let recipient = get_destination_address_0().await;
        assert_eq!(
            recipient.to_string().as_str(),
            "tb1q76ksqtcp3ph20ne5rlduz3xvvc0ysv6chj8w3n"
        );
        let mut builder = wallet.build_tx();
        let dust_amount = recipient.script_pubkey().dust_value();
        builder.add_recipient(recipient.script_pubkey(), dust_amount.to_sat());
        builder.finish().unwrap()
    };

    let finalized = wallet.sign(&mut psbt, SignOptions::default()).unwrap();
    assert!(finalized);

    let tx = psbt.extract_tx();
    blockchain.broadcast(&tx).unwrap();

    Json(details)
}
