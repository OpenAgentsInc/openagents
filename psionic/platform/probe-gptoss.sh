#!/usr/bin/env bash
# One public synthetic prompt; the native Coax client checks signature,
# provider/model pins, correlation and completed final text before success.
set -euo pipefail
umask 077
provider=${1:?provider required};evidence=${2:?evidence directory required}
mkdir -p "$evidence"
nonce="$(date +%s)-${CLOUD_RUN_EXECUTION:-watch}"
client=${PYLON_WATCH_TEXT_CLIENT:-/app/pylon-text-probe}
"$client" --relay wss://relay.openagents.com --provider "$provider" \
 --identity gpt-oss-20b@sha256:3340a61d1a0391e8c5b5d3463d18d4c48129a84bbc04a554c762c99020aa06ed \
 --prompt "Reply exactly PONG. Public health probe $nonce." --expect PONG --timeout-ms 60000 \
 > "$evidence/client.log" 2>&1
# Protocol verification is native. Extract only its successful public receipt,
# not connection diagnostics, and refuse absent/ambiguous success output.
jq -Rsc 'split("\n") | map(fromjson? | select(.ok==true and .verified==true)) | if length==1 then .[0] else error("no unique verified result") end' "$evidence/client.log" > "$evidence/result.json"
jq -c --argjson checked_at "$(date +%s)" '.+{severity:"INFO",kind:"gptoss",checked_at:$checked_at}' "$evidence/result.json"
