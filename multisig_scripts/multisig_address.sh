#!/bin/bash

source "$(dirname "$(realpath "$0")")/check_dependencies.sh"

# In case of error drob DB: rm -rf ~/.bdk-bitcoin/wallet_name_msd0*
export XPRV_00=$(cat xpriv_00.json)
# export XPRV_01=$(cat xpriv_01.json)
export XPRV_02=$(cat xpriv_02.json)

env | grep XPRV
echo

export XPUB_00=$(bdk-cli key derive --xprv $XPRV_00 --path "m/84'/1'/0'/0" | jq -r ".xpub")
# export XPUB_01=$(bdk-cli key derive --xprv $XPRV_01 --path "m/84'/1'/0'/0" | jq -r ".xpub")

# https://iancoleman.io/bitcoin-key-compression/ uncompressed and compressed public keys
# 04002c5c77d7951eaa1818a7b409181b2e4a81e93e6eb44c6fe92c637c492725bb9cf195906695937f974446cd6b602c7164158928e5bd43e808a06d831d35a1d8
# 02002c5c77d7951eaa1818a7b409181b2e4a81e93e6eb44c6fe92c637c492725bb
export XPUB_01=$(cat pub_kms_01.json)
export XPUB_02=$(bdk-cli key derive --xprv $XPRV_02 --path "m/84'/1'/0'/0" | jq -r ".xpub")

env | grep XPUB
echo

export DESCRIPTOR_00="$XPRV_00/84h/1h/0h/0/*"
# export DESCRIPTOR_01="$XPRV_01/84h/1h/0h/0/*"
export DESCRIPTOR_02="$XPRV_02/84h/1h/0h/0/*"

export MULTI_DESCRIPTOR_00=$(bdk-cli compile "thresh(2,pk($DESCRIPTOR_00),pk($XPUB_01),pk($XPUB_02))" | jq -r '.descriptor')
export MULTI_DESCRIPTOR_01=$(bdk-cli compile "thresh(2,pk($XPUB_00),pk($XPUB_01),pk($XPUB_02))" | jq -r '.descriptor')
export MULTI_DESCRIPTOR_02=$(bdk-cli compile "thresh(2,pk($XPUB_00),pk($XPUB_01),pk($DESCRIPTOR_02))" | jq -r '.descriptor')

env | grep MULTI
echo

# Clean cache and start with first address each time
rm -rf ~/.bdk-bitcoin/{wallet_name_msd00,wallet_name_msd01,wallet_name_msd02}

# `get_new_address` will get new address each time unless we delete wallet
export MULTI_ADDRESS=$(bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_new_address)
export MULTI_ADDRESS_COPY_2=$(bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 get_new_address)
export MULTI_ADDRESS_COPY_3=$(bdk-cli wallet --wallet wallet_name_msd02 --descriptor $MULTI_DESCRIPTOR_02 get_new_address)

echo $MULTI_ADDRESS | jq
echo $MULTI_ADDRESS_COPY_2 | jq
echo $MULTI_ADDRESS_COPY_3 | jq
# assert all three addresses are the same

echo $MULTI_DESCRIPTOR_00 > multi_descriptor_00.json
echo $MULTI_DESCRIPTOR_01 > multi_descriptor_01.json
echo $MULTI_DESCRIPTOR_02 > multi_descriptor_02.json

bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sync
bdk-cli wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_balance | jq

# Amount to send
# 0.00001000

# AWS multisig
# tb1qqul9d6uwrc28h92kut6d7a5nyaqxgpyk5x2yw333ezzrnmju2q2sz30mgt
