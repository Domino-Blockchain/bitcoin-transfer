#!/bin/bash

export XPRV_00=$(bdk-cli key generate | jq -r '.xprv')
export XPRV_01=$(bdk-cli key generate | jq -r '.xprv')
export XPRV_02=$(bdk-cli key generate | jq -r '.xprv')

env | grep XPRV

export XPUB_00=$(bdk-cli key derive --xprv $XPRV_00 --path "m/84'/1'/0'/0" | jq -r ".xpub")
export XPUB_01=$(bdk-cli key derive --xprv $XPRV_01 --path "m/84'/1'/0'/0" | jq -r ".xpub")
export XPUB_02=$(bdk-cli key derive --xprv $XPRV_02 --path "m/84'/1'/0'/0" | jq -r ".xpub")

env | grep XPUB

export DESCRIPTOR_00="$XPRV_00/84h/1h/0h/0/*"
export DESCRIPTOR_01="$XPRV_01/84h/1h/0h/0/*"
export DESCRIPTOR_02="$XPRV_02/84h/1h/0h/0/*"

export MULTI_DESCRIPTOR_00=$(bdk-cli compile "thresh(2,pk($DESCRIPTOR_00),pk($XPUB_01),pk($XPUB_02))" | jq -r '.descriptor')
export MULTI_DESCRIPTOR_01=$(bdk-cli compile "thresh(2,pk($XPUB_00),pk($DESCRIPTOR_01),pk($XPUB_02))" | jq -r '.descriptor')
export MULTI_DESCRIPTOR_02=$(bdk-cli compile "thresh(2,pk($XPUB_00),pk($XPUB_01),pk($DESCRIPTOR_02))" | jq -r '.descriptor')

env | grep MULTI

bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_new_address
bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 get_new_address
bdk-cli wallet --wallet wallet_name_msd02 --descriptor $MULTI_DESCRIPTOR_02 get_new_address
# assert all addresses are the same

# send BTC

bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sync
bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_balance
bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 policies

# to send TX
export UNSIGNED_PSBT=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 create_tx --send_all --to mkHS9ne12qx9pS9VojpwU5xtRd4T7X7ZUt:0 --external_policy "{\"CHANGE_ID_HERE\": [0,1]}" | jq -r '.psbt')

env | grep UNSIGNED

bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sign --psbt $UNSIGNED_PSBT
export ONESIG_PSBT=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sign --psbt $UNSIGNED_PSBT | jq -r '.psbt')

env | grep ONESIG


bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 sign --psbt $ONESIG_PSBT
export SECONDSIG_PSBT=$(bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 sign --psbt $ONESIG_PSBT | jq -r '.psbt')

env | grep SECONDSIG

# broadcast
bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 broadcast --psbt $SECONDSIG_PSBT

# Check https://mempool.space/testnet
