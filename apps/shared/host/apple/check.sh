#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper around the apps' own build scripts, xcodegen and xcodebuild; holds no logic)
# The Apple apps still build and their unit tests pass, on this Mac (GitHub #13389): development
# builds only, nothing installed, nothing signed for release, no Keychain. Run it after a change to
# a client's Coax, a realm's face, the page host (metaverse-engine/host/apple/) or a struct the Swift reads.
# The part any build host can do (every realm and pass the apps name resolves, emits and compiles)
# runs first, and alone in the pipeline's reports check: verse/native/app-realms.coax.
#   apps/shared/host/apple/check.sh SCRATCH     SCRATCH holds Xcode's DerivedData and the logs; delete it after
# Each line it prints is one step that passed. A step that fails prints its log's last lines.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../../.." && pwd)"
scratch="${1:?usage: check.sh SCRATCH}"
mkdir -p "$scratch"; scratch="$(cd "$scratch" && pwd)"
export BROWSER=/usr/bin/true
step() {   # NAME CMD...
  local name="$1"; shift
  "$@" > "$scratch/$name.log" 2>&1 || { tail -40 "$scratch/$name.log" >&2; echo "check.sh: $name failed ($scratch/$name.log)" >&2; exit 1; }
  echo "ok $name"
}
step app-realms "$repo/coax/web/tool.sh" "$repo/verse/native/app-realms.coax" "$repo" "$scratch/app-realms"
step everglade-mac "$repo/apps/everglade-desktop/build.sh"
step everglade-iphone-simulator "$repo/apps/everglade-mobile/build.sh"
step openagents-mac "$repo/apps/openagents-desktop/build.sh"
step openagents-mac-account "$repo/apps/openagents-desktop/tests/account-check.sh"
step device-key "$here/device-key-check.sh"
cd "$repo/apps/openagents-ios"
step openagents-iphone-generate ./generate.sh
step openagents-iphone-project xcodegen generate
xcode=(xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -configuration Debug -sdk iphonesimulator -destination "${OA_IOS_DESTINATION:-generic/platform=iOS Simulator}" -derivedDataPath "$scratch/DerivedData" CODE_SIGNING_ALLOWED=NO)
step openagents-iphone-build "${xcode[@]}" build-for-testing
# The unit tests need a simulator to run in: OA_IOS_DESTINATION='platform=iOS Simulator,name=NAME'.
if [[ -n "${OA_IOS_DESTINATION:-}" ]]; then step openagents-iphone-unit-tests "${xcode[@]}" -only-testing:OpenAgentsTests test-without-building; fi
echo "apple apps: ok"
