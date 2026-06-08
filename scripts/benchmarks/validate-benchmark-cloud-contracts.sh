#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../.."

cargo test -p benchmark-cloud
cargo run -p benchmark-cloud --example probe_gepa_stage0_smoke >/dev/null
cargo run -p benchmark-cloud --example probe_gepa_stage1_retained_sprint >/dev/null
cargo run -p benchmark-cloud --example probe_gepa_validation_sweep >/dev/null
