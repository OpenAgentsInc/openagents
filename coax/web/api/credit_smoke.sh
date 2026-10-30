#!/usr/bin/env bash
# Live smoke for inference credit (NIP-X402 openagents:spark-receiver:1): a
# key that is not an admin's or on the list buys credit over Lightning and
# spends it on /api/v1/inference.
#
#   coax/web/api/credit_smoke.sh [ORIGIN] [SATS]   (https://openagents.com, 50)
#
# It asks for a top-up, puts the invoice on the clipboard and prints it: pay
# it from any Lightning wallet (paste it into your phone's wallet through
# Universal Clipboard). It checks every 5 s, with PAYMENT-SIGNATURE, until our
# wallet has the payment or the invoice expires, then reads the credit, makes
# one signed inference request and reads the credit again (it went down).
# The key is ~/work/.secrets/x402-smoke-nostr.key, made on first use. It
# never pays anything itself.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
origin="${1:-https://openagents.com}" sats="${2:-50}"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
secp="$(brew --prefix secp256k1 2>/dev/null || true)"
/usr/bin/clang -std=c11 -O1 -D_DEFAULT_SOURCE ${secp:+-I"$secp/include"} -I"$(brew --prefix openssl@3)/include" -o "$work/sign" "$here/sign.c" \
  ${secp:+-L"$secp/lib"} -lsecp256k1 -L"$(brew --prefix openssl@3)/lib" -lcrypto
key="$HOME/work/.secrets/x402-smoke-nostr.key"
[[ -f "$key" ]] || "$work/sign" --new "$key" >/dev/null
pk="$("$work/sign" --key "$key" --pubkey)"
url="$origin/api/v1/inference/credit"
body="{\"pubkey\":\"$pk\",\"msat\":\"$((sats * 1000))\"}"
balance() { curl -sS -m 30 -H "Authorization: Nostr $("$work/sign" --key "$key" GET "$url")" "$url"; }
echo "key: $pk"
echo "credit before: $(balance)"
code="$(curl -sS -m 60 -o "$work/required.json" -D "$work/required.h" -w '%{http_code}' -H 'Content-Type: application/json' --data-binary "$body" "$url")"
[[ "$code" == 402 ]] || { echo "top-up: expected 402, got $code: $(cat "$work/required.json")" >&2; exit 1; }
hdr="$(grep -i '^payment-required:' "$work/required.h" | cut -d' ' -f2- | tr -d '\r')"
read -r invoice hash expiry < <(python3 -I -c 'import base64,json,sys; a=json.loads(base64.b64decode(sys.argv[1]))["accepts"][0]; print(a["extra"]["invoice"], a["extra"]["paymentHash"], a["maxTimeoutSeconds"])' "$hdr")
sig="$(python3 -I -c 'import base64,json,sys; a=json.loads(base64.b64decode(sys.argv[1]))["accepts"][0]; print(base64.b64encode(json.dumps({"x402Version":2,"accepted":a,"payload":{"paymentHash":a["extra"]["paymentHash"]}}).encode()).decode())' "$hdr")"
printf '%s' "$invoice" | pbcopy 2>/dev/null || true
echo; echo "Pay this $sats-sat invoice from any Lightning wallet (it is on the clipboard):"; echo; echo "$invoice"; echo
deadline=$(( $(date +%s) + expiry + 60 ))
while :; do
  code="$(curl -sS -m 60 -o "$work/settle.json" -D "$work/settle.h" -w '%{http_code}' -H 'Content-Type: application/json' -H "PAYMENT-SIGNATURE: $sig" --data-binary "$body" "$url")"
  if [[ "$code" == 200 || "$code" == 409 ]]; then echo "top-up $code: $(cat "$work/settle.json")"; break; fi
  [[ "$code" == 402 ]] || { echo "top-up: $code $(cat "$work/settle.json")" >&2; exit 1; }
  (( $(date +%s) < deadline )) || { echo "the invoice expired unpaid" >&2; exit 1; }
  sleep 5
done
echo "credit after top-up: $(balance)"
ask='{"model":"google/gemini-3.8-flash","messages":[{"role":"user","content":"Say OK."}],"max_tokens":16}'
printf '%s' "$ask" > "$work/ask.json"
iurl="$origin/api/v1/inference"
code="$(curl -sS -m 120 -o "$work/answer.json" -w '%{http_code}' -H 'Content-Type: application/json' -H "Authorization: Nostr $("$work/sign" --key "$key" POST "$iurl" "$work/ask.json")" --data-binary @"$work/ask.json" "$iurl")"
echo "inference $code: $(head -c 300 "$work/answer.json")"
sleep 3
echo "credit after the request: $(balance)"
[[ "$code" == 200 ]]
