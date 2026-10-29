#!/usr/bin/env bash
# The admin's side of openagents.com/admin/analytics, on this machine: signs
# a NIP-98 request with the admin's Nostr key (sign.c; the key stays in its
# file, 64 hex digits, chmod 600) and either opens the dashboard signed in or
# fetches a report. The server lets in only the keys in admins.conf.
#
#   admin.sh open [--staging]            sign in and open the dashboard in the default browser
#   admin.sh url [--staging]             the one-time sign-in link (good for 60 s), to open anywhere
#   admin.sh chat [--staging]            sign in and open /chat (docs/chat.md)
#   admin.sh chat-url [--staging]        the same link, to open anywhere
#   admin.sh get [--staging] QUERY       a report as text, e.g. "view=overview&days=7"
#   admin.sh pubkey                      the key's public key, for admins.conf
#
# OA_ADMIN_KEY names the key file (default ~/work/.secrets/openagents-analytics-admin.key).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
key="${OA_ADMIN_KEY:-$HOME/work/.secrets/openagents-analytics-admin.key}"
cmd="${1:-open}"; shift || true
origin=https://openagents.com
if [[ "${1:-}" == --staging ]]; then origin=https://staging.openagents.com; shift; fi
sign="${XDG_CACHE_HOME:-$HOME/.cache}/oa-admin/sign"
if [[ ! -x "$sign" || "$here/sign.c" -nt "$sign" ]]; then
  mkdir -p "$(dirname "$sign")"
  secp="$(brew --prefix secp256k1)"; ssl="$(brew --prefix openssl@3)"
  /usr/bin/clang -std=c11 -O2 -Wall -Wextra -Werror -D_DEFAULT_SOURCE -I"$secp/include" -I"$ssl/include" \
    -o "$sign" "$here/sign.c" -L"$secp/lib" -lsecp256k1 -L"$ssl/lib" -lcrypto
fi
[[ -f "$key" ]] || { echo "admin.sh: no key at $key (make one: $sign --new $key, then add its public key to $here/admins.conf)" >&2; exit 1; }
link() {
  local auth; auth="$("$sign" --key "$key" GET "$origin/api/admin/login")"
  printf '%s/api/admin/login?auth=%s\n' "$origin" "$(python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "$auth")"
}
case "$cmd" in
  pubkey) "$sign" --key "$key" --pubkey ;;
  url) link ;;
  open) open "$(link)" ;;
  chat) open "$(link)&next=chat" ;;
  chat-url) printf '%s&next=chat\n' "$(link | tr -d '\n')" ;;
  get)
    url="$origin/api/admin/analytics?${1:-view=overview}"
    curl -fsS -H "Authorization: Nostr $("$sign" --key "$key" GET "$url")" "$url" ;;
  *) sed -n '2,13p' "$0"; exit 2 ;;
esac
