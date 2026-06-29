#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

rm -f openagentsinc-pylon-*.tgz

echo "== unit and runtime tests =="
bun run test

echo "== bootstrap/status/inventory/operator smokes =="
PYLON_HOME="${TMPDIR:-/tmp}/pylon-release-gate-bootstrap" bun src/index.ts bootstrap --json >/tmp/pylon-release-bootstrap.json
PYLON_HOME="${TMPDIR:-/tmp}/pylon-release-gate-status" bun src/index.ts status --json >/tmp/pylon-release-status.json
bun src/index.ts inventory --json >/tmp/pylon-release-inventory.json
PYLON_HOME="${TMPDIR:-/tmp}/pylon-release-gate-operator" bun src/index.ts operator snapshot --json >/tmp/pylon-release-operator.json

echo "== headless node startup smoke =="
bun run smoke:default-start

echo "== package dry-run =="
bun pm pack --dry-run >/tmp/pylon-release-pack.log
rm -f openagentsinc-pylon-*.tgz

echo "== local package install smoke =="
bun run smoke:install:local
rm -f openagentsinc-pylon-*.tgz

echo "release gate passed"
