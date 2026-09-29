#!/usr/bin/env bash
# Build or refresh the chat router's codebase knowledge index.
#
# Reads this public repository at a pinned commit (Git objects only, never
# the working tree), chunks its Markdown docs and Rust doc comments, embeds
# every chunk the previous index does not already hold, and writes one
# gzip-compressed file. See docs/coder/design/codebase-kb.md.
#
# usage: scripts/build-codebase-kb.sh [COMMIT] [OUT]
#   COMMIT  default: origin/main after a fetch
#   OUT     default: $CODER_CODEBASE_KB, else ~/.cache/openagents/codebase-kb/codebase-kb.gz
#
# Embeddings: CODER_AI_GATEWAY_KEY or CODER_DOOR_KEY (the chat worker's own
# door key), else OPENAI_API_KEY, else OpenRouter. The key is read from the
# environment and never printed.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
commit="${1:-}"
if [[ -z "$commit" ]]; then
  git -C "$root" fetch --quiet origin main
  commit="origin/main"
fi
out="${2:-${CODER_CODEBASE_KB:-$HOME/.cache/openagents/codebase-kb/codebase-kb.gz}}"

cargo build --quiet --release --manifest-path "$root/Cargo.toml" -p coder --bin codebase-kb
target="${CARGO_TARGET_DIR:-$root/target}"
"$target/release/codebase-kb" build --repo "$root" --commit "$commit" --out "$out" --previous "$out"
