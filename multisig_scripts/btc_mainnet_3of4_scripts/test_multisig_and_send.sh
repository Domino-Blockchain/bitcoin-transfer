#!/bin/bash

source "$(dirname "$(realpath "$0")")/check_dependencies.sh"

# send BTC

export MULTI_DESCRIPTOR_00=$(cat multi_descriptor_00.json)
export MULTI_DESCRIPTOR_01=$(cat multi_descriptor_01.json)
export MULTI_DESCRIPTOR_02=$(cat multi_descriptor_02.json)

bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sync
bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_balance | jq
echo
export CHANGE_ID=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 policies | jq -r ".external.id")

# to send TX
export TO_ADDRESS="tb1qjk7wqccmetsngh9e0zff73rhsqny568g5fs758"
export UNSIGNED_PSBT=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 create_tx --send_all --to $TO_ADDRESS:0 --external_policy "{\"$CHANGE_ID\": [0,1]}" | jq -r '.psbt')

env | grep UNSIGNED
echo

bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sign --psbt $UNSIGNED_PSBT
export ONESIG_PSBT=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sign --psbt $UNSIGNED_PSBT | jq -r '.psbt')

env | grep ONESIG
echo

bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 sign --psbt $ONESIG_PSBT
export SECONDSIG_PSBT=$(bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 sign --psbt $ONESIG_PSBT | jq -r '.psbt')

env | grep SECONDSIG
echo

# broadcast
export TX_ID=$(bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 broadcast --psbt $SECONDSIG_PSBT)
echo $TX_ID

echo "Check: https://mempool.space/testnet/tx/$(echo $TX_ID | jq -r ".txid")"
