#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo run -p openagents-cad --bin parity-embroidery-dst-pes-lane -- --check
cargo test -p openagents-cad --test parity_embroidery_dst_pes_lane --quiet
