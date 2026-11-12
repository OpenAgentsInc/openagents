#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles lab-sign.coax with coax, then Account.swift, AppAuth.swift, DeviceKey.swift and the check with swiftc, and runs it)
# Checks the Mac app's account login (Account.swift on apps/shared/host/apple/AppAuth.swift, #13229):
# offline against a fake API, then, with --live ORIGIN, a key login against that API (staging), a
# refresh with the device key's proof and a reused refresh token ending the login. Everything it
# keeps is in a temporary folder; the binary must not link a Keychain call, so nothing can prompt.
#   apps/openagents-desktop/tests/account-check.sh [--live https://staging.openagents.com]
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
work="$(mktemp -d "${TMPDIR:-/tmp}/account-check.XXXXXX")"; trap 'rm -rf "$work"' EXIT
export BROWSER=/usr/bin/true
"$coax" c "$here/lab-sign.coax" --run-steps 250000000 --library --header "$work/lab-sign.h" > "$work/lab-sign.c"
/usr/bin/clang -std=gnu11 -O2 -w -I"$work" -c "$work/lab-sign.c" -o "$work/lab-sign.o"
xcrun -sdk macosx swiftc -O -suppress-warnings -parse-as-library -import-objc-header "$here/account-mac.h" \
  "$here/Account.swift" "$repo/apps/shared/host/apple/AppAuth.swift" "$repo/apps/shared/host/apple/DeviceKey.swift" "$here/tests/account-check.swift" \
  "$work/lab-sign.o" -o "$work/account-check" 2> "$work/build.log" || { cat "$work/build.log" >&2; exit 1; }
if nm -u "$work/account-check" | grep -E '_SecItem(Add|CopyMatching|Update|Delete)$'; then echo "account-check: the Mac's login links the Keychain" >&2; exit 1; fi
mkdir -p "$work/offline" "$work/live"
perl -e 'alarm 60; exec @ARGV' "$work/account-check" offline "$work/offline"
if [[ "${1:-}" == --live ]]; then perl -e 'alarm 120; exec @ARGV' "$work/account-check" live "$work/live" "${2:?--live ORIGIN}"; fi
echo "account-check: ok"
