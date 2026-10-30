#!/usr/bin/env bash
# Effects-only client: reuse the compatibility API's existing sign.c/NIP-98
# format, then POST exact body bytes. No Pylon key mode or new auth scheme.
set -euo pipefail
umask 077
here="$(cd "$(dirname "$0")" && pwd)"
origin=https://openagents.com key= body= fresh=false
usage() { echo 'usage: mesh-client.sh [--staging] (--new-demo-key | --key FILE) BODY.json'; }
while (($#)); do
  case "$1" in
    --staging) origin=https://staging.openagents.com; shift ;;
    --new-demo-key) fresh=true; shift ;;
    --key) [[ $# -ge 2 && -z "$key" ]] || { usage >&2; exit 2; }; key=$2; shift 2 ;;
    --help|-h) usage; exit ;;
    --*) usage >&2; exit 2 ;;
    *) [[ -z "$body" ]] || { usage >&2; exit 2; }; body=$1; shift ;;
  esac
done
[[ -f "$body" && ( $fresh == true || -n "$key" ) ]] || { usage >&2; exit 2; }
[[ $fresh != true || -z "$key" ]] || { echo 'mesh: choose an existing key or a disposable demo key' >&2; exit 2; }
[[ $(wc -c < "$body") -le 16384 ]] || { echo 'mesh: body exceeds 16 KiB' >&2; exit 2; }
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
sign=${OA_MESH_SIGNER:-${XDG_CACHE_HOME:-$HOME/.cache}/oa-mesh/sign}
if [[ ! -x "$sign" || "$here/sign.c" -nt "$sign" ]]; then
  [[ -z ${OA_MESH_SIGNER:-} ]] || { echo 'mesh: explicit signer is missing or older than sign.c' >&2; exit 2; }
  if [[ $(uname -s) == Darwin ]]; then
    secp=$(brew --prefix secp256k1); ssl=$(brew --prefix openssl@3)
    /usr/bin/clang -std=c11 -O2 -Wall -Wextra -Werror -D_DEFAULT_SOURCE -I"$secp/include" -I"$ssl/include" \
      "$here/sign.c" -L"$secp/lib" -lsecp256k1 -L"$ssl/lib" -lcrypto -o "$work/sign"
  else
    # pkg-config supplies only installed compiler/linker flags, never a key.
    read -r -a libs <<< "$(pkg-config --cflags --libs libsecp256k1 openssl)"
    "${COAX_CC:-cc}" -std=c11 -O2 -Wall -Wextra -Werror -D_DEFAULT_SOURCE "$here/sign.c" "${libs[@]}" -o "$work/sign"
  fi
  mkdir -p "$(dirname "$sign")"; mv "$work/sign" "$sign"
fi
if $fresh; then key="$work/demo.key"; "$sign" --new "$key" >/dev/null; fi
[[ -f "$key" ]] || { echo 'mesh: key file is missing' >&2; exit 2; }
url="$origin/api/v1/systemone"
auth=$("$sign" --key "$key" POST "$url" "$body")
status=$(curl --silent --show-error --max-time 40 --proto '=https' --request POST \
  --header "Authorization: Nostr $auth" --header 'Content-Type: application/json' \
  --data-binary "@$body" --output "$work/result" --write-out '%{http_code}' "$url")
unset auth
if [[ $status != 200 ]]; then cat "$work/result" >&2; printf '\nmesh: HTTP%s\n' "$status" >&2; exit 1; fi
cat "$work/result"; printf '\n'
