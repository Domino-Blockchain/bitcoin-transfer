# Bitcoin transfer

## Install dependencies

- Install Rust
- Install [AWS CLI](/docs/AWS%20CLI%20setup.md)
- Install [Google Cloud CLI](/docs/Google%20CLI%20setup.md) (gcloud)
- Setup MongoDB
- Install [libmongocrypt](/docs/libmongocrypt%20setup.md)
- Download `domichain-program-library` and compile `spl-token` CLI
- Download `domichain` and place into `../domichain` path
- Get some DOMI in the wallet. (`domichain account` to get address)

### Install `libsqlite3-dev`
```sh
# Fixes: /usr/bin/ld: cannot find -lsqlite3
sudo apt-get install libsqlite3-dev
```

### Install BDK CLI:
```sh
# Install the BDK CLI (default case)
cargo install bdk-cli --features compiler,electrum
# OR if you need Esplora
cargo install bdk-cli --features compiler,esplora-ureq 
```

### Compile `bdk-cli` with KMS support:
```sh
pushd multisig_scripts

git clone git@github.com:Domino-Blockchain/rust-secp256k1.git
git clone git@github.com:Domino-Blockchain/rust-miniscript.git
git clone git@github.com:Domino-Blockchain/rust-bitcoin.git
git clone git@github.com:Domino-Blockchain/bitcoindevkit-bdk-cli.git bdk-cli
git clone git@github.com:Domino-Blockchain/bitcoindevkit-bdk.git bdk

pushd bdk-cli
cargo build --release
popd

popd
```

## Get Google Cloud Service Account key

https://cloud.google.com/iam/docs/keys-create-delete

```sh
mkdir ~/google_cloud_keys
mv service_key.json ~/google_cloud_keys/service_key.json
```

## Setup environment variables

```sh
cp .env.example .env
# Update variables in .env file
```

## Create keys

```sh
# Edit `aws_kms_policy.json` file with admin user instead `user@company.com`
python3 create_aws_keys.py
# Create Google KMS keys
python3 create_google_keys.py

# Get info about BTC KMS keys
python3 get_aws_keys.py > aws_kms_keys.json
python3 get_google_keys.py > google_kms_keys.json

# setup mongodb key
cargo run --bin generate_master_key

# Create ledger_keys.json
# The JSON file represents hardware ledger to multisig
# It should contain extended public keys of ledger
cat ledger_keys.json
# {"bitcoin": {"xpub": "..."}, "testnet": {"xpub": "..."}}
```

## Start the server

```sh
cargo run --release --bin bitcoin_transfer >> ~/btc_logs.txt 2>&1
```

- [API docs](https://github.com/Domino-Blockchain/bitcoin-transfer/blob/main/docs/API.md)
```sh
~/domichain/target/release/domichain address
# AHVhj6a5XVKKB3Es6gyWFd4ZqAS5V4LZZzoGqs182f9c

~/domichain/target/release/domichain \
    -u https://api.testnet.domichain.io \
    balance
# N DOMI

~/domichain-program-library/target/release/spl-token \
    -u https://api.testnet.domichain.io \
    --output json \
    accounts
# {"accounts": []}

# Get new multisig address
curl -vv -s -H 'Content-Type: application/json' \
    -d '{ "domi_address":"AHVhj6a5XVKKB3Es6gyWFd4ZqAS5V4LZZzoGqs182f9c"}' \
    https://btc-transfer.domichain.io/get_address_from_db
```

Verify owner balance:
```sh
~/domichain-program-library/target/release/spl-token \
    --url https://api.testnet.domichain.io \
    --program-id BTCi9FUjBVY3BSaqjzfhEPKVExuvarj8Gtfn4rJ5soLC \
    accounts \
    --owner 5PCWRXtMhen9ipbq4QeeAuDgFymGachUf7ozA3NJwHDJ
```

## Backup items

See [this doc](/docs/Backup%20items.md)

## Dump of MongoDB

```sh
mongodump --uri="mongodb://localhost:27017"
```

## AWS KMS keys managment

```sh
# Describe all keys
aws kms list-aliases --query "Aliases[?contains(@.AliasName,'btci_multisig_')]"
# Get Key ID to Key ARN mapping
aws kms list-keys --query "Keys" | jq "map({ (.KeyId): .KeyArn }) | add"
```
