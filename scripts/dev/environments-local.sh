#!/usr/bin/env bash
# Run the Environments pages locally against real Boat machines.
#
#   scripts/dev/environments-local.sh
#
# Opens http://127.0.0.1:4300/environments. Pick a GitHub repository; a
# Codex-driven setup agent sets it up on a Boat machine, a clean build
# saves an image, a fresh Boat machine checks it, and you save it. A saved
# environment can run Claude Code with your Anthropic API key.
#
# Reads, without printing any value:
#   $OA_SECRETS/boat.env       BOAT_API_KEY (and BOAT_API_BASE when set)
#   $CODEX_HOME/auth.json      the Codex login the setup agent uses
#                              (default ~/.codex; run `codex login` first)
#   GH_TOKEN                   optional; from the environment or `gh auth token`.
#                              Lists your repositories and fetches private ones.
#   ANTHROPIC_API_KEY          optional; turns on "Run Claude Code here".
#
# Settings (environment variables):
#   OA_SECRETS                 default ~/work/.secrets
#   OA_ENVIRONMENTS_STATE      default ~/.openagents/environments
#   OA_ENVIRONMENTS_TEMPLATE   Boat template for setup machines; default: the
#                              newest ready oa-coder-runtime-* template
#   OA_WEB_PORT                default 4300
#   CARGO_TARGET_DIR           default ~/work/openagents-target
#
# Real machines cost money. Every setup, build, and check machine is
# deleted when its step ends; see docs/cloud/environments-local.md.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
secrets=${OA_SECRETS:-$HOME/work/.secrets}
state=${OA_ENVIRONMENTS_STATE:-$HOME/.openagents/environments}
port=${OA_WEB_PORT:-4300}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/work/openagents-target}

if [ ! -f "$secrets/boat.env" ]; then
  echo "No Boat key: $secrets/boat.env is missing." >&2
  exit 1
fi
set -a
# shellcheck disable=SC1091
. "$secrets/boat.env"
set +a
if [ -z "${BOAT_API_KEY:-}" ]; then
  echo "$secrets/boat.env does not set BOAT_API_KEY." >&2
  exit 1
fi

codex_home=${CODEX_HOME:-$HOME/.codex}
if [ ! -f "$codex_home/auth.json" ]; then
  echo "No Codex login at $codex_home/auth.json. Run: codex login" >&2
  exit 1
fi

if [ -z "${GH_TOKEN:-}" ] && command -v gh >/dev/null 2>&1; then
  GH_TOKEN=$(gh auth token 2>/dev/null || true)
fi
if [ -n "${GH_TOKEN:-}" ]; then
  export GH_TOKEN
  credentials='["GH_TOKEN"]'
  github='"GH_TOKEN"'
else
  unset GH_TOKEN
  credentials='[]'
  github='null'
  echo "No GitHub token: paste public repositories only."
fi

if [ -n "${ANTHROPIC_API_KEY:-}" ]; then
  claude='"ANTHROPIC_API_KEY"'
else
  claude='null'
  echo "No ANTHROPIC_API_KEY: saved environments won't offer Claude Code runs."
fi

if [ -n "${OA_ENVIRONMENTS_TEMPLATE:-}" ]; then
  template="\"$OA_ENVIRONMENTS_TEMPLATE\""
else
  template='null'
fi

mkdir -p "$state"
chmod 700 "$state"
config="$state/environments.json"
# The config names credentials; it holds no secret.
cat > "$config" <<JSON
{
  "schema": "openagents.environment.studio.v1",
  "state": "$state",
  "machines": {
    "schema": "openagents.environment.owners.v1",
    "provider": "boat",
    "workdir": "/home/user/repo",
    "template": $template,
    "credential_names": $credentials,
    "tick_seconds": 15
  },
  "owner": { "workspace": "local", "principal": "owner" },
  "codex_home": "$codex_home",
  "size": "small",
  "deadline_seconds": 7200,
  "github_token": $github,
  "claude_key": $claude
}
JSON
chmod 600 "$config"

cd "$root"
echo "Environments: http://127.0.0.1:$port/environments"
exec cargo run --release -p openagents-web --bin openagents-web -- \
  --listen "127.0.0.1:$port" \
  --chat-store "$state/web-chats" \
  --environments "$config"
