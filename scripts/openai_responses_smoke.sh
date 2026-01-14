#!/usr/bin/env bash
set -euo pipefail

if [[ -f ".env.local" ]]; then
  set -a
  # shellcheck disable=SC1091
  source ".env.local"
  set +a
fi

if [[ -z "${OPENAI_API_KEY:-}" ]]; then
  echo "OPENAI_API_KEY is required. Set it or add it to .env.local." >&2
  exit 1
fi

export OPENAI_MODEL="${OPENAI_MODEL:-gpt-5-nano}"
export OPENAI_MAX_TOKENS="${OPENAI_MAX_TOKENS:-2048}"

echo "Running OpenAI Responses streaming example with model: ${OPENAI_MODEL}"
cargo run -p dsrs --example 16-openai-responses-stream
