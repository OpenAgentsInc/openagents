#!/usr/bin/env bash
# Run in a source checkout on an expiring GCP GPU host allocated through cloud-budget.py.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
sudo apt-get update -qq
sudo apt-get install -y -qq build-essential pkg-config libssl-dev python3-venv curl
python3 -m venv "$HOME/flint-venv"
"$HOME/flint-venv/bin/pip" install --quiet huggingface_hub
if ! [ -x "$HOME/.cargo/bin/rustup" ]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
fi
export PATH="$HOME/.cargo/bin:/usr/local/cuda/bin:$PATH"
rustup toolchain install 1.97.1 --profile minimal
"$HOME/flint-venv/bin/python" -u "$repo/scripts/psionic/flint-download.py" "$HOME/flint-weights" > "$HOME/flint-download.log" 2>&1 &
download_pid=$!
cd "$repo/crates/psionic"
export CARGO_TARGET_DIR="$HOME/work/agent-targets/openagents-crates-psionic" CARGO_INCREMENTAL=0
export CUDA_COMPUTE_CAP=120 CUDA_ROOT=/usr/local/cuda CARGO_BUILD_JOBS=12
export LD_LIBRARY_PATH="/usr/local/cuda/lib64:${LD_LIBRARY_PATH:-}"
. "$repo/scripts/dev/cargo-build-env.sh"
cargo build --release -p psionic-flint --features cuda
cargo test --release -p psionic-flint --features cuda
wait "$download_pid"
sha256sum "$CARGO_TARGET_DIR/release/psionic-flint"
echo 'Flint build and checkpoint ready.'
