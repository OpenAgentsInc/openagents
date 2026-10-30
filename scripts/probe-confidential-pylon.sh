#!/bin/sh
# Optional operational oracle, never a product admission or signing path.
set -eu
umask 077
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
oracle_bin=${OA_ATT_ORACLE_BIN:-${OA_ATT_ORACLE_TARGET:-${XDG_CACHE_HOME:-$HOME/.cache}/openagents/att-oracle-853417f290e7cae1df18c081b89b0818432e60af}/debug/oa-att}
if [ -z "${OA_ATT_ORACLE_BIN:-}" ] && [ -x "$script_dir/oa-att" ]; then oracle_bin=$script_dir/oa-att; fi
evidence_base=${TMPDIR:-/tmp}
seconds=120
usage() { printf '%s\n' 'usage: probe-confidential-pylon.sh [--oracle FILE] [--evidence-dir DIR] [--timeout-seconds 1..120]'; }
bad_args() { printf '%s\n' '{"ok":false,"reason":"invalid_arguments"}'; exit 2; }
while [ "$#" -gt 0 ]; do
  case "$1" in
    --help|-h) usage; exit 0;;
    --oracle|--evidence-dir|--timeout-seconds)
      [ "$#" -ge 2 ] || bad_args
      case "$1" in --oracle) oracle_bin=$2;; --evidence-dir) evidence_base=$2;; --timeout-seconds) seconds=$2;; esac
      shift 2;;
    *) bad_args;;
  esac
done
case "$seconds" in ''|*[!0-9]*|????*) bad_args;; esac
[ "$seconds" -ge 1 ] && [ "$seconds" -le 120 ] || bad_args
command -v jq >/dev/null 2>&1 || { printf '%s\n' '{"ok":false,"reason":"jq_missing"}'; exit 1; }
timeout_bin=$(command -v timeout || command -v gtimeout || true)
[ -n "$timeout_bin" ] || { printf '%s\n' '{"ok":false,"reason":"timeout_missing"}'; exit 1; }
[ -x "$oracle_bin" ] || { printf '%s\n' '{"ok":false,"reason":"cached_oracle_missing"}'; exit 1; }
mkdir -p "$evidence_base"
work=$(mktemp -d "$evidence_base/oa-confidential-probe.XXXXXXXX")
oracle_pid=''
cleanup() {
  trap - EXIT HUP INT TERM
  if [ -n "$oracle_pid" ]; then kill "$oracle_pid" 2>/dev/null || true; wait "$oracle_pid" 2>/dev/null || true; fi
  rm -f "$work/pending.json"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP
fail() {
  jq -n --arg reason "$1" --argjson oracle_exit "${2:-0}" --arg proof_dir "$work" \
    '{ok:false,reason:$reason,oracle_exit:$oracle_exit,proof_dir:$proof_dir}' > "$work/summary.json"
  cat "$work/summary.json"
  exit 1
}
marker=$(date -u '+%Y%m%dT%H%M%SZ')-$(od -An -N8 -tx1 /dev/urandom | tr -d ' \n')
state="The weather in Lisbon is sunny today. This is public operational sample $marker."
question='Is this about the weather?'
jq -n --arg state "$state" --arg question "$question" '{state:$state,question:$question}' > "$work/request.json"
oracle_exit=0
# No operator key or arbitrary input. The existing oracle generates its caller,
# fetches current vendor evidence, verifies it, seals input and checks the receipt.
"$timeout_bin" -k 2 "$seconds" "$oracle_bin" round \
  --relay wss://relay.openagents.com \
  --publisher 77fabebbeb49a7b9b384422ee6ef5662cf4db7da70acc94981378c0017ecc56e \
  --workload clef-decisions --state "$state" --question "$question" \
  > "$work/pending.json" 2>/dev/null &
oracle_pid=$!
wait "$oracle_pid" || oracle_exit=$?
oracle_pid=''
[ "$oracle_exit" -eq 0 ] || fail oracle_failed "$oracle_exit"
jq -e 'type=="object"' "$work/pending.json" >/dev/null 2>&1 || fail invalid_oracle_json
mv "$work/pending.json" "$work/round.json"
jq -e 'has("refused")' "$work/round.json" >/dev/null && fail oracle_refused
# These are health-report consistency assertions, not a second verifier.
# All signature, vendor chain, measurement and key-admission decisions remain
# in the unchanged historical oracle; root's Coax controller owns recovery.
jq -e --arg proof "$work/round.json" --argjson checked_at "$(date -u +%s)" \
  -f "$script_dir/confidential-pylon-summary.jq" "$work/round.json" > "$work/summary.json" 2>/dev/null || fail invalid_oracle_report
cat "$work/summary.json"
