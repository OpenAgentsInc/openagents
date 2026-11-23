#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: generates Coax C bridge, regenerates Xcode project, and builds OpenAgents macOS; holds no logic).
set -euo pipefail
cd "$(dirname "$0")"
./generate.sh
xcodegen generate
exec ../../coax/remote/remote.sh run --local -- xcodebuild -project OpenAgentsMac.xcodeproj -scheme OpenAgentsMac -configuration Debug -sdk macosx -derivedDataPath build CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- "$@" build
