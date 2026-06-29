#!/usr/bin/env bash
#
# MirrorCode x Khala Phase-0 runner (epic #6376 / issue #6377).
#
# Sets up a THROWAWAY Python/Inspect venv from the read-only MirrorCode clone's
# deps, points Inspect's OpenAI provider at Khala (OpenAI-compatible, zero
# provider code -- Option A), runs ONE chosen public task with hard token +
# wall-clock caps, and writes a public-safe result JSON in the shared gym
# contract. See README.md.
#
# It does NOT modify the MirrorCode clone, never commits the venv, and never
# trains/RAGs on tasks (respect both canary strings).
#
# Requirements: Docker (running), uv, python3.13, jq, curl.
#
# Usage:
#   ./run.sh                         # smoke cal_python (S target), mints a free Khala key
#   MC_TASK_ID=uuidparse_python ./run.sh
#   OPENAI_API_KEY=oa_agent_xxx ./run.sh --task numfmt_python
#
# Env knobs (all optional):
#   MC_CLONE        path to the MirrorCode clone (default: projects/repos/MirrorCode)
#   OPENAI_API_KEY  a Khala key; if unset a free one is minted
#   MC_TASK_ID      sample id '<target>_<language>' (default: cal_python)
#   MC_TOKEN_LIMIT  hard token cap (default 20000000)
#   MC_TIME_LIMIT   hard wall-clock cap seconds (default 7200)
#   MC_MESSAGE_LIMIT hard message cap (default 250)
#   MC_OUT          result JSON path (default: ./mirrorcode-phase0-result.json)
#   MC_VENV         venv dir (default: a fresh mktemp dir; auto-removed)
#   MC_KEEP_VENV    set to 1 to keep the venv after the run

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# --- locate the read-only MirrorCode clone -------------------------------------
DEFAULT_CLONE="${HOME}/work/projects/repos/MirrorCode"
MC_CLONE="${MC_CLONE:-$DEFAULT_CLONE}"
if [[ ! -f "${MC_CLONE}/mc/task.py" ]]; then
  echo "ERROR: MirrorCode clone not found at MC_CLONE=${MC_CLONE}" >&2
  echo "       Set MC_CLONE to the read-only clone (mc/task.py must exist)." >&2
  exit 2
fi
echo "MirrorCode clone : ${MC_CLONE} (read-only)"

# --- preflight tooling ---------------------------------------------------------
for bin in uv curl jq; do
  command -v "$bin" >/dev/null 2>&1 || { echo "ERROR: '$bin' not on PATH" >&2; exit 2; }
done
PYTHON_BIN="${MC_PYTHON:-python3.13}"
command -v "$PYTHON_BIN" >/dev/null 2>&1 || PYTHON_BIN="3.13"  # let uv resolve
if ! docker info >/dev/null 2>&1; then
  echo "WARNING: 'docker info' failed. MirrorCode builds + runs four containers" >&2
  echo "         per sample; the run will error without a working Docker daemon." >&2
fi

# --- Khala key + endpoint ------------------------------------------------------
if [[ -z "${OPENAI_API_KEY:-}" ]]; then
  echo "Minting a free Khala key..."
  OPENAI_API_KEY="$(curl -s -X POST https://openagents.com/api/keys/free | jq -r .credential.token)"
  if [[ -z "${OPENAI_API_KEY}" || "${OPENAI_API_KEY}" == "null" ]]; then
    echo "ERROR: failed to mint a free Khala key." >&2
    exit 2
  fi
fi
export OPENAI_API_KEY
export OPENAI_BASE_URL="${OPENAI_BASE_URL:-https://openagents.com/api/v1}"
echo "Khala endpoint   : ${OPENAI_BASE_URL} (key ${OPENAI_API_KEY:0:12}...)"

# --- throwaway venv ------------------------------------------------------------
CLEANUP_VENV=0
if [[ -n "${MC_VENV:-}" ]]; then
  VENV_DIR="${MC_VENV}"
else
  VENV_DIR="$(mktemp -d "${TMPDIR:-/tmp}/mc-khala-venv.XXXXXX")"
  [[ "${MC_KEEP_VENV:-0}" == "1" ]] || CLEANUP_VENV=1
fi
cleanup() { [[ "${CLEANUP_VENV}" == "1" ]] && rm -rf "${VENV_DIR}" || true; }
trap cleanup EXIT

if [[ ! -x "${VENV_DIR}/bin/python" ]]; then
  echo "Creating throwaway venv at ${VENV_DIR}..."
  uv venv --python "${PYTHON_BIN}" "${VENV_DIR}" >/dev/null
  echo "Installing Inspect + MirrorCode runtime deps (throwaway, not committed)..."
  VIRTUAL_ENV="${VENV_DIR}" uv pip install --quiet --python "${VENV_DIR}/bin/python" \
    "inspect-ai==0.3.217" environs jsonlines openai anthropic python-levenshtein \
    PyYAML tree-sitter-typescript platformdirs universal-pathlib
fi

# --- run -----------------------------------------------------------------------
export MC_TASK_ID="${MC_TASK_ID:-cal_python}"
export MC_TOKEN_LIMIT="${MC_TOKEN_LIMIT:-20000000}"
export MC_TIME_LIMIT="${MC_TIME_LIMIT:-7200}"
export MC_MESSAGE_LIMIT="${MC_MESSAGE_LIMIT:-250}"
export MC_OUT="${MC_OUT:-${PWD}/mirrorcode-phase0-result.json}"

echo "Launching MirrorCode x Khala smoke (task ${MC_TASK_ID})..."
PYTHONPATH="${MC_CLONE}${PYTHONPATH:+:${PYTHONPATH}}" \
  "${VENV_DIR}/bin/python" "${SCRIPT_DIR}/run_smoke.py" "$@"
