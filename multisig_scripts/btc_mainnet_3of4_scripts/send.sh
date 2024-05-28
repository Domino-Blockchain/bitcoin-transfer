#!/bin/bash

set -e -o pipefail
source "$(dirname "$(realpath "$0")")/check_dependencies.sh"

export MULTI_DESCRIPTOR_01=$(cat multi_descriptor_01.json)
export THIRDSIG_PSBT=$(cat thirdsig_psbt.json)

# broadcast
export TX_ID=$(bdk-cli --network bitcoin wallet --server https://blockstream.info/api --stop_gap 1 --timeout 15 --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 broadcast --psbt $THIRDSIG_PSBT)
echo $TX_ID

echo "Check: https://mempool.space/testnet/tx/$(echo $TX_ID | jq -r ".txid")"
