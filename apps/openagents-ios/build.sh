#!/usr/bin/env bash
# Firm reason: 5 (thin Coax generation and Xcode SDK wrapper).
set -euo pipefail
cd "$(dirname "$0")"
./generate.sh
xcodegen generate
exec ../../coax/remote/remote.sh run --local -- xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -configuration Debug -sdk iphonesimulator -derivedDataPath build CODE_SIGNING_ALLOWED=NO "$@" build
