#!/usr/bin/env bash
# Check scripts/link-device.sh's deprecation notice without a build.
set -euo pipefail
source_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$(mktemp -d "${TMPDIR:-/tmp}/link-device-test.XXXXXX")"
trap 'rm -rf "$fixture"' EXIT
unset OPENAGENTS_LINK_NOTICE_SHOWN
"$source_dir/scripts/link-device.sh" --help >"$fixture/stdout" 2>"$fixture/stderr"
grep -q '^scripts/link-device.sh is deprecated' "$fixture/stderr"
grep -q 'OpenAgents desktop app' "$fixture/stderr"
grep -qF 'openagents connect' "$fixture/stderr"
grep -q '^Deprecated (issue #9978)' "$fixture/stdout"
# A caller that already showed it is not shown it twice.
OPENAGENTS_LINK_NOTICE_SHOWN=1 "$source_dir/scripts/link-device.sh" --help >/dev/null 2>"$fixture/stderr"
test ! -s "$fixture/stderr"
echo "test-link-device: ok"
