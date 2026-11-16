#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles the native services and their Swift check with swiftc and runs it)
# Checks a compiled app's native services on the Mac, with no simulator, no Keychain and no real
# server: AppHTTP, AppAccount (over AppAuth), AppWake, AppEffects and AppCoreBridge (against a stand-in core).
#   apps/shared/native/app-services-check.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
apple="$here/../host/apple"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
xcrun swiftc -swift-version 5 -parse-as-library -o "$work/check" "$here/AppHTTP.swift" "$here/AppAccount.swift" "$here/AppWake.swift" "$here/AppEffects.swift" "$here/AppCoreBridge.swift" \
  "$apple/AppAuth.swift" "$apple/DeviceKey.swift" "$here/app-services-check.swift" 2> "$work/build.log" || { cat "$work/build.log" >&2; exit 1; }
# The services must not link a Keychain item call on the Mac: a check may never raise a password prompt.
if nm -u "$work/check" | grep -E '_SecItem(Add|CopyMatching|Update|Delete)$'; then echo "app-services-check: the check links the Keychain" >&2; exit 1; fi
perl -e 'alarm 120; exec @ARGV' "$work/check"
