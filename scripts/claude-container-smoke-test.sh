#!/usr/bin/env bash
set -euo pipefail

image="${OPENAGENTS_CLAUDE_CONTAINER_IMAGE:-openagents/claude-code:latest}"
runtime="${OPENAGENTS_CLAUDE_CONTAINER_RUNTIME:-apple}"
max_budget="${CLAUDE_MAX_BUDGET_USD:-0.10}"

if [ "$#" -gt 0 ]; then
  prompt="$*"
else
  prompt="${CLAUDE_PROMPT:-Reply with OK}"
fi

tmp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t claude-container)"
cleanup() { rm -rf "$tmp_dir"; }
trap cleanup EXIT

creds_file="$tmp_dir/.credentials.json"

if [ -n "${CLAUDE_CREDENTIALS_JSON:-}" ]; then
  if [ ! -f "$CLAUDE_CREDENTIALS_JSON" ]; then
    echo "Missing credentials file: $CLAUDE_CREDENTIALS_JSON" >&2
    exit 1
  fi
  cp "$CLAUDE_CREDENTIALS_JSON" "$creds_file"
elif [ -f "$HOME/.claude/.credentials.json" ]; then
  cp "$HOME/.claude/.credentials.json" "$creds_file"
else
  if command -v security >/dev/null 2>&1; then
    account="${CLAUDE_KEYCHAIN_ACCOUNT:-$USER}"
    if ! security find-generic-password -s "Claude Code-credentials" -a "$account" -w > "$creds_file"; then
      echo "Failed to read Claude Code credentials from keychain for account: $account" >&2
      exit 1
    fi
  else
    echo "No credentials found. Set CLAUDE_CREDENTIALS_JSON or login on host." >&2
    exit 1
  fi
fi

chmod 600 "$creds_file"

if [ "$runtime" = "auto" ]; then
  if command -v container >/dev/null 2>&1; then
    runtime="apple"
  elif command -v docker >/dev/null 2>&1; then
    runtime="docker"
  else
    echo "No container runtime found (need container or docker)." >&2
    exit 1
  fi
fi

case "$runtime" in
  apple)
    command -v container >/dev/null 2>&1 || {
      echo "container CLI not found." >&2
      exit 1
    }
    container run --rm -i \
      -v "$tmp_dir":/tmp/claude \
      -e CLAUDE_CONFIG_DIR=/tmp/claude \
      "$image" \
      /home/agent/.claude/bin/claude --print "$prompt" --max-budget-usd "$max_budget"
    ;;
  docker)
    command -v docker >/dev/null 2>&1 || {
      echo "docker CLI not found." >&2
      exit 1
    }
    docker run --rm -i \
      -v "$creds_file":/home/agent/.claude/.credentials.json:ro \
      "$image" \
      claude --print "$prompt" --max-budget-usd "$max_budget"
    ;;
  *)
    echo "Unknown OPENAGENTS_CLAUDE_CONTAINER_RUNTIME: $runtime" >&2
    exit 1
    ;;
  esac
