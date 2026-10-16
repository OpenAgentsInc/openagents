#!/usr/bin/env bash
# Exercise the exact staged-attack executable on an isolated Core and test receiver.
set -euo pipefail
cd "$(dirname "$0")/../.."
: "${CARGO_TARGET_DIR:?Set the long-lived external target directory.}"
root=${LAYERTREE_STAGE_REGTEST_ROOT:-$(openagents scratch)/layertree-stage-$(date -u +%Y%m%dT%H%M%S)-$$}
mkdir -p "$root"
root=$(cd "$root" && pwd)
node="layertree-stage-$$"
receiver_pid=
cleanup() {
  if [[ -n $receiver_pid ]]; then kill "$receiver_pid" >/dev/null 2>&1 || true; wait "$receiver_pid" 2>/dev/null || true; fi
  docker rm -fv "$node" >/dev/null 2>&1 || true
}
trap cleanup EXIT
cargo build --locked --manifest-path crates/layertree-tee-lab/Cargo.toml --features stage-attacks --bins
stage="$CARGO_TARGET_DIR/debug/layertree-stage"
follower="$CARGO_TARGET_DIR/debug/layertree-follow"
"$stage" init "$root/tree" > "$root/funding-address.json"
image=bitcoin/bitcoin:30.2@sha256:2b1d8a8d23c67b426afa8cd5b3ffd66b8c4ebbbceef3e214df8627aaead1f517
docker run -d --name "$node" -p 127.0.0.1::18443 "$image" \
  -regtest -server -txindex=1 -blockfilterindex=1 -fallbackfee=0.0002 \
  -rpcbind=0.0.0.0 -rpcallowip=0.0.0.0/0 -rpcuser=regtest -rpcpassword=regtest-only >/dev/null
cli() { docker exec "$node" bitcoin-cli -regtest -rpcuser=regtest -rpcpassword=regtest-only "$@"; }
for _ in $(seq 1 50); do if cli getblockchaininfo >/dev/null 2>&1; then break; fi; sleep .1; done
cli createwallet stage >/dev/null
miner=$(cli getnewaddress)
cli generatetoaddress 101 "$miner" >/dev/null
address=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["regtest_address"])' "$root/funding-address.json")
txid=$(cli sendtoaddress "$address" 0.003)
cli generatetoaddress 1 "$miner" >/dev/null
cli getrawtransaction "$txid" true > "$root/funding-transaction.json"
vout=$(python3 - "$root/funding-transaction.json" "$address" <<'PY'
import json,sys
print(next(out['n'] for out in json.load(open(sys.argv[1]))['vout'] if out['scriptPubKey'].get('address')==sys.argv[2]))
PY
)
until=$(($(date +%s)+240))
python3 scripts/layertree/alarm-receiver.py --directory "$root/notifications" --until "$until" --network regtest > "$root/receiver.log" 2>&1 &
receiver_pid=$!
for _ in $(seq 1 100); do [[ -f $root/notifications/receiver.json ]] && break; sleep .05; done
export LAYERTREE_STAGE_ALARM_WEBHOOK
LAYERTREE_STAGE_ALARM_WEBHOOK=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["url"])' "$root/notifications/receiver.json")
port=$(docker port "$node" 18443/tcp | head -n1 | sed 's/.*://')
LAYERTREE_RPC_USER=regtest LAYERTREE_RPC_PASSWORD=regtest-only LAYERTREE_RPC_URL="http://127.0.0.1:$port" \
  "$stage" run "$root/tree" "$txid:$vout" "$follower" --until "$until" --network regtest --mine-address "$miner" > "$root/stage.jsonl" 2> "$root/stage.log"
python3 - "$root" <<'PY'
import json,sys
from pathlib import Path
root=Path(sys.argv[1]);evidence=json.loads((root/'tree/attack-evidence.json').read_text())
assert [row['scenario'] for row in evidence]==['withheld','mismatch','theft']
receipts={row['id'] for row in map(json.loads,(root/'notifications/received.jsonl').read_text().splitlines())}
for scenario in evidence:
 assert len(scenario['alarms'])==3
 assert {row['member'] for row in scenario['alarms']}=={0,1,2}
 assert all(row['id'] in receipts for row in scenario['alarms'])
 if scenario['scenario']=='theft':
  assert all(row['observed_tip']<=scenario['height']+1 for row in scenario['alarms'])
print('All three staged attacks reached all three follower logs and the webhook receiver.')
PY
