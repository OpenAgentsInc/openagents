#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles DeviceKey.swift and its Swift check with swiftc and runs it)
# Checks the Mac's device key (DeviceKey.swift, #13216) the way a rebuilt debug app meets it:
# builds the check twice, as two different binaries (a rebuilt app has a new code signature), and
# has the second open the key the first made. Nothing touches the Keychain, so neither run can
# raise a password prompt; a run that waited on one would hit the 60-second limit and fail.
# Then the same with a software key (a Mac with no Secure Enclave), and forgetting a key.
#   apps/shared/host/apple/device-key-check.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
build() { xcrun swiftc -O -D "$2" -o "$work/$1" "$here/DeviceKey.swift" "$here/device-key-check.swift" 2> "$work/$1.log" || { cat "$work/$1.log" >&2; exit 1; }; }
build first BUILD_ONE
printf '// rebuilt\n' > "$work/salt.swift"
xcrun swiftc -O -D BUILD_TWO -o "$work/second" "$here/DeviceKey.swift" "$here/device-key-check.swift" "$work/salt.swift" 2> "$work/second.log" || { cat "$work/second.log" >&2; exit 1; }
[[ "$(shasum -a 256 < "$work/first")" != "$(shasum -a 256 < "$work/second")" ]] || { echo "device-key-check: the two builds are the same binary" >&2; exit 1; }
# The module must not link a Keychain item call at all.
if nm -u "$work/first" | grep -E '_SecItem(Add|CopyMatching|Update|Delete)$'; then echo "device-key-check: the check links the Keychain" >&2; exit 1; fi
run() { perl -e 'alarm 60; exec @ARGV' "$@"; }
mkdir -p "$work/hw" "$work/sw"
run "$work/first" check "$work/hw"
run "$work/second" check "$work/hw" | grep -q 'reopened' || { echo "device-key-check: the rebuilt binary didn't reopen the key" >&2; exit 1; }
echo "device key: a rebuilt binary reopened it with no prompt"
OA_DEVICE_KEY_SOFTWARE=1 run "$work/first" check "$work/sw"
OA_DEVICE_KEY_SOFTWARE=1 run "$work/second" check "$work/sw" | grep -q 'software, reopened' || { echo "device-key-check: the software key didn't reopen" >&2; exit 1; }
OA_DEVICE_KEY_FORGET=1 run "$work/second" check "$work/hw"
echo "device-key-check: ok"
