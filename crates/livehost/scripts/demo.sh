#!/usr/bin/env bash
# Set up and run the livehost demo against a relay that relay.sh started:
# copies the demo sites and guests to a work directory, creates deploy keys
# for the two tenants (site and game), writes the host config, and starts
# the host. Deploy from another terminal with the commands it prints.
#
# Usage: crates/livehost/scripts/demo.sh WORK_DIR

set -euo pipefail

work="${1:?usage: demo.sh WORK_DIR}"
crate="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
: "${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR to the shared target}"
livehost="$CARGO_TARGET_DIR/release/livehost"
[[ -x "$livehost" ]] || { echo "demo.sh: build it first: cargo build --release -p livehost" >&2; exit 1; }
relay="${LIVEHOST_RELAY:-ws://127.0.0.1:7448}"
port="${LIVEHOST_PORT:-8090}"

mkdir -p "$work"
if [[ ! -d "$work/src" ]]; then
  mkdir -p "$work/src"
  cp -R "$crate/demo" "$crate/guests" "$crate/wit" "$work/src/"
  rm -rf "$work/src/guests/target"
fi
for tenant in site game; do
  [[ -f "$work/$tenant.key" ]] || "$livehost" keygen --out "$work/$tenant.key" >/dev/null
done
cat > "$work/host.json" <<JSON
{"tenants": [
  {"name": "site", "pubkey": "$("$livehost" pubkey --key "$work/site.key")", "channel": "staging", "hosts": ["127.0.0.1", "localhost", "site.localhost"]},
  {"name": "game", "pubkey": "$("$livehost" pubkey --key "$work/game.key")", "channel": "staging", "hosts": ["game.localhost"]}
]}
JSON

cat <<TEXT
Deploy:
  $livehost deploy $work/src/demo/site --store $work/store --relay $relay --key $work/site.key
  $livehost deploy $work/src/demo/game --store $work/store --relay $relay --key $work/game.key
Open http://127.0.0.1:$port/ and http://game.localhost:$port/ (or /t/game/).
TEXT
exec "$livehost" serve --config "$work/host.json" --store "$work/store" --relay "$relay" --listen "127.0.0.1:$port"
