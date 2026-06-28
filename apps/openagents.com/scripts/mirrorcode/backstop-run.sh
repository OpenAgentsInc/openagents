#!/usr/bin/env bash
#
# backstop-run.sh — MirrorCode gym backstop runner (issue #6710).
#
# The fleet-saturation prio:4 (`prio:4-backstop-burn`) standing task invokes this
# to do GENUINE own-capacity ($0) high-density work whenever the higher tiers are
# clear, so no fleet slot ever idles. It runs a bounded batch of small coding
# problems through a model (Khala, OpenAI-compatible, own capacity), executes the
# generated solutions against hidden test cases, computes pass rates, and writes
# execution traces.
#
# This is the LIGHTWEIGHT density burner. The full Docker MirrorCode harness
# (./run.sh) burns >=1B tokens per sample and is NOT backstop-appropriate; this
# runner uses the public-domain fixture problem set (NOT MirrorCode tasks, so no
# benchmark contamination). The read-only MirrorCode clone is detected and noted
# in the result when present; wiring the full harness per-task into the backstop
# is a documented follow-up (see README.md).
#
# Usage:
#   ./backstop-run.sh                 # mints a free Khala key, runs the bounded batch
#   OPENAI_API_KEY=oa_agent_xxx ./backstop-run.sh --limit 8
#
# Env knobs (all optional):
#   OPENAI_API_KEY   a Khala key; if unset a free one is minted
#   OPENAI_BASE_URL  Khala OpenAI-compatible base (default https://openagents.com/api/v1)
#   MC_BACKSTOP_LIMIT      max problems in the batch (default 8; 0 = all)
#   MC_BACKSTOP_OUT       results dir (default ./results/backstop)
#   MC_BACKSTOP_MODEL     model id (default openagents/khala)
#   MC_BACKSTOP_DRY_RUN   set to 1 to skip the live model (diagnostic; no spend)
#   MC_BACKSTOP_SMOKE     set to 1 for a burn smoke only (one live call; nonzero on 0-burn)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PYTHON_BIN="${MC_PYTHON:-python3}"
command -v "$PYTHON_BIN" >/dev/null 2>&1 || { echo "ERROR: '$PYTHON_BIN' not on PATH" >&2; exit 2; }

OUT="${MC_BACKSTOP_OUT:-${SCRIPT_DIR}/results/backstop}"
mkdir -p "$OUT"

# Diagnostic dry-run: exercise the pipeline with no model call / no spend.
if [[ "${MC_BACKSTOP_DRY_RUN:-0}" == "1" ]]; then
  echo "Backstop DRY-RUN (no model call, no spend)..."
  exec "$PYTHON_BIN" "${SCRIPT_DIR}/backstop_eval.py" \
    --limit "${MC_BACKSTOP_LIMIT:-8}" --out "$OUT" --model "${MC_BACKSTOP_MODEL:-openagents/khala}" "$@"
fi

export OPENAI_BASE_URL="${OPENAI_BASE_URL:-https://openagents.com/api/v1}"
if [[ -z "${OPENAI_API_KEY:-}" ]]; then
  command -v curl >/dev/null 2>&1 || { echo "ERROR: 'curl' needed to mint a free Khala key" >&2; exit 2; }
  echo "Minting a free Khala key (own-capacity, \$0)..."
  base="${OPENAI_BASE_URL%/api/v1}"
  OPENAI_API_KEY="$(curl -fsS -A "openagents-mirrorcode-backstop/1.0" -X POST "${base}/api/keys/free" 2>/dev/null | "$PYTHON_BIN" -c 'import sys,json;
try:
    print(json.load(sys.stdin).get("credential",{}).get("token",""))
except Exception:
    pass' )"
  if [[ -z "${OPENAI_API_KEY}" ]]; then
    echo "ERROR: failed to mint a free Khala key." >&2
    exit 2
  fi
fi
export OPENAI_API_KEY
echo "Khala endpoint   : ${OPENAI_BASE_URL} (key ${OPENAI_API_KEY:0:12}...)"
# BURN SMOKE (issue #6735): one live own-capacity call that FAILS LOUD (nonzero)
# if it does not actually burn tokens. Use before trusting the backstop for burn.
if [[ "${MC_BACKSTOP_SMOKE:-0}" == "1" ]]; then
  echo "Running backstop burn smoke (one live own-capacity call)..."
  exec "$PYTHON_BIN" "${SCRIPT_DIR}/backstop_eval.py" --smoke \
    --model "${MC_BACKSTOP_MODEL:-openagents/khala}" "$@"
fi

echo "Launching gym backstop (own-capacity \$0 high-density fixture eval)..."

# The Python runner runs a burn PREFLIGHT before the batch and exits nonzero
# (fail loud) if the own-capacity path is unauthorized / WAF-blocked / 0-burn;
# `exec` propagates that exit code so this script never silently reports a
# successful 0-burn run.
exec "$PYTHON_BIN" "${SCRIPT_DIR}/backstop_eval.py" --live \
  --limit "${MC_BACKSTOP_LIMIT:-8}" --out "$OUT" --model "${MC_BACKSTOP_MODEL:-openagents/khala}" "$@"
