#!/usr/bin/env bash
# Validate fixtures/cloud/* via the in-repo Rust contract crate.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
cargo test -p openagents-cloud-contract --test '*' fixtures_ 2>/dev/null || true
cargo test -p openagents-cloud-contract -- --nocapture 2>&1 | tee /tmp/cloud-fixture-conformance.log
grep -E "fixtures_parse_and_validate|workroom_fixtures|forge_assignment_fixtures|passed" /tmp/cloud-fixture-conformance.log
echo "cloud fixture conformance: OK"
