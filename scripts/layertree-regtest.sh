#!/usr/bin/env bash
# LayerTree P0 (#11781): run the lab's transitions and a timed unroll on a
# real bitcoind (regtest, Docker), with the enclave gone for the unroll.
#
# Needs Docker. Uses the host's cargo when present; otherwise builds and
# runs the harness in a pinned Rust image. Everything lives in one scratch
# root (LAYERTREE_REGTEST_ROOT, or a fresh temp dir) and the container is
# removed on exit. Exits non-zero on any failure. Regtest only: no money.
set -euo pipefail
cd "$(dirname "$0")/.."
repo=$(pwd)

# Bitcoin Core 30: TRUC (v3), pay-to-anchor and ephemeral-dust relay, and
# submitpackage. Same pinned image as deploy/lnd-sdk/compose.yaml.
BITCOIND_IMAGE=bitcoin/bitcoin:30.2@sha256:2b1d8a8d23c67b426afa8cd5b3ffd66b8c4ebbbceef3e214df8627aaead1f517
RUST_IMAGE=rust:1.97.1@sha256:b1b3c9c0d921d7fa0a6d1f9ec7e4eab87f8c8ec97644c3d791450f131dec813f

root="${LAYERTREE_REGTEST_ROOT:-$(mktemp -d "${TMPDIR:-/tmp}/layertree-regtest.XXXXXX")}"
mkdir -p "$root"
root=$(cd "$root" && pwd)
name="layertree-regtest-$$"
net="$name-net"

cleanup() {
  docker rm -f "$name" >/dev/null 2>&1 || true
  docker network rm "$net" >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker network create "$net" >/dev/null
docker run -d --name "$name" --network "$net" -p 127.0.0.1::18443 "$BITCOIND_IMAGE" \
  -regtest -server -txindex=1 -blockfilterindex=1 -fallbackfee=0.0002 \
  -rpcbind=0.0.0.0 -rpcallowip=0.0.0.0/0 \
  -rpcuser=regtest -rpcpassword=regtest-only >/dev/null

echo "layertree-regtest: scratch root $root"
args=(--features regtest --test regtest -- --ignored --nocapture --test-threads=1)
if command -v cargo >/dev/null 2>&1; then
  port=$(docker port "$name" 18443/tcp | head -n1 | sed 's/.*://')
  LAYERTREE_REGTEST_RPC="127.0.0.1:$port" LAYERTREE_REGTEST_ROOT="$root" \
    cargo test --manifest-path crates/layertree-tee-lab/Cargo.toml "${args[@]}"
else
  # No Rust on this machine: build and run inside the pinned image, on the
  # node's private network. The lab is its own workspace, so only it builds.
  docker run --rm --network "$net" \
    -v "$repo/crates/layertree-tee-lab:/lab" -v "$root:/scratch" \
    -e CARGO_TARGET_DIR=/scratch/target \
    -e LAYERTREE_REGTEST_RPC="$name:18443" -e LAYERTREE_REGTEST_ROOT=/scratch \
    -e LAYERTREE_WRITE_EVENTS \
    "$RUST_IMAGE" cargo test --manifest-path /lab/Cargo.toml "${args[@]}"
fi
echo "layertree-regtest: passed; event logs in $root"
