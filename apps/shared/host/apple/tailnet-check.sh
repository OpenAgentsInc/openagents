#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles the Coax tailnet codec, AppTailnet.swift, AppTailnetLink.swift and their Swift check and runs it)
# Checks zero-config pairing and direct transport over Tailscale (GitHub #13778, #13795) on this Mac.
#   apps/shared/host/apple/tailnet-check.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../../.." && pwd)"
native="$here/../../native"
coax="${COAX:-$repo/coax/bin/coax}"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
"$coax" c "$repo/coax/lib/nostr/host/tailnet.coax" --library --header "$work/tailnet-gen.h" > "$work/tailnet-gen.c"
printf '#include "tailnet-gen.h"\n#include "%s/tailnet-codec.c"\n' "$native" > "$work/tailnet-bridge.c"
/usr/bin/clang -std=gnu11 -O2 -w -I"$work" -I"$native" -c "$work/tailnet-gen.c" -o "$work/tailnet-gen.o"
/usr/bin/clang -std=gnu11 -O2 -w -I"$work" -I"$native" -c "$work/tailnet-bridge.c" -o "$work/tailnet-bridge.o"
cp "$here/tailnet-check.swift" "$work/main.swift"
xcrun swiftc -O -swift-version 5 -import-objc-header "$native/tailnet-codec.h" -o "$work/tailnet-check" "$native/AppTailnet.swift" "$native/AppTailnetLink.swift" "$work/main.swift" "$work/tailnet-gen.o" "$work/tailnet-bridge.o" 2> "$work/build.log" || { cat "$work/build.log" >&2; exit 1; }
if nm -u "$work/tailnet-check" | grep -E '_SecItem(Add|CopyMatching|Update|Delete)$'; then echo "tailnet-check: the check links the Keychain" >&2; exit 1; fi
perl -e 'alarm 90; exec @ARGV' "$work/tailnet-check" "$work"
