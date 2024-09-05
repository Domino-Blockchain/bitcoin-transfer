#!/bib/bash

set -e -o pipefail

echo "Starting BTC Bridge service"
./target/release/bitcoin_transfer --skip-catchup >> ~/btc_logs.txt 2>&1
