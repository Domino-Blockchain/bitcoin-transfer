# Bitcoin transfer

## Install dependencies

- Install Rust
- Install AWS CLI
- Install Google Cloud CLI (gcloud)
- Install [libmongocrypt](https://www.mongodb.com/docs/manual/core/csfle/reference/libmongocrypt/)
- Download `domichain-program-library` and compile `spl-token` CLI

- Install BDK CLI:
```sh
# Install the BDK CLI (default case)
cargo install bdk-cli --features compiler,electrum
# OR if you need Esplora
cargo install bdk-cli --features compiler,esplora-ureq 
```

- Compile `bdk-cli` with KMS support:
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

## Setup environment variables

```sh
cp .env.example .env
# Update variables in .env file
```

## Create keys

```sh
# Edit `aws_kms_policy.json` file with admin user instead `user@company.com`
python create_aws_keys.py
# Create Google KMS keys
python create_google_keys.py

# Get info about BTC KMS keys
python get_aws_keys.py > aws_kms_keys.json
python3 get_google_keys.py > google_kms_keys.json

# setup mongodb key
cargo run --bin generate_master_key
```

## Start the server

```sh
# btc_server
cargo run --release --bin bitcoin_transfer >> ~/btc_logs.txt
```

- [API docs](https://github.com/Domino-Blockchain/bitcoin-transfer/blob/main/docs/API.md)

Verify owner balance:
```sh
spl-token \
    --url http://108.48.39.243:8899 \
    --program-id BTCi9FUjBVY3BSaqjzfhEPKVExuvarj8Gtfn4rJ5soLC \
    accounts \
    --owner 5PCWRXtMhen9ipbq4QeeAuDgFymGachUf7ozA3NJwHDJ
```

## Dump of MongoDB

```sh
mongodump --uri="mongodb://localhost:27017"
```

## AWS KMS keys managment

```sh
# List all keys
aws kms list-aliases --query "Aliases[?contains(@.AliasName,'btci_multisig_')]"
# Key ID to Key ARN mapping
aws kms list-keys --query "Keys" | jq "map({ (.KeyId): .KeyArn }) | add"
```
