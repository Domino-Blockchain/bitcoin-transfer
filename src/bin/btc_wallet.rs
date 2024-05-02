use std::time::Instant;

use bdk::bitcoin::Network;
use bdk::blockchain::ElectrumBlockchain;
use bdk::database::MemoryDatabase;
use bdk::electrum_client::Client;
use bdk::keys::{
    bip39::{Language, Mnemonic, WordCount},
    DerivableKey, ExtendedKey, GeneratableKey, GeneratedKey,
};
use bdk::template::Bip84;
use bdk::wallet::AddressIndex::New;
use bdk::{miniscript, KeychainKind, SyncOptions, Wallet};

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

fn mnemonic_from_entropy(entropy: [u8; 32]) -> GeneratedKey<Mnemonic, miniscript::Segwitv0> {
    Mnemonic::generate_with_entropy((WordCount::Words12, Language::English), entropy).unwrap()
}

fn main() {
    let network = Network::Testnet; // Or this can be Network::Bitcoin, Network::Signet or Network::Regtest

    // Generate fresh mnemonic
    let mnemonic = mnemonic_from_entropy(SERVICE_ADDRESS);
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

    let wallet_sync = Instant::now();
    wallet.sync(&blockchain, SyncOptions::default()).unwrap();
    dbg!(wallet_sync.elapsed());

    println!(
        "mnemonic: {}\n\nrecv desc (pub key): {:#?}\n\nchng desc (pub key): {:#?}",
        mnemonic_words,
        wallet
            .get_descriptor_for_keychain(KeychainKind::External)
            .to_string(),
        wallet
            .get_descriptor_for_keychain(KeychainKind::Internal)
            .to_string()
    );

    println!("Address #0: {}", wallet.get_address(New).unwrap());
    println!("Descriptor balance: {} SAT", wallet.get_balance().unwrap());
}
