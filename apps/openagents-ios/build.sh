#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
./generate.sh
xcodegen generate
exec xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -configuration Debug -sdk iphonesimulator -derivedDataPath build CODE_SIGNING_ALLOWED=NO "$@" build
