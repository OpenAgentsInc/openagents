#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles the Coax API checks to C, links sockets and TLS, runs them)
# Runs coax/web/api/tests/cli_auth.coax, the API's checks for the CLI as a caller
# (NIP-98 callers, realms/mine, device login), against one API:
#   coax/web/api/cli-auth-test.sh http://127.0.0.1:PORT [nip98|device]
#   coax/web/api/cli-auth-test.sh https://staging.openagents.com
# test.sh runs it on its throwaway API. Keys are made for the run and never printed.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"; p="$repo/coax/platform"
[[ $# -ge 1 ]] || { echo "usage: coax/web/api/cli-auth-test.sh API_URL [nip98|device]" >&2; exit 2; }
coax="${COAX:-$repo/coax/bin/coax}"
if [[ "$(uname -s)" == Darwin ]]; then cc=(/usr/bin/clang -D_DARWIN_C_SOURCE); libs=(-framework Security -framework CoreFoundation); else cc=("${CC:-gcc}" -D_DEFAULT_SOURCE); libs=(-lpthread); fi
. "$p/tls-env.sh"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/entropy-policy.h" > "$work/entropy.c"
"$coax" c "$here/tests/cli_auth.coax" --library --run-steps 250000000 > "$work/cli_auth.c"
# shellcheck disable=SC2086
"${cc[@]}" -std=c11 -O2 $CXTLS_CFLAGS -I"$work" -I"$p" -o "$work/cli_auth" "$work/cli_auth.c" \
  "$p/tool-native.c" "$p/process-native.c" "$p/store-file-native.c" "$p/entropy-native.c" "$p/entropy-foreign.c" \
  "$p/socket-native.c" "$p/tls-native.c" "$work/entropy.c" "${libs[@]}" $CXTLS_LIBS -lm
API="$1" ONLY="${2:-}" "$work/cli_auth"
