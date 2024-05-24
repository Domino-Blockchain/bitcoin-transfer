#!/bin/bash

source "$(dirname "$(realpath "$0")")/check_dependencies.sh"

export XPRV_00=$(bdk-cli --network bitcoin key generate | jq -r '.xprv')
export XPRV_01=$(bdk-cli --network bitcoin key generate | jq -r '.xprv')
export XPRV_02=$(bdk-cli --network bitcoin key generate | jq -r '.xprv')

env | grep XPRV

echo $XPRV_00 > xpriv_00.json
echo $XPRV_01 > xpriv_01.json
echo $XPRV_02 > xpriv_02.json

echo DONE
