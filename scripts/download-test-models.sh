#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURES_DIR="$ROOT_DIR/crates/ml/tests/fixtures"

mkdir -p "$FIXTURES_DIR"

# Small (CI/local) - 1MB (ggml test model; not compatible with Candle gguf naming)
curl -fL -o "$FIXTURES_DIR/llama2c-260k.gguf" \
  "https://huggingface.co/ggml-org/test-model-stories260K/resolve/main/stories260K-f32.gguf"

# Medium (local dev) - 27MB (ggml naming, used for manual validation)
curl -fL -o "$FIXTURES_DIR/llama2c-42m-q4.gguf" \
  "https://huggingface.co/mradermacher/llama2.c-stories42M-GGUF/resolve/main/llama2.c-stories42M.Q4_K_M.gguf"

# Tokenizer
curl -fL -o "$FIXTURES_DIR/tokenizer.json" \
  "https://huggingface.co/Xenova/llama2.c-stories42M/resolve/main/tokenizer.json"

echo "Downloaded models to $FIXTURES_DIR"
