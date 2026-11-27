#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles Account.swift, AppAuth.swift, DeviceKey.swift and the check with swiftc, and runs it)
# Checks the Mac app's v1 account login (Account.swift on apps/shared/host/apple/AppAuth.swift):
# offline against a fake API (WorkOS Magic Auth/Password/PKCE/SSO/Orgs, email code, 0600 tokens,
# security, push target, freeze, logout, transient-refresh preservation). Everything it keeps is
# in a temporary folder; the binary must not link a Keychain call, so nothing can prompt.
#   apps/openagents-desktop/tests/account-check.sh
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
repo="$(cd "$here/../.." && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/account-check.XXXXXX")"; trap 'rm -rf "$work"' EXIT
export BROWSER=/usr/bin/true
xcrun -sdk macosx swiftc -O -suppress-warnings -parse-as-library \
  "$here/Account.swift" "$repo/apps/shared/host/apple/AppAuth.swift" "$repo/apps/shared/host/apple/DeviceKey.swift" "$repo/apps/shared/native/KitPasskey.swift" "$here/tests/account-check.swift" \
  -o "$work/account-check" 2> "$work/build.log" || { cat "$work/build.log" >&2; exit 1; }
if nm -u "$work/account-check" | grep -E '_SecItem(Add|CopyMatching|Update|Delete)$'; then echo "account-check: the Mac's login links the Keychain" >&2; exit 1; fi
mkdir -p "$work/offline"
perl -e 'alarm 60; exec @ARGV' "$work/account-check" offline "$work/offline"
echo "account-check: ok"
