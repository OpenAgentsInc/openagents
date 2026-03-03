#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo run -p openagents-cad --bin parity-trimmed-surface-ray-hit -- --check
cargo test -p openagents-cad --test parity_trimmed_surface_ray_hit --quiet
