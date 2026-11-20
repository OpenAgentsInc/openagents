#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around swiftc; holds no logic)
# Checks one-time UserDefaults migration from com.openagents.coder to com.openagents.desktop (#13784).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d /tmp/defaults-migration-test.XXXXXX)"
trap 'rm -rf "$tmp"' EXIT
xcrun -sdk macosx swiftc -swift-version 5 -suppress-warnings -parse-as-library -target arm64-apple-macos14.0 \
  "$here/DefaultsMigration.swift" "$here/defaults-migration-test.swift" -o "$tmp/test"
"$tmp/test"
