# OpenAgents mobile TestFlight build 128 Sarah command-center release evidence

Date: 2026-07-29

Status: App Store Connect accepted build 128. Internal TestFlight testing is
active.

## Release decision

Build 128 adds the admitted mobile Sarah command-center profile to the
supported OpenAgents iOS app. It uses the canonical owner-private Sarah thread
and exposes only the existing receipted coding-worker and Full Auto brokers:

- inspect coding-worker capacity;
- start a coding worker;
- inspect coding-worker status;
- inspect Desktop Full Auto status; and
- queue pause, resume, or stop intent for Desktop Full Auto.

The phone does not receive workspace, editor, shell, Git, credential, payment,
account, or device authority. The paired Omega bridge remains read-only. Sarah
does not report a queued Full Auto intent as applied.

Final user, Sarah, and tool-activity items are serialized to local JSONL. Audio
is not stored. A 15-second client heartbeat, stable-session reconnect-budget
reset, and serialized shutdown tails cover the previously observed mobile
session-expiry and incomplete-transcript paths.

## Candidate identity

- App: `OpenAgents`
- Bundle identifier: `com.openagents.app`
- Marketing version: `0.5.2`
- Build number: `128`
- Apple team: `HQWSG26L43`
- App Store Connect app ID: `6748620735`
- Binary source commit: `a61bb35abd59157b03c3604d4cf7d66ad2dfa8df`
- Source branch: `codex/mobile-sarah-parity`
- Published source branch: `main`

Before the native archive, the release worktree was clean, its source commit
equaled `origin/main`, and App Store Connect's latest build was 127.

## Service and data-plane evidence

Database migration `0106_sarah_mobile_command_center_profile.sql` was applied
to staging. The exact binary source commit was deployed as Cloud Run revision
`openagents-monolith-staging-00152-mjt`, receiving 100 percent of staging
traffic at:

```text
https://openagents-monolith-staging-ezxz4mgdsq-uc.a.run.app
```

The deployment health endpoint passed and the retired `/sarah` surface
remained a 404 tombstone. A browser smoke verified the logged-out boundary.

Synthetic live owner-command execution was not claimed: available local Nostr
identities did not match the configured staging owner, and dedicated mobile
test-agent credentials are not user bearer credentials. The release operator
did not weaken owner authentication or re-enable Nostr self-provision to make
that test pass. The authenticated command profile is instead covered by the
focused server and behavior-contract tests below.

## Verification

The following checks passed from the release worktree:

```text
pnpm --dir apps/openagents-mobile run typecheck
pnpm --dir apps/openagents-mobile run test
```

Mobile tests passed 71 tests in 13 files. Focused server and contract checks
passed 83 tests in 5 files. The API server bundle succeeded. The protected
`main` push guard also passed lint, protocol, assurance, mobile typecheck, and
the 71 mobile tests.

The repository-wide `pnpm run check` was attempted but remains blocked by
pre-existing, unrelated Effect language-service diagnostics. This release does
not claim that repository-wide check as green.

A clean Release simulator build succeeded. Its launch showed build 128 and the
Sarah voice staging entry point. Opening Sarah showed the ready screen,
command-activity persistence disclosure, and the closed unauthenticated error
boundary. The simulator does not possess the canonical user identity, so this
was not an authenticated voice test.

The device archive installed on the paired `Christopher's iPhone` and launched
successfully with bundle identifier `com.openagents.app`. This proves device
installation and launch, but does not claim that the release operator heard an
authenticated voice reply on the physical device.

## Build and upload evidence

- Archive command result: success.
- Archive Xcode store validation: success.
- Archive signature check: valid on disk and satisfies its designated
  requirement.
- Archive identity: `com.openagents.app`, version `0.5.2`, build `128`.
- `ITSAppUsesNonExemptEncryption`: `false`.
- Export command result: success.
- Export profile: `com.openagents.app AppStore`.
- Export signing identity: Apple Distribution for team `HQWSG26L43`.
- Export signature check: valid on disk and satisfies its designated
  requirement.
- Expo runtime fingerprint:
  `e3745288b368a01e96e264111d370818c6bd1e5f`.
- IPA SHA-256:
  `a46eb571560a815879742077d510cd309be806f2482b7c941278a6f2a9ee55d2`.
- IPA size: `19353127` bytes.
- Upload result: `UPLOAD SUCCEEDED with no errors`.
- Delivery UUID: `83167607-ab5f-4e59-a145-0d273816ae4a`.

The App Store Connect environment and private key were read from the dedicated
local secrets directory. This record contains no credential values.

## TestFlight state

App Store Connect returned this exact build record:

```text
build id: 83167607-ab5f-4e59-a145-0d273816ae4a
uploaded date: 2026-07-29T14:36:48-07:00
minimum iOS version: 16.4
processing state: VALID
internal build state: IN_BETA_TESTING
external build state: READY_FOR_BETA_SUBMISSION
auto notify enabled: true
```

Build 128 is available to configured internal TestFlight testers. It is not in
external beta review.

## Physical acceptance

Install build 128 from TestFlight and use the existing owner identity. Confirm
an audible Sarah exchange, then ask Sarah to inspect coding-worker capacity,
start a bounded worker, inspect its status, and inspect or queue a Full Auto
control intent. Confirm each tool activity item and the local transcript after
ending and reopening the session. This user-observed audio and owner-auth path
remains the final physical acceptance; this release record does not fabricate
that observation.
