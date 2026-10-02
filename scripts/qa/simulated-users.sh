#!/usr/bin/env bash
# The simulated-user QA run: pretend people use every OpenAgents surface,
# a model judges each transcript, and confirmed findings become GitHub
# issues labeled `qa`. docs/qa/simulated-users.md is the runbook.
#
#   scripts/qa/simulated-users.sh [--build] [--personas ID,...] [--surfaces S,...]
#                                 [--out DIR] [--max-jobs N] [--judge-model M]
#   scripts/qa/simulated-users.sh --list
#   scripts/qa/simulated-users.sh --out DIR --file FINDING_ID,...
#
# --build first builds openagents, openagents-desktop, coder, and microcoder
# into $CARGO_TARGET_DIR/debug, the binaries the run drives. The persona and
# judge models default to Space Bunny Alpha through OpenRouter; the key is
# read from OPENROUTER_API_KEY or ~/work/.secrets/openrouter.env and never
# printed. `--judge-model claude:sonnet` judges with the Claude CLI instead.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

build=0
args=()
for arg in "$@"; do
  case "$arg" in
    --build) build=1 ;;
    -h|--help) sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed '$d' | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) args+=("$arg") ;;
  esac
done

if [ -z "${OPENROUTER_API_KEY:-}" ]; then
  secrets="${OPENAGENTS_QA_SECRETS:-$HOME/work/.secrets/openrouter.env}"
  if [ -f "$secrets" ]; then
    OPENROUTER_API_KEY="$(sed -n 's/^OPENROUTER_API_KEY=//p' "$secrets" | head -1)"
    export OPENROUTER_API_KEY
  fi
fi

if [ "$build" = 1 ]; then
  (cd "$root" && cargo build -p openagents-cli -p openagents-desktop -p coder -p microcoder)
fi

command -v uv >/dev/null || { echo "simulated-users: uv is required (https://docs.astral.sh/uv/)" >&2; exit 2; }
exec uv run --quiet --no-project --python 3.12 --with pyte \
  python "$root/scripts/qa/simulated_users.py" ${args[@]+"${args[@]}"}
