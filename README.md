# Bitcoin transfer

```sh
curl http://0.0.0.0:3000/get_address
curl -X POST http://0.0.0.0:3000/check_balance | jq
curl -X POST http://0.0.0.0:3000/mint_token | jq
```


TODO:

- include spl-token mint as Rust dep
- Write test for sending BTC


- Add DB for user mint requests, track all transfers/mints locally
- Add an entry point for users to get their bitcoins back
- Automate multi-sig BTC TX signing


- Install libmongocrypt https://www.mongodb.com/docs/manual/core/csfle/reference/libmongocrypt/
```sh
# Install the BDK CLI
cargo install bdk-cli --features compiler,electrum
# OR
cargo install bdk-cli --features compiler,esplora-ureq

# Create AWS KMS keys
# Edit `aws_kms_policy.json` file with admin user instead `user@company.com`
python create_aws_keys.py

# Get info about BTC KMS keys
python get_aws_keys.py > kms_keys.json

# setup mongodb key
cargo run --bin generate_master_key

# btc_ui
cd bitcoin_bridge_repos/unisat-dev-support/brc20-swap-demo
npm run start

# setup config
# DOMI Testnet URL: http://103.106.59.69:8899

# btc_server
cargo run -r --bin bitcoin_transfer

# db: btc
# collection: keys

# Verify owner balance:
spl-token \
    --url http://108.48.39.243:8899 \
    --program-id BTCi9FUjBVY3BSaqjzfhEPKVExuvarj8Gtfn4rJ5soLC \
    accounts \
    --owner 5PCWRXtMhen9ipbq4QeeAuDgFymGachUf7ozA3NJwHDJ
```

AWS KMS keys managment:
```
# List all keys
aws kms list-aliases --query "Aliases[?contains(@.AliasName,'btci_multisig_')]"
# Key ID to Key ARN mapping
aws kms list-keys --query "Keys" | jq "map({ (.KeyId): .KeyArn }) | add"
```