#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> docs-check"
"$ROOT/scripts/docs-check.mjs"

if command -v bun >/dev/null 2>&1; then
  has_script() {
    local dir="$1"
    local script="$2"
    node -e 'const p=require(process.argv[1]);process.exit(p?.scripts?.[process.argv[2]]?0:1)' "$dir/package.json" "$script"
  }

  for pkg in packages/dse packages/effuse packages/effuse-panes packages/effuse-test packages/lightning-effect packages/lnd-effect; do
    if [ -d "$ROOT/$pkg" ]; then
      echo "==> $pkg typecheck+test"
      if has_script "$ROOT/$pkg" typecheck; then
        (cd "$ROOT/$pkg" && bun run typecheck)
      fi

      if has_script "$ROOT/$pkg" lint:structure; then
        (cd "$ROOT/$pkg" && bun run lint:structure)
      fi

      if has_script "$ROOT/$pkg" test; then
        (cd "$ROOT/$pkg" && bun run test)
      else
        (cd "$ROOT/$pkg" && bun test)
      fi
    fi
  done
else
  echo "bun not found; skipping bun-based package tests"
fi

echo "verify: OK"
