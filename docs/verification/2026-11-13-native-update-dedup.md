# Fresh TestFlight installs and identical OTA packages

## Cause

Build 1072's released archive contains exactly the five resources in its live
signed generation-1 feed. On 2026-11-13 both `updates.openagents.com` and the
pinned Cloud Run fallback returned manifests whose file sizes and SHA-256
digests all matched the archive's `Starter/` directory. Its runtime is
`f2e09cdfd651f8e9cca9b34a9b2d37198a5dc8e273fb5ea45c4371e2c2d03060`.

The updater only compared the feed generation with `highwater.cnu`, which
does not exist on a fresh install. Consequently generation 1 was downloaded,
staged and offered even though the bundled resources were already identical.
The automatic check runs after the first interactive module, explaining the
prompt a few seconds after launch. The TestFlight app was already up to date.

## Fix

After signature, app and runtime verification, the native adapter observes the
size and digest of every manifest file in the currently running directory.
This is the embedded bundle when no OTA package is active. The existing shared
Coax authority now decides between already-seen generation, identical resources
and changed resources. Identical resources only persist the verified generation
high-water mark: no artifact downloads, pending package or activation prompt.
Changed resources retain the existing download, integrity, staging, activation
and trial-recovery path. Signature/runtime/generation admission is unchanged.

No new wire shape, signing key or update service is introduced. The native
adapter/authority change requires a new installed runtime, build 1073; it cannot
repair build 1072's updater through application OTA.

## Verification

- Remote targeted `coax/native/updates-policy.coax` suite passed.
- Explicit authority mutation: **75 mutants, 75 killed, zero survivors**.
- `NativePackageTests`: **5 tests passed** on the headless iOS simulator. The
  new regressions cover fresh bundled content, generation persistence across
  restart, genuinely changed content, stale generations, an already-running OTA
  package, corrupt/missing resources and absence of staged files on a match.
  Existing signature/runtime/app/path refusal and interrupted-trial recovery
  tests also passed.
- `tools-check` passed with zero new tools missing a firm reason.
- Fresh-device live-feed UI regression passed: launch with no update prompt,
  manual check reports the latest compatible update, relaunch and check again
  with no activation alert. This used a new headless simulator and the actual
  published production-key feed, not an empty/missing feed.
  After the run, its update store contained only the verified generation-1
  `highwater.cnu`: no downloaded objects, pending/active package or trial files.

Native test evidence: `/private/tmp/oa-update-prompt-tests.xcresult` and its
companion log; live UI evidence: `/private/tmp/oa-update-prompt-live.xcresult`.
Copies of both result bundles and release/test logs are retained with the release.
Release artifacts are kept under
`~/.openagents/releases/openagents-ios-1073/`.

## Release

Build 1073 preserves bundle `com.openagents.app`, version 1.0.0, icons,
credentials and account/chat functionality. Its runtime feed is separate from
1072's feed. Runtime:
`4a481b72f086cdb0ceff8912bafb7d68fd4de977ecc6f1ff8e9e9359a2182dfb`.

The archive/export/upload succeeded. Apple reports **VALID / IN_BETA_TESTING**,
and the build and release notes are verified in the existing **Owner** group.
Both public endpoints returned the exact signed generation-1 envelope and all
five immutable artifacts, byte-for-byte equal to the archived embedded files.
The feed remains published: the fix recognizes matching code rather than hiding
or removing the compatible update. Older runtime feeds are unchanged.
