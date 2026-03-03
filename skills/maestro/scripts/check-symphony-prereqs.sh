#!/usr/bin/env bash
set -euo pipefail

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'Missing required command: %s\n' "$1" >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq

: "${SYMPHONY_BASE_URL:?Set SYMPHONY_BASE_URL}"

printf 'Checking Symphony base URL: %s\n' "$SYMPHONY_BASE_URL"

curl -fsS "${SYMPHONY_BASE_URL}/" >/dev/null
TIP_JSON="$(curl -fsS "${SYMPHONY_BASE_URL}/tip")"
TIP_HEIGHT="$(printf '%s' "$TIP_JSON" | jq -r '.height // .block_height // .data.block_height // empty')"

if [[ -z "$TIP_HEIGHT" ]]; then
  printf 'Unable to parse tip height from /tip response\n' >&2
  exit 1
fi

printf 'Symphony tip height parsed: %s\n' "$TIP_HEIGHT"
printf 'Symphony prereq check passed.\n'
