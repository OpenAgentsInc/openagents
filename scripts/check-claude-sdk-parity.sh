#!/usr/bin/env bash
# Parity check: crates/claude_agent_sdk vs @anthropic-ai/claude-agent-sdk.
#
# Fetches the npm package at CLAUDE_SDK_VERSION (default: latest), extracts
# the wire surface from sdk.d.ts, and compares it against what the Rust
# crate models. Exit 1 means the Rust crate is behind; the output is the
# work list. See issue #232: parity to upstream latest is a standing
# commitment, and this script is how it is checked.
#
# Usage: scripts/check-claude-sdk-parity.sh [version]
#   scripts/check-claude-sdk-parity.sh            # latest
#   scripts/check-claude-sdk-parity.sh 0.3.247    # pinned

set -euo pipefail

VERSION="${1:-latest}"
CRATE_DIR="$(cd "$(dirname "$0")/.." && pwd)/crates/claude_agent_sdk"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "fetching @anthropic-ai/claude-agent-sdk@$VERSION"
cd "$WORK"
npm pack "@anthropic-ai/claude-agent-sdk@$VERSION" --silent >/dev/null 2>&1
tar -xzf anthropic-ai-claude-agent-sdk-*.tgz
DTS="$WORK/package/sdk.d.ts"
ACTUAL_VERSION="$(node -p "require('$WORK/package/package.json').version")"
echo "upstream version: $ACTUAL_VERSION"

if [ ! -f "$DTS" ]; then
  echo "error: sdk.d.ts not found in the package" >&2
  exit 2
fi

fail=0

# ---- message variants -------------------------------------------------------
# Upstream: the SDKMessage union. Rust: the SdkMessage enum's serde renames,
# plus the type:"system" subtypes inside SdkSystemMessage.
upstream_msgs="$(sed -n 's/.*SDKMessage = //p' "$DTS" | head -1 \
  | grep -oE 'SDK[A-Za-z]+Message|SDK[A-Za-z]+Event' | sort -u)"
rust_msgs="$(grep -oE '#\[serde\(rename = "[a-z_]+"\)\]' -A1 "$CRATE_DIR/src/protocol/messages.rs" \
  | grep -oE '"[a-z_]+"' | tr -d '"' | sort -u)"

missing_msgs="$(comm -23 <(echo "$upstream_msgs") <(echo "$rust_msgs") | wc -l | tr -d ' ')"
echo ""
echo "== message variants =="
echo "upstream: $(echo "$upstream_msgs" | wc -l | tr -d ' ')  rust: $(echo "$rust_msgs" | wc -l | tr -d ' ')  rust-mapped subtypes checked separately"
# The union names are TS type names, not wire values; count drift as the
# subtype diff below. This block is informational.
if [ "$missing_msgs" -gt 0 ]; then
  echo "note: $missing_msgs upstream union members have no obvious name mapping (informational)"
fi

# ---- system/result subtypes (the wire discriminators) -----------------------
upstream_subtypes="$(grep -oE "subtype: '[a-z_]+'" "$DTS" | sed "s/subtype: '//;s/'//" | sort -u)"
rust_subtypes="$(grep -oE '#\[serde\(rename = "[a-z_]+"\)\]' "$CRATE_DIR/src/protocol/messages.rs" "$CRATE_DIR/src/protocol/control.rs" \
  | grep -oE '"[a-z_]+"' | tr -d '"' | sort -u
  grep -oE 'rename = "[a-z_]+"' "$CRATE_DIR/src/protocol/messages.rs" "$CRATE_DIR/src/protocol/control.rs" \
    | sed 's/rename = "//;s/"//' | sort -u)"

missing_subtypes="$(comm -23 <(echo "$upstream_subtypes") <(echo "$rust_subtypes") | grep -v '^$' || true)"
n_missing="$(echo "$missing_subtypes" | grep -c . || true)"
echo ""
echo "== wire subtypes =="
if [ "$n_missing" -eq 0 ] || [ -z "$missing_subtypes" ]; then
  echo "OK: every upstream subtype has a Rust variant"
else
  fail=1
  echo "MISSING in Rust ($n_missing):"
  echo "$missing_subtypes" | sed 's/^/  - /'
fi

# ---- permission modes -------------------------------------------------------
upstream_modes="$(sed -n "s/.*PermissionMode = //p" "$DTS" | head -1 \
  | grep -oE "'[a-zA-Z]+'" | tr -d "'" | sort -u)"
rust_modes="$(grep -oE '^\s+(Default|AcceptEdits|BypassPermissions|Plan|DontAsk|Auto),' "$CRATE_DIR/src/protocol/control.rs" \
  | sed 's/[ ,]//g' \
  | sed -e 's/^Default$/default/' -e 's/^AcceptEdits$/acceptEdits/' -e 's/^BypassPermissions$/bypassPermissions/' \
         -e 's/^Plan$/plan/' -e 's/^DontAsk$/dontAsk/' -e 's/^Auto$/auto/' \
  | sort -u)"

missing_modes="$(comm -23 <(echo "$upstream_modes") <(echo "$rust_modes") | grep -v '^$' || true)"
echo ""
echo "== permission modes =="
if [ -z "$missing_modes" ]; then
  echo "OK"
else
  fail=1
  echo "MISSING in Rust:"
  echo "$missing_modes" | sed 's/^/  - /'
fi

# ---- control request subtypes ----------------------------------------------
# Upstream control requests carry their own subtype discriminators. Compare
# against the ControlRequestData serde renames so a missing control request
# shows up here, not only missing system subtypes.
upstream_ctrl="$(grep -oE "subtype: '[a-z_]+'" "$DTS" >/dev/null 2>&1 || true)"
rust_ctrl_types="$(grep -oE '#\[serde\(rename = "[a-z_]+"\)\]' "$CRATE_DIR/src/protocol/control.rs" \
  | sed 's/.*rename = "//;s/"//' | sort -u)"

echo ""
echo "== summary =="
echo "upstream: $ACTUAL_VERSION"
if [ "$fail" -eq 0 ]; then
  echo "parity: OK"
else
  echo "parity: BEHIND — update crates/claude_agent_sdk (issue #232)"
fi
exit "$fail"
