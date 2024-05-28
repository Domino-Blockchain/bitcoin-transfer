#!/bin/bash

set -e -o pipefail
source "$(dirname "$(realpath "$0")")/check_dependencies.sh"

# In case of error drop DB: rm -rf ~/.bdk-bitcoin/wallet_name_msd0*
export XPRV_00=$(cat xpriv_00.json)
export XPRV_02=$(cat xpriv_02.json)

env | grep XPRV
echo

export XPUB_00=$(bdk-cli --network bitcoin key derive --xprv $XPRV_00 --path "m/84'/1'/0'/0" | jq -r ".xpub")

# https://iancoleman.io/bitcoin-key-compression/ uncompressed and compressed public keys
# 04002c5c77d7951eaa1818a7b409181b2e4a81e93e6eb44c6fe92c637c492725bb9cf195906695937f974446cd6b602c7164158928e5bd43e808a06d831d35a1d8
# 02002c5c77d7951eaa1818a7b409181b2e4a81e93e6eb44c6fe92c637c492725bb
export XPUB_01=$(cat pub_aws_kms_01.json)

export XPUB_02=$(bdk-cli --network bitcoin key derive --xprv $XPRV_02 --path "m/84'/1'/0'/0" | jq -r ".xpub")

export XPUB_03=$(cat pub_google_kms_03.json)

env | grep XPUB
echo

export DESCRIPTOR_00="$XPRV_00/84h/1h/0h/0/*"
export DESCRIPTOR_02="$XPRV_02/84h/1h/0h/0/*"

export MULTI_DESCRIPTOR_00=$(bdk-cli --network bitcoin compile "thresh(3,pk($DESCRIPTOR_00),pk($XPUB_01),pk($XPUB_02),pk($XPUB_03))" | jq -r '.descriptor')
export MULTI_DESCRIPTOR_01=$(bdk-cli --network bitcoin compile "thresh(3,pk($XPUB_00),pk($XPUB_01),pk($XPUB_02),pk($XPUB_03))" | jq -r '.descriptor')
export MULTI_DESCRIPTOR_02=$(bdk-cli --network bitcoin compile "thresh(3,pk($XPUB_00),pk($XPUB_01),pk($DESCRIPTOR_02),pk($XPUB_03))" | jq -r '.descriptor')
export MULTI_DESCRIPTOR_03=$(bdk-cli --network bitcoin compile "thresh(3,pk($XPUB_00),pk($XPUB_01),pk($XPUB_02),pk($XPUB_03))" | jq -r '.descriptor')

env | grep MULTI
echo

# Clean cache and start with first address each time
rm -rf ~/.bdk-bitcoin/{wallet_name_msd00,wallet_name_msd01,wallet_name_msd02,wallet_name_msd03}

# `get_new_address` will get new address each time unless we delete wallet
export MULTI_ADDRESS_00=$(bdk-cli --network bitcoin wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_new_address)
export MULTI_ADDRESS_01=$(bdk-cli --network bitcoin wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 get_new_address)
export MULTI_ADDRESS_02=$(bdk-cli --network bitcoin wallet --wallet wallet_name_msd02 --descriptor $MULTI_DESCRIPTOR_02 get_new_address)
export MULTI_ADDRESS_03=$(bdk-cli --network bitcoin wallet --wallet wallet_name_msd03 --descriptor $MULTI_DESCRIPTOR_03 get_new_address)

echo $MULTI_ADDRESS_00 | jq
echo $MULTI_ADDRESS_01 | jq
echo $MULTI_ADDRESS_02 | jq
echo $MULTI_ADDRESS_03 | jq
# assert all three addresses are the same

echo $MULTI_DESCRIPTOR_00 > multi_descriptor_00.json
echo $MULTI_DESCRIPTOR_01 > multi_descriptor_01.json
echo $MULTI_DESCRIPTOR_02 > multi_descriptor_02.json
echo $MULTI_DESCRIPTOR_03 > multi_descriptor_03.json

echo "Syncing balance..."
bdk-cli --network bitcoin wallet --server https://blockstream.info/api --stop_gap 1 --timeout 15 --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 sync | jq
bdk-cli --network bitcoin wallet --wallet wallet_name_msd00 --descriptor $MULTI_DESCRIPTOR_00 get_balance | jq

# Amount to send
# 0.00001000

# AWS/Google multisig
# bc1qdmj02mywvvm4z53pak6rkwne2jgh08zsdun44tv920dxl30k7m6sfkgdkd

echo DONE
