#!/usr/bin/env bash
# Usage: run.sh <task> <attempt> <deadline-sec>   (issue #9776)
# The frozen s7a1 arm: coder-one-delegate-fable-low-kb-jev2, the pinned
# bb38f2751c12 artifact (sha256 dbae6671...), explore_steps=0, and the
# task's candidates from select_knowledge.py --for-jev with the series-3 note.
set -euo pipefail
task=$1; attempt=$2; deadline=$3
KNOWLEDGE=$HOME/fable-delegate-9776/candidates/$task.json
cd ~/fable-delegate-9746/openagents-s7/bench/terminal-bench
export OPENAGENTS_API_KEY="$(tr -d '\n' < ~/.openagents/bearer)"
export TYPESAFE_API_KEY="$(jq -r .api_key ~/.openagents/jev.json)"
export CLAUDE_CODE_OAUTH_TOKEN="$(tr -d '\n' < ~/.openagents/claude-setup-token)"
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN OPENAI_API_KEY CODER_ONE_DELEGATE_EFFORT CODER_ONE_BRIEFING_CAP
artifact_path=/home/christopherdavid/fable-delegate-9746/s6/target/x86_64-unknown-linux-musl/release/coder-one
artifact_sha256=dbae667157c74e04385376c6d035059f1e5b001cd2fbf918863baa591f49fdc9
exec nix shell nixpkgs#uv -c uv run tbench run --profile tb4 --task "$task" \
  --agent coder-one-delegate-fable-low-kb-jev2 --auth-mode subscription-oauth \
  --job-name "tb4--coder-one-delegate-fable-low-kb-jev2--$task--9776-$attempt" \
  --agent-kwarg artifact_path="$artifact_path" \
  --agent-kwarg artifact_sha256="$artifact_sha256" \
  --agent-kwarg briefing_knowledge="$KNOWLEDGE" \
  --agent-kwarg explore_steps=0 \
  --agent-kwarg delegate_timeout_sec="$deadline"
