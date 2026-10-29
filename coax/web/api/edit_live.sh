#!/usr/bin/env bash
# Everglade's voice edits against the real model (eg-voice-edit): sentences
# to a deployed service as an admin (NIP-98, the key admin.sh uses), which
# asks Gemini on Vertex AI for one typed edit. test.sh checks the endpoint
# against a fake model; this checks the real one reads plurals, groups and
# names as it should. It asks only: nothing is placed, removed or published.
#   coax/web/api/edit_live.sh [https://staging.openagents.com]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
base="${1:-https://staging.openagents.com}"
key="${OA_ADMIN_KEY:-$HOME/work/.secrets/openagents-analytics-admin.key}"
sign="${XDG_CACHE_HOME:-$HOME/.cache}/oa-admin/sign"
if [[ ! -x "$sign" || "$here/sign.c" -nt "$sign" ]]; then
  mkdir -p "$(dirname "$sign")"
  secp="$(brew --prefix secp256k1)"; ssl="$(brew --prefix openssl@3)"
  /usr/bin/clang -std=c11 -O2 -Wall -Wextra -Werror -D_DEFAULT_SOURCE -I"$secp/include" -I"$ssl/include" \
    -o "$sign" "$here/sign.c" -L"$secp/lib" -lsecp256k1 -L"$ssl/lib" -lcrypto
fi
body="$(mktemp)"; trap 'rm -f "$body"' EXIT
url="$base/api/playtest/realm/everglade/edit"
failed=0
ask() {  # sentence, then each thing the answer must contain
  local sentence="$1" got; shift
  printf '{"sentence":"%s"}' "$sentence" > "$body"
  sleep 1.6   # one edit at a time per admin
  got="$(curl -s -w ' %{http_code}' -X POST "$url" -H 'Content-Type: application/json' \
         -H "Authorization: Nostr $("$sign" --key "$key" POST "$url" "$body")" --data-binary @"$body")"
  for want in "$@"; do
    if [[ "$got" != *"$want"* ]]; then echo "  FAIL  $sentence -> $got (wanted $want)"; failed=1; return; fi
  done
  echo "  ok    $sentence -> $got"
}
ask "remove these crates" '"edit": "remove"' '"item": "crate"' '"all": true' ' 200'
ask "get rid of those benches" '"edit": "remove"' '"item": "bench"' '"all": true'
ask "remove all the barrels here" '"item": "barrel"' '"all": true'
ask "clear this" '"edit": "remove"' '"all": true'
ask "remove that" '"edit": "remove"' ' 200'
ask "remove Marta" '"edit": "remove"' '"who": "Marta"'
ask "remove these cats" '"item": "cat"' '"all": true'
ask "put two barrels here" '"edit": "place"' '"item": "barrel"' '"count": 2'
ask "what time is it" ' 422'
exit $failed
