#!/usr/bin/env bash
# The admin's side of realm makers (makers.c, docs/verse/makers.md), on this machine: signs a
# NIP-98 request with the admin's key (admin.sh's signer and key) and prints the answer.
#
#   makers.sh code [--staging] REALM     a new one-time claim code; prints the claim link
#   makers.sh who  [--staging] REALM     the realm's maker, as anyone sees it
#   makers.sh list-fairy [--staging] LIGHTNING [SATS] [NAME]   lists a fairy for Fairy Forest's UPGRADE
#                                        (made by OpenAgents, its share to LIGHTNING; 100 sats); it waits
#                                        for an admin's APPROVE in the UPGRADE pane
#   makers.sh upgrades [--staging] ARG   the listings as lines (ARG: -)
#   makers.sh approve|reject [--staging] ID
#
# OA_ADMIN_KEY names the key file (default ~/work/.secrets/openagents-analytics-admin.key).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
key="${OA_ADMIN_KEY:-$HOME/work/.secrets/openagents-analytics-admin.key}"
cmd="${1:-}"; shift || true
origin=https://openagents.com
if [[ "${1:-}" == --staging ]]; then origin=https://staging.openagents.com; shift; fi
realm="${1:-}"
signed() {   # METHOD URL BODY: the answer of a request signed by the admin's key
  local body; body="$(mktemp)"; printf '%s' "$3" > "$body"
  curl -sS -X "$1" -H "Content-Type: application/json" -H "Origin: $origin" -H "Authorization: Nostr $("$sign" --key "$key" "$1" "$2" "$body")" \
    --data-binary @"$body" "$2"; echo; rm -f "$body"
}
if [[ "$cmd" == list-fairy || "$cmd" == upgrades || "$cmd" == approve || "$cmd" == reject ]]; then
  "$here/admin.sh" pubkey >/dev/null
  sign="${XDG_CACHE_HOME:-$HOME/.cache}/oa-admin/sign"
  case "$cmd" in
    list-fairy)
      [[ "$realm" == *@* ]] || { sed -n '2,12p' "$0"; exit 2; }
      body="$(python3 -c 'import json, sys; print(json.dumps({"name": sys.argv[3], "by": "OpenAgents", "look": "120140210f2c94c3a1f6b7b4fd6e8d8ff9be4ffc76fe0fff6c8", "sats": int(sys.argv[2]), "lightning": sys.argv[1]}))' "$realm" "${2:-100}" "${3:-First Light}")"
      signed POST "$origin/api/playtest/upgrades/fairy" "$body" ;;
    upgrades) curl -sS -H "Authorization: Nostr $("$sign" --key "$key" GET "$origin/api/admin/upgrades")" "$origin/api/admin/upgrades"; echo ;;
    approve|reject) [[ "$realm" =~ ^[0-9]{1,12}$ ]] || exit 2; signed POST "$origin/api/admin/upgrades/$realm/$cmd" '{}' ;;
  esac
  exit 0
fi
[[ "$realm" =~ ^[a-z0-9-]{1,64}$ ]] || { sed -n '2,12p' "$0"; exit 2; }
"$here/admin.sh" pubkey >/dev/null   # builds the signer when it's missing
sign="${XDG_CACHE_HOME:-$HOME/.cache}/oa-admin/sign"
case "$cmd" in
  code)
    url="$origin/api/admin/makers/$realm/code"
    body="$(mktemp)"; trap 'rm -f "$body"' EXIT; printf '{}' > "$body"
    curl -fsS -X POST -H "Content-Type: application/json" -H "Authorization: Nostr $("$sign" --key "$key" POST "$url" "$body")" \
      --data-binary @"$body" "$url" | python3 -c 'import json, sys; print(json.load(sys.stdin)["link"])' ;;
  who) curl -sS "$origin/api/playtest/makers/$realm"; echo ;;
  *) sed -n '2,12p' "$0"; exit 2 ;;
esac
