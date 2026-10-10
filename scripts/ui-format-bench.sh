#!/bin/sh
# The interactive-answer format benchmark (#11113,
# docs/research/2026-10-09-ui-format-benchmark.md): the OpenUI Lang subset
# against minified nested JSON, on our models and our catalog.
#
#   scripts/ui-format-bench.sh --model ID [--model ID]... [--runs N] [--format lang|json] [--prompt ID]...
#
# Prints a summary table and writes the JSON report to
# bench/ui-format/results/<unix>.json (or --out PATH). The door is
# CODER_DOOR_URL (default the public Vercel AI Gateway); its key is
# CODER_DOOR_KEY, CODER_AI_GATEWAY_KEY, or AI_GATEWAY_API_KEY, read from
# ai-gateway.env in $OPENAGENTS_SECRETS (default ~/work/.secrets) when
# unset. Nothing a model writes is printed.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
[ $# -gt 0 ] || { sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }

secrets=${OPENAGENTS_SECRETS:-$HOME/work/.secrets}
if [ -z "${CODER_DOOR_KEY:-}${CODER_AI_GATEWAY_KEY:-}${AI_GATEWAY_API_KEY:-}" ] && [ -f "$secrets/ai-gateway.env" ]; then
    set -a
    # shellcheck disable=SC1091
    . "$secrets/ai-gateway.env"
    set +a
fi

cd "$root"
cargo build -q --release -p coder --bin ui-format-bench
exec "${CARGO_TARGET_DIR:-$root/target}/release/ui-format-bench" "$@"
