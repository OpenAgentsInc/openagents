# Coax Native starter, 2026-11-13

The first OpenAgents starter is iOS UIKit + WebKit, running Coax-generated
Wasm with the shared DOM component backend. It is not the proposed native
control renderer. The initial native archive bundles **Start here.**; signed
OTA generation 1 changes the Coax module to **Updates are live.** with a new
description. Login/chat screens remain subsequent application work.

## Release identity

- Bundle `com.openagents.app`, version 1.0.0, build **1068**, team HQWSG26L43.
- The bundle identity, icons, entitlements and Keychain namespaces are retained.
- Native runtime: `ffad2d971046f0e8d2981b023cea6cdee8f688638e8c6eeecedb07ddac679f0c`.
- Release executable SHA-256:
  `174081b6706429d1ba052c00beeea566c1806dceb8323c9f44177f37f6e80859`.
- Native implementation `3cf386e1ba`, release metadata correction `12171c7480`.
- Apple accepted the corrected upload, delivery
  `24dfd3cf-0fe9-45c4-a139-fa26e12e8ce1`, with no errors. Processing and internal
  group assignment are separate from upload acceptance.
- The first upload was refused for an omitted iPad orientation. All four
  orientations were added; it did not produce a usable TestFlight build.
- Archive, IPA, runtime config and symbols are retained outside the repository
  under `~/.openagents/releases/openagents-ios-1068-r2/`. The release dSYM is
  also retained in the existing symbols bucket under UUID
  `af57369692643c318643c1d134f7156f`.

## Checks actually run

`coax/remote/remote.sh run -- bash coax/native/check.sh` passed: five installed
policy checks, all **70/70 policy mutants killed**, 77 codec/dependency checks,
132 starter/component/dependency checks, and the exact published profile.
No equivalent mutants were excluded. Policy behavior was unchanged afterward;
the wire codec gained its exported shared path adapter and the starter's
presentation changed.

On iOS 26.5, `OpenAgents` simulator tests passed: three native package tests
(signature/runtime/app/replay/truncation/path rejection, independent artifact
digest checking, staged activation, interrupted-trial quarantine and corrupt
fallback) and the embedded rendered-button test. The suite membership test
also passed; `coax/native/` is now included by the shared suite discovery.

Tools, wire, layer, components, parity, state-keep, agent-reference and book
structure checks passed. The pre-push hook accepted both native commits.

## Live OTA receipt

The precompiled `CoaxNativeLive` test runner used `test-without-building`,
against the embedded starter, after publishing a separately built Coax/Wasm
module to the live feed. It checked the original heading, waited for the
native Apply prompt, applied the signed package, checked the new heading,
tapped the shared counter, waited through the 30-second foreground health
window, then terminated and relaunched the app. The new heading survived.
One test passed in 48.715 seconds; no failures.

The simulator executable's SHA-256 before and after was identical:
`0040f5123277768e7217dadf65941e2fc1103113016cb7d2f54d2de58b201dd2`.
The tested package's SHA-256 was
`a5ebcfc70ccc735fa841b7bc7276b0acaad78a513fd348b34a8395023f92d5a3`.
The public HTTPS envelope matched the locally signed envelope byte for byte.
The test result is retained at `/tmp/coax-native-live-ota.xcresult` on the Mac.

## Infrastructure and remaining acceptance

The existing `updates.openagents.com` CNAME was retained and its Cloud Run
mapping recreated for `oa-updates`; HTTPS works. The readonly GCS feed,
dedicated bucket-reader identity and pinned image are described in
[`coax/native/README.md`](../../coax/native/README.md). The private Ed25519 key
is outside the repo, with a Secret Manager backup, and is not mounted by the
public service. There are nine signed artifacts in generation 1.

The simulator receipt is not an on-device TestFlight result. The phone
installation, cached offline relaunch and later real-device login/passkey
checks are in [NEEDS_OWNER.md](../../NEEDS_OWNER.md). New native services
still require a native build. This installed profile is intended to cover
initial email/passkey/key login and chat through existing APIs; it exposes
bounded request/reply HTTP and polling, not a native streaming transport.
