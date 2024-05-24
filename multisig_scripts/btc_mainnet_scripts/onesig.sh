#!/bin/bash

source "$(dirname "$(realpath "$0")")/check_dependencies.sh"

export MULTI_DESCRIPTOR_00=$(cat multi_descriptor_00.json)

bdk-cli --network bitcoin wallet --server https://blockstream.info/api --stop_gap 1 --timeout 15 --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sync | jq
bdk-cli --network bitcoin wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_balance | jq
echo
export CHANGE_ID=$(bdk-cli --network bitcoin wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 policies | jq -r ".external.id")

# to send TX
export TO_ADDRESS="bc1qzn2c04kd7zrk0csn8z906jexakgs79gf3qf7m2" # Electrum Wallet address
export AMOUNT="550" # More than dust limit
echo "Sending $AMOUNT sat to $TO_ADDRESS"
export JSON_OUTPUT=$(bdk-cli --network bitcoin wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 create_tx --to $TO_ADDRESS:$AMOUNT --external_policy "{\"$CHANGE_ID\": [0,1]}")
echo "Fee is `echo $JSON_OUTPUT | jq -r '.details.fee'`"
echo
export UNSIGNED_PSBT=$(echo $JSON_OUTPUT | jq -r '.psbt')

env | grep UNSIGNED
echo

export ONESIG_PSBT=$(bdk-cli --network bitcoin wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sign --psbt $UNSIGNED_PSBT | jq -r '.psbt')

env | grep ONESIG
echo

echo $ONESIG_PSBT > onesig_psbt.json
