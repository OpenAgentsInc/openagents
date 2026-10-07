#!/usr/bin/env bash
# The Pylon end-to-end demo: a Psionic pylon on a GPU box, used from here.
#
#   scripts/pylon-demo.sh [PROMPT]
#
# Steps:
#   1. Build the `pylon` client here and print this computer's buyer key.
#   2. On the box (over ssh), start Psionic and the provider, admitting only
#      that buyer key (scripts/pylon-psionic.sh start --allow NPUB).
#   3. Wait for the pylon's beacon to appear on the relay.
#   4. Ask it one question, encrypted, and publish the receipt.
#   5. Publish a pool aggregate and recompute it as a reader.
#   6. Write the transcript (prompt, answer, latency, pylon npub, receipt).
#
# The box keeps serving afterward; stop it with
#   ssh $PYLON_HOST $PYLON_REPO/scripts/pylon-psionic.sh stop
#
# Environment:
#   PYLON_HOST     ssh destination of the GPU box (default coderos-4080).
#   PYLON_REPO     The openagents checkout on the box, already set up with
#                  `scripts/pylon-psionic.sh setup` (default ~/work/pylon-p1/openagents).
#   PYLON_RELAY    Relay (default wss://relay.openagents.com).
#   REMOTE_PYLON_DIR, REMOTE_PYLON_TARGET, REMOTE_OPENAGENTS_PYLON_HOME
#                  Passed to the box as PYLON_DIR, PYLON_TARGET, and
#                  OPENAGENTS_PYLON_HOME (see scripts/pylon-psionic.sh).
#   PYLON_TARGET   Local Cargo target directory (default $CARGO_TARGET_DIR,
#                  else ~/work/openagents-target-pylon).
#   PYLON_OUT      Transcript file (default ./pylon-demo-<time>.txt in the
#                  scratch directory `openagents scratch` prints, else here).
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
host="${PYLON_HOST:-coderos-4080}"
remote_repo="${PYLON_REPO:-~/work/pylon-p1/openagents}"
relay="${PYLON_RELAY:-wss://relay.openagents.com}"
target="${PYLON_TARGET:-${CARGO_TARGET_DIR:-$HOME/work/openagents-target-pylon}}"
prompt="${1:-In one sentence, what is a Lightning Network payment channel?}"
scratch="$(openagents scratch 2>/dev/null || echo .)"
out="${PYLON_OUT:-$scratch/pylon-demo-$(date +%Y%m%d-%H%M%S).txt}"

echo "== building the pylon client"
cargo build --manifest-path "$repo/Cargo.toml" -p pylon --bin pylon --target-dir "$target" >/dev/null
pylon="$target/debug/pylon"
buyer="$("$pylon" --json whoami | sed -E 's/.*"buyer":"([^"]+)".*/\1/')"
echo "buyer key here: $buyer"

echo "== starting the pylon on $host"
remote_env="PYLON_RELAY=$relay"
for name in PYLON_DIR PYLON_TARGET OPENAGENTS_PYLON_HOME; do
  remote="REMOTE_$name"
  [ -n "${!remote:-}" ] && remote_env="$remote_env $name=${!remote}"
done
ssh -o BatchMode=yes "$host" "$remote_env $remote_repo/scripts/pylon-psionic.sh start --allow $buyer"
pylon_npub="$(ssh -o BatchMode=yes "$host" "$remote_env $remote_repo/scripts/pylon-psionic.sh whoami" | sed -E 's/.*"provider":"([^"]+)".*/\1/')"
echo "pylon key there: $pylon_npub"

echo "== waiting for its beacon"
for _ in $(seq 1 30); do
  if "$pylon" status --relay "$relay" | grep -q "^online"; then break; fi
  sleep 2
done
"$pylon" status --relay "$relay"

echo "== asking"
answer_text="$("$pylon" ask --relay "$relay" --pylon "$pylon_npub" "$prompt")"
echo "$answer_text"

echo "== pool aggregate"
pool_text="$("$pylon" pool --relay "$relay" --publish)"
echo "$pool_text"
aggregator="$("$pylon" --json whoami | sed -E 's/.*"aggregator":"([^"]+)".*/\1/')"
verify_text="$("$pylon" pool verify --relay "$relay" --aggregator "$aggregator")"
echo "$verify_text"

{
  echo "Pylon demo, $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "relay:  $relay"
  echo "pylon:  $pylon_npub on $host (Psionic, CUDA)"
  echo "buyer:  $buyer"
  echo "prompt: $prompt"
  echo "$answer_text"
  echo "pool: $pool_text"
  echo "verify: $verify_text"
} >"$out"
echo "== transcript: $out"
