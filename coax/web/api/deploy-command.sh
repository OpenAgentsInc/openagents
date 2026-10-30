#!/usr/bin/env bash
# Preserve the deploy command's exit status before filtering successful logs.
# This does not establish revision readiness, image identity or health freshness.
api_run_deploy() {
  local output status=0
  output="$("$@" 2>&1)" || status=$?
  if (( status != 0 )); then
    printf '%s\n' "$output" >&2
    return "$status"
  fi
  printf '%s\n' "$output" | grep -E 'ERROR|revision' || true
}
