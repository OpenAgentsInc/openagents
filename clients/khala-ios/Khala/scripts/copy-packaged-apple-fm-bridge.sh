#!/usr/bin/env bash
# Xcode build phase: bundle the Apple FM helper into a macOS Khala .app.
set -euo pipefail

if [[ "${PLATFORM_NAME:-}" != macosx* ]]; then
  exit 0
fi

if [[ -z "${TARGET_BUILD_DIR:-}" || -z "${UNLOCALIZED_RESOURCES_FOLDER_PATH:-}" ]]; then
  echo "Khala Apple FM packaging requires Xcode TARGET_BUILD_DIR and UNLOCALIZED_RESOURCES_FOLDER_PATH" >&2
  exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../../.." && pwd)"
source_helper="${KHALA_APPLE_FM_BRIDGE_HELPER_PATH:-$repo_root/apps/pylon/bin/foundation-bridge}"
dest_dir="$TARGET_BUILD_DIR/$UNLOCALIZED_RESOURCES_FOLDER_PATH/app/apple-fm-bridge"
dest_helper="$dest_dir/foundation-bridge"
unavailable_marker="$dest_dir/APPLE_FM_UNAVAILABLE.txt"

mkdir -p "$dest_dir"

if [[ "${KHALA_SKIP_APPLE_FM_BRIDGE_CHECK:-0}" == "1" ]]; then
  rm -f "$dest_helper"
  printf '%s\n' "Apple FM unavailable: KHALA_SKIP_APPLE_FM_BRIDGE_CHECK=1 at build time." > "$unavailable_marker"
  exit 0
fi

if [[ ! -f "$source_helper" ]]; then
  echo "Khala Apple FM helper missing at $source_helper" >&2
  echo "Run apps/pylon/swift/foundation-bridge/build.sh or set KHALA_APPLE_FM_BRIDGE_HELPER_PATH." >&2
  exit 1
fi

if [[ ! -s "$source_helper" ]]; then
  echo "Khala Apple FM helper is empty at $source_helper" >&2
  exit 1
fi

if [[ ! -x "$source_helper" ]]; then
  echo "Khala Apple FM helper is not executable at $source_helper" >&2
  exit 1
fi

rm -f "$unavailable_marker"
cp "$source_helper" "$dest_helper"
chmod 755 "$dest_helper"

if [[ ! -s "$dest_helper" || ! -x "$dest_helper" ]]; then
  echo "Khala Apple FM helper copy failed verification at $dest_helper" >&2
  exit 1
fi

echo "Khala Apple FM helper bundled at $dest_helper"
