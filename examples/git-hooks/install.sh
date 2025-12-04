#!/usr/bin/env bash
set -euo pipefail

# Install sample git hooks for task validation.

ROOT=$(git rev-parse --show-toplevel)
HOOK_DIR="$(git rev-parse --git-dir)/hooks"
SRC_DIR="$ROOT/examples/git-hooks"

echo "[git-hooks] Installing hooks into $HOOK_DIR"
mkdir -p "$HOOK_DIR"

for hook in pre-commit post-merge pre-push; do
  cp "$SRC_DIR/$hook" "$HOOK_DIR/$hook"
  chmod +x "$HOOK_DIR/$hook"
  echo "  - installed $hook"
done

echo "[git-hooks] Done."
