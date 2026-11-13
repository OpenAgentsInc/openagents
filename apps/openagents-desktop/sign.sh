#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper around security, PlistBuddy and codesign; holds no logic)
# How build.sh signs OpenAgents.app (#13388), in two steps it sources, so tests/sign-check.sh can run
# the same steps on a built app.
#
# A development build is signed ad hoc: no Keychain, no prompt, and no push token (Apple gives one
# only to a build signed with the push entitlement and a provisioning profile).
# OA_MAC_SIGN_IDENTITY (a signing identity's name) with OA_MAC_PROFILE (a .provisionprofile for
# com.openagents.desktop that has Push Notifications) signs the build for push: the profile goes
# into the bundle and the build takes the profile's own entitlements. A development profile's token
# is only good on Apple's sandbox, so the app then names that service (OA_PUSH_SANDBOX,
# AppAuth.pushService). codesign reads the identity's key from the Keychain, so only a person runs
# it this way (docs/desktop/coax-desktop.md, "A build signed for push"). The identity "-" signs ad
# hoc with the profile's entitlements: every step but the identity, for the check; macOS won't
# launch that build.
#
#   sign_prepare WORK    before compiling: reads the profile into WORK/OpenAgents.entitlements and sets
#                        push_env ("", development or production) and swift_defs; stops on a profile
#                        with no push entitlement
#   sign_bundle APP WORK after the bundle is whole: signs it
sign_prepare() {
  push_env="" swift_defs=()
  [[ -n "${OA_MAC_SIGN_IDENTITY:-}" && -n "${OA_MAC_PROFILE:-}" ]] || return 0
  [[ -f "$OA_MAC_PROFILE" ]] || { echo "build.sh: no profile at $OA_MAC_PROFILE" >&2; return 1; }
  security cms -D -i "$OA_MAC_PROFILE" > "$1/profile.plist" 2> /dev/null || { echo "build.sh: $OA_MAC_PROFILE is not a provisioning profile" >&2; return 1; }
  /usr/libexec/PlistBuddy -x -c 'Print :Entitlements' "$1/profile.plist" > "$1/OpenAgents.entitlements" 2> /dev/null || { echo "build.sh: $OA_MAC_PROFILE has no entitlements" >&2; return 1; }
  push_env="$(/usr/libexec/PlistBuddy -c 'Print :com.apple.developer.aps-environment' "$1/OpenAgents.entitlements" 2> /dev/null || true)"
  [[ "$push_env" == development || "$push_env" == production ]] || { echo "build.sh: $OA_MAC_PROFILE has no push entitlement: turn on Push Notifications for com.openagents.desktop and make the profile again" >&2; return 1; }
  [[ "$push_env" != development ]] || swift_defs=(-D OA_PUSH_SANDBOX)
}
sign_bundle() {
  if [[ -n "$push_env" ]]; then
    cp "$OA_MAC_PROFILE" "$1/Contents/embedded.provisionprofile"
    codesign --force --options runtime --entitlements "$2/OpenAgents.entitlements" --sign "$OA_MAC_SIGN_IDENTITY" "$1" >/dev/null
    echo "signed for push ($push_env) with $OA_MAC_SIGN_IDENTITY"
  else
    rm -f "$1/Contents/embedded.provisionprofile"
    codesign --force --sign - "$1" >/dev/null
  fi
}
