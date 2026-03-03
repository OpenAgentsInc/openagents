#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo run -p openagents-cad --bin parity-drafting-kernel-scaffolding -- --check
cargo test -p openagents-cad --test parity_drafting_kernel_scaffolding --quiet
