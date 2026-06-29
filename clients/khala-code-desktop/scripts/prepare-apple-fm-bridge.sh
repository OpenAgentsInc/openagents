#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
desktop_root="$(cd "$script_dir/.." && pwd)"
repo_root="$(cd "$desktop_root/../.." && pwd)"
bridge_root="$repo_root/apps/pylon/swift/foundation-bridge"
bridge_binary="$bridge_root/.build/release/foundation-bridge"
target_dir="$desktop_root/resources/apple-fm-bridge"
target="$target_dir/foundation-bridge"

bash "$bridge_root/build.sh"

if [[ ! -x "$bridge_binary" ]]; then
  echo "foundation-bridge build did not produce an executable at $bridge_binary" >&2
  exit 1
fi

mkdir -p "$target_dir"
cp "$bridge_binary" "$target"
chmod 755 "$target"
printf 'Khala Desktop Apple FM bridge prepared: %s\n' "$target"
