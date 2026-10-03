#!/usr/bin/env bash
# The payments end-to-end demo (#10199): a real plugin, a real x402 payment,
# the split, the live flow, the payout, and reconciliation, one step at a
# time, printing where to look at each.
#
#   scripts/payments-demo.sh --quote-only            # steps 1-2: no money moves
#   scripts/payments-demo.sh                         # the paid run (2 calls)
#   scripts/payments-demo.sh --operator --publish --payout spark1...   # publish first
#
# Steps:
#   0. (--publish --operator) publish "Explain this error" from
#      crates/plugin-explain-error on oa-pay-1 with a 10-sat fee (--fee-msat)
#      and the author's payout address, and register its listing with the
#      flow server so /live names it.
#   1. Quote: a keyless POST to api.openagents.com/v1/plugins/ID/invoke gets a
#      402 naming the price (endpoint + author fee), payTo, and the invoice.
#   2. Before: the public totals on openagents.com/api/stats.
#   3. Pay and retry, CALLS times: `openagents x402 fetch ... --pay-with
#      phone` (the phone asks to approve each payment; `--pay-with wallet`
#      pays from this computer's x402 node instead).
#   4. Settlement and split: the payment, share, and bonus events on the
#      public flow (openagents.com/api/flow/snapshot) and the time each took
#      to appear; with --operator, the ledger rows on oa-pay-1.
#   5. Live: openagents.com/live and /stats.
#   6. Payout: when the author's share goes out (with --operator, the payout
#      rows); a Spark payee is paid once 100 sats are owed or its oldest share
#      is a day old.
#   7. Reconciliation: /stats `reconciliation` (with --operator, a fresh run
#      and its report).
#
# Options:
#   --plugin ID        plugin id or slug (default explain-error)
#   --calls N          paid calls (default 2)
#   --pay-with HOW     phone (default) or wallet
#   --max-msat N       the buyer ceiling per call (default 20000)
#   --body FILE        the failing output sent to the plugin (default a rustc error)
#   --publish          publish the plugin first (needs --operator and --payout)
#   --payout ADDRESS   the author's Spark address or Lightning address
#   --fee-msat N       the author fee when publishing (default 10000)
#   --version V        the version published (default 0.2.0)
#   --operator         also read and act on oa-pay-1 over IAP ssh
#   --first-receive    (operator) print a fresh 2,000-sat invoice that opens the
#                      receiver's first channel when paid, then exit
#   --quote-only       stop after the quote and the totals; nothing is paid
#
# Environment: OPENAGENTS_BIN (default `openagents`), PAY_API (default
# https://api.openagents.com), PAY_SITE (default https://openagents.com),
# CLOUDSDK_CONFIG for --operator (default the workspace automation config
# when present). Never prints a secret: no seed, key, or preimage.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/.." && pwd)
oa=${OPENAGENTS_BIN:-openagents}
api=${PAY_API:-https://api.openagents.com}
site=${PAY_SITE:-https://openagents.com}
plugin=explain-error
calls=2
pay_with=phone
max_msat=20000
body=""
publish=0
payout=""
fee_msat=10000
version=0.2.0
operator=0
quote_only=0
first_receive=0

while [ $# -gt 0 ]; do
  case "$1" in
    --plugin) plugin=$2; shift 2 ;;
    --calls) calls=$2; shift 2 ;;
    --pay-with) pay_with=$2; shift 2 ;;
    --max-msat) max_msat=$2; shift 2 ;;
    --body) body=$2; shift 2 ;;
    --publish) publish=1; shift ;;
    --payout) payout=$2; shift 2 ;;
    --fee-msat) fee_msat=$2; shift 2 ;;
    --version) version=$2; shift 2 ;;
    --operator) operator=1; shift ;;
    --quote-only) quote_only=1; shift ;;
    --first-receive) first_receive=1; operator=1; shift ;;
    -h|--help) awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"; exit 0 ;;
    *) echo "unknown option $1 (see --help)" >&2; exit 2 ;;
  esac
done

for tool in curl jq base64; do
  command -v "$tool" >/dev/null || { echo "needs $tool" >&2; exit 1; }
done
work=$(mktemp -d "${TMPDIR:-/tmp}/payments-demo.XXXXXX")
trap 'rm -rf "$work"' EXIT

step() { printf '\n== %s\n' "$*"; }
look() { printf '   look: %s\n' "$*"; }
now_ms() { python3 -c 'import time; print(int(time.time()*1000))' 2>/dev/null || echo $(( $(date +%s) * 1000 )); }

if [ "$operator" = 1 ] && [ -z "${CLOUDSDK_CONFIG:-}" ] && [ -d "$HOME/work/.secrets/gcloud-sa-config" ]; then
  export CLOUDSDK_CONFIG="$HOME/work/.secrets/gcloud-sa-config"
fi
on_host() {
  gcloud compute ssh oa-pay-1 --zone us-central1-a --project openagentsgemini \
    --tunnel-through-iap --command "$1" 2>/dev/null
}
ledger_sql() {
  on_host "sudo sqlite3 -readonly -header -column /var/lib/openagents-pay/ledger/ledger.sqlite \"$1\""
}

if [ "$first_receive" = 1 ]; then
  step "First receive: a 2,000-sat invoice to the receiver (opens its first channel)"
  on_host 'R=$(openssl rand -hex 32); sudo -u openagents-pay sh -c "set -a; . /etc/openagents-pay/openagents-pay.env; exec /opt/openagents-pay/current/openagents --json x402 node invoice --msat 2000000 --request-hash $R --expiry 604800"' \
    | jq -r '"   payment hash \(.payment_hash), expires in \(.expiry_secs) s\n\n\(.bolt11)\n"'
  look "pay it from any Lightning wallet; then: sudo journalctl -u openagents-pay -f on oa-pay-1 (channel_pending, channel_ready, the payment)"
  exit 0
fi

if [ -z "$body" ]; then
  body="$work/error.txt"
  cat > "$body" <<'EOF'
error[E0308]: mismatched types
 --> src/main.rs:4:18
  |
4 |     let n: u32 = "5";
  |            ---   ^^^ expected `u32`, found `&str`
  |            |
  |            expected due to this
EOF
fi

# 0. Publish, on oa-pay-1: its `openagents` signs releases with a fee (NIP-EXT
#    G9), under the pay host's key, which is then the author party; the
#    author's money goes to --payout. The plugin directory there is
#    crates/plugin-explain-error's package.json (publisher cleared) and
#    program.
if [ "$publish" = 1 ]; then
  step "0. Publish \"Explain this error\" with a ${fee_msat} msat author fee"
  [ "$operator" = 1 ] || { echo "   --publish runs on oa-pay-1: add --operator" >&2; exit 1; }
  [ -n "$payout" ] || { echo "   needs --payout ADDRESS (the phone: Wallet > Receive > Spark, Copy)" >&2; exit 1; }
  case "$payout" in *[!A-Za-z0-9@._+-]*) echo "   --payout has characters no address has" >&2; exit 1 ;; esac
  case "$version" in *[!0-9.]*) echo "   --version is digits and dots" >&2; exit 1 ;; esac
  case "$fee_msat" in *[!0-9]*) echo "   --fee-msat is a number" >&2; exit 1 ;; esac
  pkg=$(jq -c --arg v "$version" '.publisher = "" | .version = $v' "$repo/crates/plugin-explain-error/package.json" | base64 | tr -d '\n')
  src=/var/lib/openagents-pay/plugins-src
  on_host "set -e; sudo -u openagents-pay install -d -m 0700 $src/explain-error/programs; \
    echo $pkg | base64 -d | sudo -u openagents-pay tee $src/explain-error/package.json >/dev/null; \
    sudo -u openagents-pay cp $src/explain-error-check/programs/explain-error.json $src/explain-error/programs/; \
    cd /; sudo -u openagents-pay env HOME=/var/lib/openagents-pay /opt/openagents-pay/current/openagents --json \
      plugin publish $src/explain-error --fee-msat $fee_msat --payout $payout" > "$work/published.json"
  plugin=$(jq -r .id "$work/published.json")
  jq -r '"   id \(.id)\n   release \(.release) on \(.relay), fee \(.fee.fee_msat) msat to \(.fee.payout)"' "$work/published.json"
  look "openagents plugin search explain-error"
  if command -v nak >/dev/null; then
    listing=$(nak req -k 30184 -a "${plugin%%:*}" -d "${plugin#*:}" -l 1 wss://relay.openagents.com 2>/dev/null | head -n 1)
    if [ -n "$listing" ]; then
      b64=$(printf '%s\n' "$listing" | base64 | tr -d '\n')
      on_host "echo $b64 | base64 -d | sudo -u openagents-pay tee -a /var/lib/openagents-pay/flow/publications.ndjson >/dev/null" \
        && echo "   registered the listing with the flow server: /live names it ${plugin#*:}"
    fi
  else
    echo "   (no nak here: /live shows the plugin under an alias until its listing is registered)"
  fi
fi

url="$api/v1/plugins/$plugin/invoke"

# 1. Quote.
step "1. Quote: POST $url with no payment"
status=$(curl -s -o "$work/402.json" -D "$work/402.headers" -w '%{http_code}' -X POST --data-binary @"$body" "$url")
echo "   HTTP $status"
if [ "$status" != 402 ]; then
  cat "$work/402.json"; echo
  echo "   expected 402; is the plugin published (step 0) and the pay front up?" >&2
  exit 1
fi
jq -r '"   \(.detail)", (.price_parts[] | "   part \(.name): \(.msat) msat")' "$work/402.json"
header=$(grep -i '^payment-required:' "$work/402.headers" | cut -d' ' -f2 | tr -d '\r')
printf '%s' "$header" | base64 -d 2>/dev/null > "$work/required.json" || true
jq -r '.accepts[0] | "   payTo \(.payTo)\n   network \(.network)\n   amount \(.amount) msat\n   invoice \(.extra.invoice // "(in the header)" | .[0:60])..."' "$work/required.json" 2>/dev/null || true
grep -i '^www-authenticate:' "$work/402.headers" | cut -c1-110 | sed 's/^/   /'
look "the same 402 carries x402 v2 (PAYMENT-REQUIRED) and the HTTP Payment scheme (WWW-Authenticate)"

# 2. Before.
step "2. Before: public totals"
curl -s "$site/api/stats" | jq -c '{totals, reconciliation}' | sed 's/^/   /'
base_seq=$(curl -s "$site/api/flow/snapshot" | jq '[.events[].seq] | max // 0')
look "$site/stats and $site/live (keep /live open now)"

if [ "$quote_only" = 1 ]; then
  step "Quote only: nothing was paid. Next: run without --quote-only."
  exit 0
fi

# 3. Pay and retry.
"$oa" x402 --help >/dev/null 2>&1 || { echo "this \`openagents\` has no x402 command; install a release with #10199" >&2; exit 1; }
paid_at=()
for i in $(seq 1 "$calls"); do
  step "3. Pay and retry, call $i of $calls (--pay-with $pay_with)"
  [ "$pay_with" = phone ] && echo "   your phone asks to approve ${max_msat} msat at most: tap Approve and pay"
  wait=60; [ "$pay_with" = phone ] && wait=300
  "$oa" --json x402 fetch "$url" --method POST --body "$body" --max-msat "$max_msat" \
    --pay-with "$pay_with" --wait "$wait" > "$work/paid-$i.json" || true
  paid_at+=("$(now_ms)")
  jq -r '"   HTTP \(.status) paid \(.paid) amount \(.amount_msat) msat routing fee \(.fee_msat) msat\n   payment \(.payment_hash)"' "$work/paid-$i.json" 2>/dev/null || cat "$work/paid-$i.json"
  jq -r '.body | fromjson? | .value // . | tostring | .[0:300]' "$work/paid-$i.json" 2>/dev/null | sed 's/^/   result: /'
done
look "openagents x402 ledger --since 3600 (this buyer's side)"

# 4. Settlement and split on the public flow.
step "4. Settlement and split"
deadline=$(( $(date +%s) + 30 ))
while :; do
  curl -s "$site/api/flow/snapshot" > "$work/snapshot.json"
  n=$(jq --argjson s "$base_seq" '[.events[] | select(.seq > $s and .type == "payment")] | length' "$work/snapshot.json")
  [ "$n" -ge "$calls" ] || [ "$(date +%s)" -ge "$deadline" ] || { sleep 1; continue; }
  break
done
jq -r --argjson s "$base_seq" '.events[] | select(.seq > $s) |
  "   #\(.seq) \(.type) \(.plugin // .node) \(.amount_sats // "")\(if .split then " split \(.split|tojson)" else "" end)\(if .author then " author \(.author)" else "" end)"' "$work/snapshot.json"
first_payment_at=$(jq --argjson s "$base_seq" '[.events[] | select(.seq > $s and .type == "payment") | .at] | min // empty' "$work/snapshot.json")
if [ -n "$first_payment_at" ] && [ "${#paid_at[@]}" -gt 0 ]; then
  echo "   first payment event at $first_payment_at ms; call 1 returned at ${paid_at[0]} ms (clock skew applies)"
fi
look "$site/api/flow/snapshot (public events), $site/api/stats per_plugin and per_author"
if [ "$operator" = 1 ]; then
  echo "   ledger on oa-pay-1 (newest settlements, their shares and bonuses):"
  ledger_sql "SELECT seq, substr(payment_hash,1,12) AS hash, plugin_id IS NOT NULL AS plugin, price_msat, received_msat, lsp_fee_msat, rule_version, datetime(settled_at,'unixepoch') AS at FROM settlement ORDER BY seq DESC LIMIT $calls;" | sed 's/^/   /'
  ledger_sql "SELECT substr(s.settlement,1,12) AS hash, substr(s.party,1,16) AS party, s.role, s.amount_msat FROM share s JOIN (SELECT payment_hash FROM settlement ORDER BY seq DESC LIMIT $calls) t ON t.payment_hash = s.settlement ORDER BY 1, 3;" | sed 's/^/   /'
  ledger_sql "SELECT substr(settlement,1,12) AS hash, kind, requested_msat, amount_msat, outcome FROM bonus ORDER BY rowid DESC LIMIT 4;" | sed 's/^/   /'
fi

# 5. Live.
step "5. Live"
echo "   /live draws each call as a white dot out, each payment gold back, each share gold on to the author"
look "$site/live   $site/stats   desktop: the routes-live deck scene"

# 6. Payout.
step "6. Payout"
echo "   the payout worker (every 60 s) pays a Spark address once 100 sats are owed, a Lightning address at 1,000 sats, or either once the oldest owed share is a day old"
curl -s "$site/api/stats" | jq -c '{received: .totals.received_sats, paid_out: .totals.paid_out_sats, owed: .totals.pending_accruals_sats}' | sed 's/^/   /'
if [ "$operator" = 1 ]; then
  on_host "sudo -u openagents-pay sh -c 'set -a; . /etc/openagents-pay/openagents-pay.env; exec /opt/openagents-pay/current/openagents pay payout-list --ledger /var/lib/openagents-pay/ledger/ledger.sqlite'" | tail -n 8 | sed 's/^/   /'
fi
look "your phone: Wallet > History (the payout arrives as a Spark or Lightning payment)"

# 7. Reconciliation.
step "7. Reconciliation"
curl -s "$site/api/stats" | jq -r '"   /stats reconciliation: \(.reconciliation)"'
if [ "$operator" = 1 ]; then
  on_host "sudo systemctl start openagents-pay-reconcile; sudo cat /var/lib/openagents-pay/reconcile/latest.txt" | sed 's/^/   /'
fi
look "$site/stats (reconciliation ok); operator: journalctl -u openagents-pay-reconcile -p err on oa-pay-1"
