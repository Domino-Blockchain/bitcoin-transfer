#!/bin/bash

set -o errexit
set -o pipefail

trap 'kill $BGPID; pkill -P $$; exit' SIGINT SIGTERM

echo "Building"
n="0"

cargo b --bin bitcoin_transfer &
((n+=1))
cargo b --bin client &
((n+=1))

for i in $(seq 1 $n);
    do wait -n;
done
n="0"
# Building

./target/debug/bitcoin_transfer &
BGPID=$!

wait_for_port.sh 3000
./target/debug/client get_address || { kill $BGPID; pkill -P $$; }

kill $BGPID
pkill -P $$ || true
