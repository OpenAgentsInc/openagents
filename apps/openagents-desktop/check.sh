#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper around build.sh, account-check.sh and sign-check.sh; holds no logic)
# Verifies OpenAgents.app builds, account auth/NIP-98/push checks pass, code signing checks pass (#13785),
# and the Windows command-line rules hold (#13782).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
export BROWSER=/usr/bin/true
"$here/build.sh"
"$repo/coax/web/tool.sh" "$here/v1-bundle-check.coax" --platform macos --stage "${OA_V1_STAGE:-1}" "${CODER_BUILD:-$here/build}/OpenAgents.app"
"$here/tests/account-check.sh"
"$here/tests/sign-check.sh"
args_bin="$(mktemp -t oa-win-args)"
/usr/bin/clang -std=c11 -Wall -Wextra -Werror -I"$here/../../coax/platform" "$here/windows/args-test.c" -o "$args_bin"
"$args_bin"
rm -f "$args_bin"
local_bin="$(mktemp -t oa-local-models)"
xcrun -sdk macosx swiftc -O -swift-version 5 -suppress-warnings -parse-as-library -target arm64-apple-macos14.0 \
  "$here/LocalModels.swift" "$here/tests/local-models-check.swift" -framework CryptoKit -o "$local_bin"
"$local_bin"
rm -f "$local_bin"
echo "openagents-desktop check: ok"
