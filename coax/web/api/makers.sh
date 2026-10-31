#!/usr/bin/env bash
# The admin's side of realm makers (makers.c, docs/verse/makers.md), on this machine: signs a
# NIP-98 request with the admin's key (admin.sh's signer and key) and prints the answer.
#
#   makers.sh code [--staging] REALM     a new one-time claim code; prints the claim link
#   makers.sh who  [--staging] REALM     the realm's maker, as anyone sees it
#
# OA_ADMIN_KEY names the key file (default ~/work/.secrets/openagents-analytics-admin.key).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
key="${OA_ADMIN_KEY:-$HOME/work/.secrets/openagents-analytics-admin.key}"
cmd="${1:-}"; shift || true
origin=https://openagents.com
if [[ "${1:-}" == --staging ]]; then origin=https://staging.openagents.com; shift; fi
realm="${1:-}"
[[ "$realm" =~ ^[a-z0-9-]{1,64}$ ]] || { sed -n '2,7p' "$0"; exit 2; }
"$here/admin.sh" pubkey >/dev/null   # builds the signer when it's missing
sign="${XDG_CACHE_HOME:-$HOME/.cache}/oa-admin/sign"
case "$cmd" in
  code)
    url="$origin/api/admin/makers/$realm/code"
    body="$(mktemp)"; trap 'rm -f "$body"' EXIT; printf '{}' > "$body"
    curl -fsS -X POST -H "Content-Type: application/json" -H "Authorization: Nostr $("$sign" --key "$key" POST "$url" "$body")" \
      --data-binary @"$body" "$url" | python3 -c 'import json, sys; print(json.load(sys.stdin)["link"])' ;;
  who) curl -sS "$origin/api/playtest/makers/$realm"; echo ;;
  *) sed -n '2,7p' "$0"; exit 2 ;;
esac
