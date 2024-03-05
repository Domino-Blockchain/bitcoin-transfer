use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    rc::Rc,
};

use bdk::{
    bitcoin::{bip32::ExtendedPrivKey, constants::COINBASE_MATURITY, Address, Network},
    blockchain::{ElectrumBlockchain, GetHeight},
    database::{BatchDatabase, Database, MemoryDatabase},
    template::Bip84,
    wallet::{
        AddressIndex::{self, LastUnused, New, Peek},
        AddressInfo,
    },
    Balance, Error, KeychainKind, Wallet,
};

// struct OurDB(pub Rc<RefCell<MemoryDatabase>>);

// impl BatchDatabase for OurDB {
//     type Batch = OurDB;

//     fn begin_batch(&self) -> Self::Batch {
//         todo!()
//     }

//     fn commit_batch(&mut self, batch: Self::Batch) -> Result<(), Error> {
//         todo!()
//     }
// }

// pub fn new_wallet(xprv: ExtendedPrivKey, network: Network) -> Wallet<MemoryDatabase> {
//     let database = Rc::new(RefCell::new(MemoryDatabase::default()));
//     let wallet = Wallet::new(
//         // Bip84Public(key.clone(), fingerprint, KeychainKind::External),
//         // Some(Bip84Public(key, fingerprint, KeychainKind::Internal)),
//         Bip84(xprv, KeychainKind::External),
//         Some(Bip84(xprv, KeychainKind::Internal)),
//         network,
//         Rc::clone(&database),
//     )
//     .unwrap();
//     wallet
// }

pub fn get_balance_by_address(
    wallet: &Wallet<MemoryDatabase>,
    last_sync_height: u32,
) -> Result<Vec<(Address, Balance)>, Error> {
    let mut data: HashMap<Address, Balance> = HashMap::new();
    let mut addresses = Vec::new();

    let utxos = wallet.list_unspent()?;
    for u in utxos {
        let address = Address::from_script(&u.txout.script_pubkey, wallet.network()).unwrap();
        let balance = data.entry(address.clone()).or_default();
        if !addresses.contains(&address) {
            addresses.push(address);
        }

        // Unwrap used since utxo set is created from database
        let tx = wallet
            .get_tx(&u.outpoint.txid, true)?
            .expect("Transaction not found in database");
        if let Some(tx_conf_time) = &tx.confirmation_time {
            if tx.transaction.expect("No transaction").is_coin_base()
                && (last_sync_height - tx_conf_time.height) < COINBASE_MATURITY
            {
                balance.immature += u.txout.value;
            } else {
                balance.confirmed += u.txout.value;
            }
        } else if u.keychain == KeychainKind::Internal {
            balance.trusted_pending += u.txout.value;
        } else {
            balance.untrusted_pending += u.txout.value;
        }
    }

    Ok(addresses
        .into_iter()
        .map(|address| {
            let balance = data.remove(&address).unwrap();
            (address, balance)
        })
        .collect())
}

pub fn get_known_addresses(
    wallet: &Wallet<MemoryDatabase>,
    blockchain: &ElectrumBlockchain,
) -> Vec<String> {
    let last_sync_height = blockchain.get_height().unwrap();
    let balances: Vec<_> = get_balance_by_address(&wallet, last_sync_height)
        .unwrap()
        .into_iter()
        .map(|(a, b)| (a.to_string(), b.get_total()))
        .collect();

    let address = wallet.get_address(LastUnused).unwrap();
    let fmt_addr = |a: AddressInfo| {
        let addr = a.to_string();
        format!(
            "{}:{}:{}:{}",
            std::str::from_utf8(a.keychain.as_ref()).unwrap(),
            a.index,
            addr,
            balances
                .iter()
                .find_map(|(a, b)| (a == &addr).then_some(*b))
                .unwrap_or_default()
        )
    };
    let mut addresses: Vec<_> = (0..=address.index)
        .into_iter()
        .map(|index| wallet.get_address(Peek(index)).unwrap())
        .map(fmt_addr)
        .collect();
    let internal_address = wallet.get_internal_address(LastUnused).unwrap();
    let internal_addresses: Vec<_> = (0..=internal_address.index)
        .into_iter()
        .map(|index| wallet.get_internal_address(Peek(index)).unwrap())
        .map(fmt_addr)
        .collect();

    addresses.extend(internal_addresses);

    addresses
}
