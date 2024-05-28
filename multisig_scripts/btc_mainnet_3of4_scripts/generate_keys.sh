#!/bin/bash

set -e -o pipefail
source "$(dirname "$(realpath "$0")")/check_dependencies.sh"

# 00 Local
export XPRV_00=$(bdk-cli --network bitcoin key generate | jq -r '.xprv')

# 01 AWS KMS
# export XPRV_01=$(bdk-cli --network bitcoin key generate | jq -r '.xprv')

# 02 Hardcoded/Ledger
export XPRV_02=$(bdk-cli --network bitcoin key generate | jq -r '.xprv')

# 03 Google KMS
# export XPRV_03=$(bdk-cli --network bitcoin key generate | jq -r '.xprv')


env | grep XPRV

echo $XPRV_00 > xpriv_00.json
echo $XPRV_02 > xpriv_02.json

echo DONE
