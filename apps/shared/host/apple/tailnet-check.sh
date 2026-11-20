#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles AppTailnet.swift, AppTailnetLink.swift and their Swift check with swiftc and runs it)
# Checks zero-config pairing over Tailscale (GitHub #13778) on this Mac: the TXT record, the
# `tailscale whois` admission decision, the two lines, the phone's saved Macs, and the phone's
# link against a stand-in Mac on the loopback (no discovery, fallback to the name, asking again on
# a network change). No Keychain, no Tailscale, no browsing, nothing listening past the loopback.
#   apps/shared/host/apple/tailnet-check.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
native="$here/../../native"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
cp "$here/tailnet-check.swift" "$work/main.swift"
xcrun swiftc -O -swift-version 5 -o "$work/tailnet-check" "$native/AppTailnet.swift" "$native/AppTailnetLink.swift" "$work/main.swift" 2> "$work/build.log" || { cat "$work/build.log" >&2; exit 1; }
if nm -u "$work/tailnet-check" | grep -E '_SecItem(Add|CopyMatching|Update|Delete)$'; then echo "tailnet-check: the check links the Keychain" >&2; exit 1; fi
perl -e 'alarm 90; exec @ARGV' "$work/tailnet-check" "$work"
