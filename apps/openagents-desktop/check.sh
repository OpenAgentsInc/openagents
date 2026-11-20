#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper around build.sh, account-check.sh and sign-check.sh; holds no logic)
# Verifies OpenAgents.app builds, account auth/NIP-98/push checks pass, and code signing checks pass (#13785).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
export BROWSER=/usr/bin/true
"$here/build.sh"
"$here/tests/account-check.sh"
"$here/tests/sign-check.sh"
echo "openagents-desktop check: ok"
