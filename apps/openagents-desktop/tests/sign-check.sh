#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: runs sign.sh's two steps on a built app and reads the result with codesign)
# Checks the Mac build's signing (sign.sh, #13388) as far as it goes without a signing identity:
# no Keychain, no prompt. It signs a copy of the built app, never the app itself.
#   1. the default path: ad hoc, no profile in the bundle, no entitlements, and the signature holds
#   2. the push path with a throwaway development profile and the identity "-": the profile is read,
#      it lands in the bundle, the signature carries the profile's entitlements and the hardened
#      runtime, and the Swift is compiled to name Apple's sandbox push service
#   3. a production profile compiles without the sandbox flag
#   4. a profile with no push entitlement, a file that is no profile and a missing file stop the build
# Not checked here: a real identity's signature, and a token from Apple (NEEDS_OWNER.md).
#   apps/openagents-desktop/tests/sign-check.sh     after apps/openagents-desktop/build.sh
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
repo="$(cd "$here/../.." && pwd)"
built="$here/build/OpenAgents.app"
[[ -x "$built/Contents/MacOS/OpenAgents" ]] || { echo "sign-check: build the app first (apps/openagents-desktop/build.sh)" >&2; exit 1; }
work="$(mktemp -d "${TMPDIR:-/tmp}/sign-check.XXXXXX")"; trap 'rm -rf "$work"' EXIT
fail() { echo "sign-check: $1" >&2; exit 1; }
profile() {   # NAME APS: a throwaway profile, CMS-signed by a key made here; APS "" leaves push out
  local aps=""
  [[ -z "$2" ]] || aps="<key>com.apple.developer.aps-environment</key><string>$2</string>"
  printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>' '<plist version="1.0"><dict><key>Name</key><string>sign-check</string><key>Entitlements</key><dict>' \
    "$aps" '<key>com.apple.application-identifier</key><string>CHECK00000.com.openagents.desktop</string>' \
    '<key>com.apple.developer.team-identifier</key><string>CHECK00000</string></dict></dict></plist>' > "$work/$1.plist"
  openssl cms -sign -binary -in "$work/$1.plist" -signer "$work/cert.pem" -inkey "$work/key.pem" -outform DER -nodetach -out "$work/$1.provisionprofile" 2> /dev/null
}
openssl req -x509 -newkey rsa:2048 -nodes -keyout "$work/key.pem" -out "$work/cert.pem" -subj /CN=oa-sign-check -days 1 2> /dev/null
profile dev development; profile prod production; profile none ""
printf 'not a profile\n' > "$work/junk.provisionprofile"
source "$here/sign.sh"
app="$work/OpenAgents.app"
cp -R "$built" "$app"

# 1. the default path
unset OA_MAC_SIGN_IDENTITY OA_MAC_PROFILE
mkdir -p "$work/a"; sign_prepare "$work/a"
[[ -z "$push_env" && ${#swift_defs[@]} -eq 0 ]] || fail "the default build asked for push"
touch "$app/Contents/embedded.provisionprofile"   # one left by an earlier signed build
sign_bundle "$app" "$work/a"
[[ ! -e "$app/Contents/embedded.provisionprofile" ]] || fail "the default build kept a profile in the bundle"
codesign --verify --strict "$app" || fail "the default build's signature does not hold"
said="$(codesign -dv "$app" 2>&1)"; grep -q 'Signature=adhoc' <<< "$said" || fail "the default build is not signed ad hoc"
[[ -z "$(codesign -d --entitlements - --xml "$app" 2> /dev/null)" ]] || fail "the default build carries entitlements"

# 2. the push path, with every step but a real identity
export OA_MAC_SIGN_IDENTITY=- OA_MAC_PROFILE="$work/dev.provisionprofile"
mkdir -p "$work/b"; sign_prepare "$work/b"
[[ "$push_env" == development && "${swift_defs[*]}" == "-D OA_PUSH_SANDBOX" ]] || fail "a development profile did not ask for the sandbox service"
said="$(sign_bundle "$app" "$work/b")"
[[ "$said" == "signed for push (development) with -" ]] || fail "the push build said: $said"
cmp -s "$app/Contents/embedded.provisionprofile" "$OA_MAC_PROFILE" || fail "the profile is not in the bundle"
codesign --verify --strict "$app" || fail "the push build's signature does not hold"
seen="$(codesign -dv "$app" 2>&1)"; grep -Eq 'flags=0x[0-9a-f]*\(.*runtime' <<< "$seen" || fail "the push build has no hardened runtime"
codesign -d --entitlements - --xml "$app" 2> /dev/null > "$work/got.plist"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.developer.aps-environment' "$work/got.plist" 2> /dev/null)" == development ]] || fail "the signature has no push entitlement"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.application-identifier' "$work/got.plist" 2> /dev/null)" == CHECK00000.com.openagents.desktop ]] || fail "the signature's entitlements are not the profile's"
# The flag reaches the Swift: with it the login names Apple's sandbox service, without it the live one.
printf '@main enum SignCheck { static func main() { print(AppAuth.pushService) } }\n' > "$work/service.swift"
service() { xcrun -sdk macosx swiftc -O "$@" -suppress-warnings -parse-as-library \
  "$repo/apps/shared/host/apple/AppAuth.swift" "$repo/apps/shared/host/apple/DeviceKey.swift" "$work/service.swift" -o "$work/service" 2> "$work/swift.log" || { cat "$work/swift.log" >&2; fail "AppAuth.swift does not compile"; }
  "$work/service"; }
[[ "$(service "${swift_defs[@]}")" == apns_sandbox ]] || fail "a development build does not name the sandbox push service"

# 3. a production profile
export OA_MAC_PROFILE="$work/prod.provisionprofile"
sign_prepare "$work/b"
[[ "$push_env" == production && ${#swift_defs[@]} -eq 0 ]] || fail "a production profile asked for the sandbox service"
[[ "$(service)" == apns ]] || fail "a production build does not name the live push service"

# 4. what stops the build
for bad in none junk missing; do
  export OA_MAC_PROFILE="$work/$bad.provisionprofile"
  if (sign_prepare "$work/b") 2> "$work/err"; then fail "a build went on with the profile '$bad'"; fi
  grep -q '^build.sh: ' "$work/err" || fail "the profile '$bad' stopped the build without saying why"
done
grep -q 'has no push entitlement' <(export OA_MAC_PROFILE="$work/none.provisionprofile"; sign_prepare "$work/b" 2>&1 || true) || fail "a profile without push is not named as that"
echo "sign-check: ok"
