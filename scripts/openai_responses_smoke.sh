#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${OPENAI_API_KEY:-}" ]]; then
  echo "OPENAI_API_KEY is required." >&2
  exit 1
fi

export OPENAI_MODEL="${OPENAI_MODEL:-gpt-5-nano}"

echo "Running OpenAI Responses streaming example with model: ${OPENAI_MODEL}"
cargo run -p dsrs --example 16-openai-responses-stream
