#!/usr/bin/env bash

# Shared cutover-aware fence for supervisor paths that formerly created
# internal GitHub issues. This file only defines functions when sourced.

: "${SUP_NODE_BIN:=node}"

sup_internal_github_write_policy_cli() {
  local script_dir repo_root
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "$script_dir/../../../.." && pwd)"
  printf '%s/scripts/internal-github-write-policy.ts' "$repo_root"
}

sup_assert_internal_github_write_allowed() {
  local operation="$1"
  "$SUP_NODE_BIN" --experimental-strip-types \
    "$(sup_internal_github_write_policy_cli)" "$operation" >/dev/null
}
