# OpenAgents mobile TestFlight build 127 Sarah voice release evidence

Date: 2026-07-28

Issue:
[OpenAgentsInc/openagents#9273](https://github.com/OpenAgentsInc/openagents/issues/9273)

Status: App Store Connect accepted build 127. Internal TestFlight testing is
active.

## Release decision

The owner directed the release operator to upload build 127 before the
physical iPhone acceptance test. The physical iPhone was connected and paired,
but it was locked. Apple device control returned
`kAMDMobileImageMounterDeviceLocked`.

This release does not change a production service. The app uses the staging
Sarah gateway:

```text
https://openagents-monolith-staging-ezxz4mgdsq-uc.a.run.app
```

## Candidate identity

- App: `OpenAgents`
- Bundle identifier: `com.openagents.app`
- Marketing version: `0.5.2`
- Build number: `127`
- Apple team: `HQWSG26L43`
- App Store Connect app ID: `6748620735`
- Source commit: `56f742d9228e162a034be6b85219133f59b8a81a`

The release worktree was clean. Its source commit was equal to `origin/main`.
App Store Connect did not contain build 127 before the upload.

## Verification

The following mobile checks passed from the release worktree:

```text
pnpm --dir apps/openagents-mobile run test
pnpm --dir apps/openagents-mobile run typecheck
```

The test command passed 56 tests in 11 files. The TypeScript check passed.

The generated and exported app has this microphone purpose:

```text
Allow OpenAgents to use the microphone for live conversations with Sarah.
```

The exported JavaScript bundle contains the staging gateway URL. It does not
contain the production Sarah voice URL.

The signed simulator test showed the microphone permission prompt with the
same purpose text. It also verified fresh protected-device identity creation
and the closed identity-bootstrap error. The simulator test did not prove
audible speaker output.

The staging service has the compatible Sarah voice profile. Its default
traffic uses revision `openagents-monolith-staging-00143-j2h`. That revision
has Nostr self-provision disabled. The staging database still has six Nostr
users, so the bounded test attempts did not create an identity.

The release operator removed the `sarah-bootstrap` traffic tag after the
bounded bootstrap test. The default staging URL sends 100 percent of traffic
to revision `openagents-monolith-staging-00143-j2h`. No alternate traffic tag
is active.

## Build and upload evidence

- Archive command result: success.
- Export command result: success.
- Archive signing identity: Apple Development.
- Export profile: `com.openagents.app AppStore`.
- Export signing identity: Apple Distribution.
- Export signature check: valid on disk and satisfies its designated
  requirement.
- Expo runtime fingerprint:
  `4d325a423edf99c9fcdadf67ed6b8b0bcb743dfc`.
- IPA SHA-256:
  `e7d41ae7d802c5f6c4757158c65d08686dcb44c50f289acd32c397d7e638537e`.
- IPA size: `19318812` bytes.
- Upload result: `UPLOAD SUCCEEDED with no errors`.
- Delivery UUID: `588bcbad-847f-4abd-b5a5-87ef44cf4aaa`.

The App Store Connect environment file and private key file had mode `0600`
before use. This record does not contain credential values. The release
operator did not use a Keychain probe.

## TestFlight state

App Store Connect returned this build record:

```text
build id: 588bcbad-847f-4abd-b5a5-87ef44cf4aaa
uploaded date: 2026-07-28T15:44:24-07:00
minimum iOS version: 16.4
processing state: VALID
internal build state: IN_BETA_TESTING
external build state: READY_FOR_BETA_SUBMISSION
```

Build 127 is available to the configured internal TestFlight testers. The
build is not in external beta review. The API key does not have permission to
list the beta groups, so this record does not name the internal groups.

## Residual physical acceptance

After the owner unlocks the paired iPhone, install build 127 from TestFlight.
Use the dedicated funded staging identity. Verify these items:

1. Confirm the microphone purpose and select Allow.
2. Start a Sarah voice session.
3. Confirm microphone capture and an audible Sarah reply.
4. Confirm the user and Sarah transcripts.
5. Confirm mute, interrupt, end, and retry.
6. Confirm cleanup when the app enters the background.
7. Confirm the denied, expired-session, credit, and network errors.

This physical acceptance is a user-owned residual step. This release record
does not claim that it passed.
