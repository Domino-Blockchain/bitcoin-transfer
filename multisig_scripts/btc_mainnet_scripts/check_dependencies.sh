#!/bin/bash

set -e -o pipefail

# sudo apt-get install jq
jq --version || { echo "'jq' is not installed"; exit 1; }

# sudo apt-get install libsqlite3-dev
ld -lsqlite3 -o /dev/null 2> /dev/null || { echo "package 'libsqlite3-dev' is not installed; required for bdk-cli"; echo "run 'sudo apt-get install libsqlite3-dev'"; exit 1; }

# Install bdk-cli in case we dont have it
bdk-cli --version || cargo install bdk-cli --features=compiler,electrum

export RUSTFLAGS="-Awarnings" # Remove warnings
cargo build --release --manifest-path ~/bitcoin-transfer/Cargo.toml
cargo build --release --manifest-path ~/bitcoin-transfer/multisig_scripts/bdk-cli/Cargo.toml
