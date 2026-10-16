#!/usr/bin/env bash
# Recover a test key, rewind public state, and prove Core rejects the stale spend.
set -euo pipefail
root=${LAYERTREE_ROLLBACK_ROOT:?set a persistent scratch evidence directory}
mkdir -p "$root"
root=$(cd "$root" && pwd)
name="layertree-key-rollback-$$"
image=bitcoin/bitcoin:30.2@sha256:2b1d8a8d23c67b426afa8cd5b3ffd66b8c4ebbbceef3e214df8627aaead1f517
cleanup() { docker rm -fv "$name" >/dev/null 2>&1 || true; }
trap cleanup EXIT
docker run -d --name "$name" -p 127.0.0.1::18443 "$image" -regtest -server -fallbackfee=0.0002 \
  -rpcbind=0.0.0.0 -rpcallowip=0.0.0.0/0 -rpcuser=regtest -rpcpassword=regtest-only >/dev/null
port=$(docker port "$name" 18443/tcp | head -n1 | sed 's/.*://')
LAYERTREE_REGTEST_RPC="127.0.0.1:$port" LAYERTREE_SEAL_ROLLBACK_EVIDENCE="$root/rollback.json" \
  cargo test --locked --manifest-path crates/layertree-tee-lab/Cargo.toml --features gcp-sealing --lib \
    stale_sealed_key_cannot_spend_the_consumed_tree_output -- --ignored --nocapture
