#!/usr/bin/env bash
# The Sandbox's set pieces against the real model (rl-sandbox-pieces): three
# sentences to a deployed service, which asks Gemini on Vertex AI. test.sh
# checks the endpoint against a fake model; this checks the real one picks
# from the catalogue as it should.
#   coax/web/api/piece_live.sh [https://staging.openagents.com]
set -euo pipefail
base="${1:-https://staging.openagents.com}"
failed=0
ask() {  # sentence, then what the answer must contain
  local got
  got="$(curl -s -w ' %{http_code}' -X POST "$base/api/playtest/sandbox/piece" -H 'Content-Type: application/json' \
         --data "{\"sentence\":\"$1\"}")"
  if [[ "$got" == *"$2"* ]]; then echo "  ok    $1 -> $got"; else echo "  FAIL  $1 -> $got (wanted $2)"; failed=1; fi
}
ask "a flock of birds follows me" '"piece": "flock"'
ask "a ring of twelve blue lanterns" '"colour": "blue"'
ask "park a red sports car here" ' 422'
exit $failed
