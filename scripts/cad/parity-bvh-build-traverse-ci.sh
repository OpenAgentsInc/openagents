#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo run -p openagents-cad --bin parity-bvh-build-traverse -- --check
cargo test -p openagents-cad --test parity_bvh_build_traverse --quiet
