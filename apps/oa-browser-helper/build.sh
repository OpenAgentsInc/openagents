#!/usr/bin/env bash
# Firm reason: 5 (thin standalone build wrapper for the out-of-process signed oa-browser-helper target; #14109).
# Keeps OpenAgents Desktop base & --minimal installers 100% free of CEF libraries.
#   apps/oa-browser-helper/build.sh [--self-test] [--manifest]
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
work="$here/build"
mkdir -p "$work"

bin="$work/oa-browser-helper"
cc_bin="${CC:-/usr/bin/clang}"
if ! command -v "$cc_bin" >/dev/null 2>&1; then cc_bin="cc"; fi

extra_flags=()
if [[ -n "${OA_CEF_ROOT:-}" && -d "$OA_CEF_ROOT" ]]; then
  extra_flags+=(-DOA_WITH_CEF -I"$OA_CEF_ROOT/include" -L"$OA_CEF_ROOT/Release" -lcef)
fi

"$cc_bin" -std=gnu11 -O2 -Wall -Wextra -Werror \
  -I"$here" -I"$repo/apps/openagents-desktop" \
  "$here/main.c" "$repo/apps/openagents-desktop/br_host.c" \
  ${extra_flags[@]+"${extra_flags[@]}"} \
  -o "$bin"

sha="$(shasum -a 256 "$bin" | awk '{print $1}')"
arch="$(uname -m)"
os="$(uname -s | tr '[:upper:]' '[:lower:]')"
[[ "$os" == "darwin" ]] && os="macos"
platform="${os}-${arch}"

cat > "$work/manifest.json" <<EOF
{
  "version": "1.0.0",
  "platform": "${platform}",
  "binary": "oa-browser-helper",
  "sha256": "${sha}",
  "install_dir": "~/.openagents/engines/browser/"
}
EOF

echo "built $bin (platform=${platform}, sha256=${sha})"
for arg in "$@"; do
  if [[ "$arg" == "--self-test" ]]; then
    "$bin" --self-test
  fi
done
