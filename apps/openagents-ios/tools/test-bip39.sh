#!/usr/bin/env bash
# Published BIP-39 test vectors against the app's entropy-to-words code.
set -euo pipefail
cd "$(dirname "$0")/.."
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
xcrun --sdk macosx swiftc -O OpenAgents/Wallet/BIP39.swift tools/bip39/main.swift -o "$out/bip39"
"$out/bip39" OpenAgents/Resources/bip39-english.txt
