#!/usr/bin/env bash
# Parity check: crates/claude_agent_sdk against @anthropic-ai/claude-agent-sdk.
#
# Fetches the npm package into a temporary directory (never into the
# repository), reads the wire surface from its sdk.d.ts, and compares it
# with what the Rust crate models:
#
#   - every `type:` and `subtype:` wire discriminator
#   - the control request subtypes in SDKControlRequestInner
#   - the permission modes
#   - the Options fields, against the mapped and deferred lists below
#
# Exit 1 means the crate is behind; the output is the work list. Gaps that
# PARITY.md lists as deferred are reported but do not fail the check.
#
# Usage: scripts/check-claude-sdk-parity.sh [version]
#   scripts/check-claude-sdk-parity.sh           # the version the crate tracks
#   scripts/check-claude-sdk-parity.sh latest    # the newest published version

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CRATE_DIR="$ROOT/crates/claude_agent_sdk"
TRACKED="$(sed -n 's/^pub const UPSTREAM_SDK_VERSION: &str = "\(.*\)";/\1/p' "$CRATE_DIR/src/lib.rs")"
VERSION="${1:-$TRACKED}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "fetching @anthropic-ai/claude-agent-sdk@$VERSION (crate tracks $TRACKED)"
(cd "$WORK" && npm pack "@anthropic-ai/claude-agent-sdk@$VERSION" --silent >/dev/null 2>&1)
tar -xzf "$WORK"/anthropic-ai-claude-agent-sdk-*.tgz -C "$WORK"
DTS="$WORK/package/sdk.d.ts"
ACTUAL="$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' "$WORK/package/package.json" | head -1)"
echo "upstream version: $ACTUAL"
[ -f "$DTS" ] || { echo "error: sdk.d.ts not found in the package" >&2; exit 2; }

RUST_SRC="$CRATE_DIR/src/protocol/messages.rs $CRATE_DIR/src/protocol/control.rs"
# shellcheck disable=SC2086
rust_renames="$(grep -ohE 'rename = "[a-z_]+"' $RUST_SRC | sed 's/rename = "//;s/"//' | sort -u)"

fail=0
report() { # title, missing list, deferred list
  local title="$1" missing="$2" deferred="$3" blocking
  blocking="$(comm -23 <(echo "$missing" | grep -v '^$' | sort -u) <(echo "$deferred" | tr ' ' '\n' | sort -u) || true)"
  echo ""
  echo "== $title =="
  if [ -z "$(echo "$missing" | grep -v '^$' || true)" ]; then
    echo "OK"
    return
  fi
  if [ -n "$blocking" ]; then
    fail=1
    echo "MISSING in Rust:"
    echo "$blocking" | sed 's/^/  - /'
  fi
  local listed
  listed="$(comm -12 <(echo "$missing" | grep -v '^$' | sort -u) <(echo "$deferred" | tr ' ' '\n' | sort -u) || true)"
  if [ -n "$listed" ]; then
    echo "deferred (PARITY.md):"
    echo "$listed" | sed 's/^/  - /'
  fi
}

# ---- wire discriminators ----------------------------------------------------
# Every quoted `type:` / `subtype:` literal in sdk.d.ts. Some are not frames
# (content blocks, settings sources); those are listed as not-wire below.
NOT_WIRE="custom regex local stdio sse http sdk text image document tool_use
tool_result thinking redacted_thinking enabled disabled adaptive json_schema
preset agent command prompt mcp_tool object string"
upstream_types="$(grep -oE "\b(type|subtype): '[a-z_]+'" "$DTS" | sed -E "s/.*'([a-z_]+)'/\1/" | sort -u)"
known="$({ echo "$rust_renames"; echo $NOT_WIRE | tr ' ' '\n'; printf '%s\n' control_request control_response control_cancel_request keep_alive; } | sort -u)"
missing="$(comm -23 <(echo "$upstream_types") <(echo "$known") || true)"
DEFERRED_WIRE="transcript_mirror"
report "wire type and subtype values" "$missing" "$DEFERRED_WIRE"

# ---- control request subtypes ----------------------------------------------
inner="$(sed -n 's/.*declare type SDKControlRequestInner = //p' "$DTS" | grep -oE 'SDK[A-Za-z]+Request' | sort -u)"
ctrl=""
for name in $inner; do
  sub="$(awk -v n="declare type $name = " 'index($0, n) {f=1} f && /subtype:/ {print; exit}' "$DTS" | sed -E "s/.*'([a-z_]+)'.*/\1/")"
  ctrl="$ctrl$sub"$'\n'
done
missing="$(comm -23 <(echo "$ctrl" | grep -v '^$' | sort -u) <(echo "$rust_renames") || true)"
report "control request subtypes" "$missing" ""

# ---- permission modes -------------------------------------------------------
upstream_modes="$(sed -n "s/.*declare type PermissionMode = //p" "$DTS" | head -1 | grep -oE "'[a-zA-Z]+'" | tr -d "'" | sort -u)"
rust_modes="$(awk '/pub enum PermissionMode/{f=1} f&&/^}/{exit} f' "$CRATE_DIR/src/protocol/control.rs" \
  | grep -oE '^\s+[A-Z][A-Za-z]+,' | tr -d ' ,' \
  | awk '{print tolower(substr($0,1,1)) substr($0,2)}' | sort -u)"
missing="$(comm -23 <(echo "$upstream_modes") <(echo "$rust_modes") || true)"
report "permission modes" "$missing" ""

# ---- Options fields ---------------------------------------------------------
# MAPPED: TS option name -> handled by QueryOptions (flag, initialize field,
# environment variable, or host callback). DEFERRED: listed in PARITY.md.
MAPPED="additionalDirectories agent agentProgressSummaries agents
allowDangerouslySkipPermissions allowedTools betas canUseTool continue cwd
debug debugFile disallowedTools effort enableFileCheckpointing env executable
executableArgs extraArgs fallbackModel forkSession forwardSubagentText hooks
includeHookEvents includePartialMessages managedSettings maxBudgetUsd
maxThinkingTokens maxTurns mcpServers model onElicitation onUserDialog
outputFormat pathToClaudeCodeExecutable permissionMode permissionPrompts
permissionPromptToolName persistSession perTaskStopAffordance
planModeInstructions pluginDelivery plugins projectConfigRoot
promptSuggestions resume resumeDropsTurn resumeSessionAt sandbox sessionId
settings settingSources skills strictMcpConfig supportedDialogKinds
systemPrompt taskBudget thinking title toolAliases tools verbatimPrompts"
DEFERRED_OPTIONS="abortController loadTimeoutMs sessionStore sessionStoreFlush
spawnClaudeCodeProcess stderr toolConfig"
upstream_opts="$(awk '/^export declare type Options = \{/{f=1;next} f&&/^};/{exit} f' "$DTS" \
  | grep -oE '^    [a-zA-Z]+\??:' | tr -d ' ?:' | sort -u)"
missing="$(comm -23 <(echo "$upstream_opts") <(echo "$MAPPED" | tr ' \n' '\n\n' | grep -v '^$' | sort -u) || true)"
report "Options fields" "$missing" "$DEFERRED_OPTIONS"

echo ""
echo "== summary =="
echo "upstream: $ACTUAL  crate tracks: $TRACKED"
if [ "$fail" -eq 0 ]; then
  echo "parity: OK (deferred gaps are listed in crates/claude_agent_sdk/PARITY.md)"
else
  echo "parity: BEHIND — update crates/claude_agent_sdk and PARITY.md"
fi
exit "$fail"
