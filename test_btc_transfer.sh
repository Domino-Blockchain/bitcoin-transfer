#!/bin/bash

set -o errexit
set -o pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

trap 'pkill -P $$; exit' SIGINT SIGTERM


echo "Building"
n="0"

cd .. # Common dir

cd ./domichain
cargo build --release &
((n+=1))
cd -

cd ./domichain-program-library
cargo build --release --manifest-path token/cli/Cargo.toml --target-dir target_0 & # CLI
((n+=1))
cargo wasi build --release --manifest-path token/program/Cargo.toml --target-dir target_1 & # Token
((n+=1))
cargo wasi build --release --manifest-path token/program-2022/Cargo.toml --target-dir target_2 & # Token2022
((n+=1))
cargo wasi build --release --manifest-path token/program-btci/Cargo.toml --target-dir target_3 & # Token BTCi
((n+=1))
cargo wasi build --release --manifest-path associated-token-account/program/Cargo.toml --target-dir target_4 & # Associated Token
((n+=1))
cargo wasi build --release --manifest-path token-swap/program/Cargo.toml --target-dir target_5 & # Token swap
((n+=1))
cd -

cd ./bitcoin_transfer
cargo build --release &
((n+=1))
cd -

for i in $(seq 1 $n);
    do wait -n;
done
n="0"

# Building


echo "Copying"

cp ./domichain-program-library/target_1/wasm32-wasi/release/spl_token.wasm ./domichain/spl_token-4.0.0.wasm &
cp ./domichain-program-library/target_2/wasm32-wasi/release/spl_token_2022.wasm ./domichain/spl_token-2022-0.6.1.wasm &
cp ./domichain-program-library/target_3/wasm32-wasi/release/spl_token_btci.wasm ./domichain/spl_token-btci-4.0.0.wasm &
cp ./domichain-program-library/target_4/wasm32-wasi/release/spl_associated_token_account.wasm ./domichain/spl_associated-token-account-1.0.5.wasm &
cp ./domichain-program-library/target_5/wasm32-wasi/release/spl_token_swap.wasm ./domichain/spl_token-swap-3.0.0.wasm &

wait # Copying


echo "wasm-strip"

for i in ./domichain/spl_*.wasm;
    do echo $i ; wasm-strip $i ;
done

wait # wasm-strip