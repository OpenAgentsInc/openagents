#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

chmod +x "$ROOT_DIR/scripts/local-ci.sh"
chmod +x "$ROOT_DIR/scripts/install-git-hooks.sh"
chmod +x "$ROOT_DIR/.githooks/pre-commit"
chmod +x "$ROOT_DIR/.githooks/pre-push"

git -C "$ROOT_DIR" config core.hooksPath .githooks

echo "Local git hooks installed."
echo "hooksPath=$(git -C "$ROOT_DIR" config --get core.hooksPath)"
