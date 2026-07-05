#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

SOURCE="${1:-assets/images/icon.png}"
OUT_DIR="assets/images"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

if [[ ! -f "$SOURCE" ]]; then
  echo "source image not found: $SOURCE" >&2
  exit 1
fi

# Local-only asset generation for Expo prebuild/native tooling. This uses
# macOS `sips`, which is already present on the iOS build machine, and avoids
# EAS/cloud generation lanes.
sips -s format png -z 1024 1024 "$SOURCE" --out "$TMP_DIR/icon.png" >/dev/null
sips -s format png -z 1024 1024 "$SOURCE" --out "$TMP_DIR/adaptive-icon.png" >/dev/null
sips -s format png -z 512 512 "$SOURCE" --out "$TMP_DIR/splash-icon.png" >/dev/null

mkdir -p "$OUT_DIR"
cp "$TMP_DIR/icon.png" "$OUT_DIR/icon.png"
cp "$TMP_DIR/adaptive-icon.png" "$OUT_DIR/adaptive-icon.png"
cp "$TMP_DIR/splash-icon.png" "$OUT_DIR/splash-icon.png"

echo "Generated Khala Mobile assets in $OUT_DIR"
