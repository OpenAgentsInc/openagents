#!/usr/bin/env bash
# Runs ON the TPU VM: Python 3.12 venv with SGL-JAX (pins jax[tpu]), qwix, HF tools.
set -euo pipefail
T0=$(date +%s)
command -v uv >/dev/null || { curl -LsSf https://astral.sh/uv/install.sh | sh >/dev/null 2>&1; }
export PATH=$HOME/.local/bin:$PATH
[ -d ~/sglang-jax ] || git clone -q https://github.com/sgl-project/sglang-jax ~/sglang-jax
[ -d ~/venv ] || uv venv -q -p 3.12 ~/venv
source ~/venv/bin/activate
uv pip install -q -e "$HOME/sglang-jax/python[tpu]" 2>&1 | tail -2
uv pip install -q qwix "huggingface_hub[hf_transfer]" 2>&1 | tail -2 || true
uv pip install -q torch torchvision --index-url https://download.pytorch.org/whl/cpu 2>&1 | tail -2 || true
python -c "import jax; print(jax.__version__, jax.devices())" || true
echo "{\"setup_seconds\": $(( $(date +%s) - T0 )), \"sglang_jax_commit\": \"$(git -C ~/sglang-jax rev-parse HEAD)\"}" > ~/setup.json
cat ~/setup.json
