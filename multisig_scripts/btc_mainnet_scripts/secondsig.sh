#!/bin/bash

source "$(dirname "$(realpath "$0")")/check_dependencies.sh"

export MULTI_DESCRIPTOR_01=$(cat multi_descriptor_01.json)
export ONESIG_PSBT=$(cat onesig_psbt.json)
export KEY_ARN="arn:aws:kms:us-east-2:571922870935:key/17be5d9e-d752-4350-bbc1-68993fa25a4f"

export JSON_OUTPUT=$(~/bitcoin-transfer/multisig_scripts/bdk-cli/target/release/bdk-cli --network bitcoin wallet --aws_kms $KEY_ARN --wallet wallet_name_msd01 --descriptor $MULTI_DESCRIPTOR_01 sign --psbt $ONESIG_PSBT)
echo "Is finalized: `echo $JSON_OUTPUT | jq -r '.is_finalized'`"
export SECONDSIG_PSBT=$(echo $JSON_OUTPUT | jq -r '.psbt')

if [ "$ONESIG_PSBT" = "$SECONDSIG_PSBT" ]; then
  echo "ERROR: Secondsig don't change PSBT"
  exit 1
fi

env | grep SECONDSIG
echo

echo $SECONDSIG_PSBT > secondsig_psbt.json
