#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around swiftc and the compiled check; holds no logic)
# Checks each chat's own worktree (GitHub #13758) under a throwaway home folder: apps/coder-desktop/worktree-test.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
t="${TMPDIR:-/tmp}"; tmp="$(mktemp -d "${t%/}/wt-test.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/home" "$tmp/work"
xcrun -sdk macosx swiftc -swift-version 5 -suppress-warnings -target arm64-apple-macos14.0 \
  "$here/Worktree.swift" "$here/Store.swift" "$here/Foreman.swift" "$here/ForemanOperator.swift" "$here/worktree-test.swift" -o "$tmp/test"
HOME="$tmp/home" CFFIXED_USER_HOME="$tmp/home" WT_TEST_DIR="$tmp" CODER_WORKTREES= "$tmp/test"
