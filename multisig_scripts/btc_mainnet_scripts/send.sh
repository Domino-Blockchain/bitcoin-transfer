#!/bin/bash

source "$(dirname "$(realpath "$0")")/check_dependencies.sh"

export MULTI_DESCRIPTOR_01=$(cat multi_descriptor_01.json)
export SECONDSIG_PSBT=$(cat secondsig_psbt.json)

# broadcast
export TX_ID=$(bdk-cli --network bitcoin wallet --server https://blockstream.info/api --stop_gap 1 --timeout 15 --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 broadcast --psbt $SECONDSIG_PSBT)
echo $TX_ID

echo "Check: https://mempool.space/testnet/tx/$(echo $TX_ID | jq -r ".txid")"
