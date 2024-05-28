#!/bin/bash

set -e -o pipefail
source "$(dirname "$(realpath "$0")")/check_dependencies.sh"

export MULTI_DESCRIPTOR_03=$(cat multi_descriptor_03.json)
export SECONDSIG_PSBT=$(cat secondsig_psbt.json)
export KEY_NAME="projects/domichain-archive/locations/global/keyRings/TestKeyring/cryptoKeys/TestKey1/cryptoKeyVersions/1"

export JSON_OUTPUT=$(~/bitcoin-transfer/multisig_scripts/bdk-cli/target/release/bdk-cli --network bitcoin wallet --google_kms $KEY_NAME --wallet wallet_name_msd03 --descriptor $MULTI_DESCRIPTOR_03 sign --psbt $SECONDSIG_PSBT)
echo "Is finalized: `echo $JSON_OUTPUT | jq -r '.is_finalized'`"
export THIRDSIG_PSBT=$(echo $JSON_OUTPUT | jq -r '.psbt')

if [ "$SECONDSIG_PSBT" = "$THIRDSIG_PSBT" ]; then
  echo "ERROR: Thirdsig don't change PSBT"
  exit 1
fi

env | grep THIRDSIG_PSBT
echo

echo $THIRDSIG_PSBT > thirdsig_psbt.json
