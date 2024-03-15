#!/bin/bash

source "$(dirname "$(realpath "$0")")/check_dependencies.sh"

export MULTI_DESCRIPTOR_01=$(cat multi_descriptor_01.json)
export ONESIG_PSBT=$(cat onesig_psbt.json)

bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 sign --psbt $ONESIG_PSBT
export SECONDSIG_PSBT=$(bdk-cli wallet --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 sign --psbt $ONESIG_PSBT | jq -r '.psbt')

if [ "$ONESIG_PSBT" = "$SECONDSIG_PSBT" ]; then
  echo "ERROR: Secondsig don't change PSBT"
  exit 1
fi

env | grep SECONDSIG
echo

echo $SECONDSIG_PSBT > secondsig_psbt.json
