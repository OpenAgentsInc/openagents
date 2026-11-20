#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, swiftc and the compiled check; holds no logic)
# Checks each chat's own worktree (GitHub #13758) under a temporary HOME so nothing in ~/.openagents is touched.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/home" "$tmp/work" "$tmp/c"
"$coax" c "$here/desktop-core.coax" --library --header "$tmp/c/desktop-core.h" > "$tmp/c/desktop-core.generated.c"
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$tmp/c/entropy-policy.h" > "$tmp/c/entropy-policy.c"
cp "$here/desktop-core-unit.c" "$tmp/c/desktop-core-unit.c"
cflags=(-mmacosx-version-min=14.0 -O2 -w -I"$tmp/c" -I"$repo/coax/platform")
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$tmp/c/desktop-core-unit.c" -o "$tmp/c/desktop-core-all.o"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$tmp/c/entropy-policy.c" -o "$tmp/c/entropy-policy.o"
for _dc_f in process-native.c store-file-native.c read-file-native.c socket-native.c entropy-native.c entropy-foreign.c; do
  /usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$repo/coax/platform/$_dc_f" -o "$tmp/c/$_dc_f.o"
done
ld -r "$tmp/c/desktop-core-all.o" "$tmp/c/entropy-policy.o" "$tmp/c/process-native.c.o" "$tmp/c/store-file-native.c.o" "$tmp/c/read-file-native.c.o" "$tmp/c/socket-native.c.o" "$tmp/c/entropy-native.c.o" "$tmp/c/entropy-foreign.c.o" -exported_symbol '_oa_dc_*' -o "$tmp/c/desktop-core.o"
xcrun -sdk macosx swiftc -swift-version 5 -suppress-warnings -parse-as-library -target arm64-apple-macos14.0 -import-objc-header "$here/desktop-core-mac.h" \
  "$here/DesktopCore.swift" "$here/Worktree.swift" "$here/Store.swift" "$here/Foreman.swift" "$here/ForemanOperator.swift" "$here/worktree-test.swift" \
  "$tmp/c/desktop-core.o" -o "$tmp/test"
HOME="$tmp/home" CFFIXED_USER_HOME="$tmp/home" WT_TEST_DIR="$tmp" CODER_WORKTREES= CARGO_TARGET_DIR= COAX_TOOL_CACHE= "$tmp/test"
