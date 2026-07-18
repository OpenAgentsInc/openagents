# OpenAgents mobile TestFlight build 119 release evidence

Date: 2026-07-18

Build 119 was generated and submitted before the Sarah terminal-history
harness implementation began, preserving an exact release-source boundary.

## Artifact identity

- Application: OpenAgents mobile
- Bundle identifier: `com.openagents.app`
- Marketing version: `0.5.2`
- Build number: `119`
- Source commit: `a728099ffbeaf39c403e192d4fcfe076cbfa18d6`
- Expo fingerprint: `4f8ebe4ae66dc08a4c34411d25df85b22394f946`
- Archive: `/tmp/OpenAgents-119-a728099.xcarchive`
- IPA: `/tmp/OpenAgents-119-a728099-export/OpenAgents.ipa`
- IPA bytes: `19,202,930`
- IPA SHA-256: `86cae1a984710624c1a9384182f147fdb60829828d7859f2ca8aeb3ad24866d4`

## Apple receipt

- Delivery/build ID: `17ba03ee-2e51-4a54-b0da-7a30ebacf259`
- Uploaded at: `2026-07-18T14:37:08-07:00`
- App Store Connect processing state: `VALID`
- Expired: `false`

The App Store Connect API returned the exact `0.5.2 (119)` build record in
`VALID` state after upload. This proves Apple accepted the bytes for TestFlight
processing; it does not claim App Review, stable App Store publication, or a
different binary than the digest above.

## Gates

- OpenAgents mobile test suite: 57 files, 283 tests passed.
- OpenAgents mobile TypeScript check: passed.
- Xcode archive: passed.
- Manual App Store distribution export/signing: passed.
- Apple upload: passed.
- App Store Connect exact-build verification: passed (`VALID`).

## Authority and limitations

The owner explicitly directed generation and TestFlight submission in the
current thread. Release actor: operating/release agent. The submission is a
mobile candidate only; it does not promote Desktop stable, alter the signed
Desktop update feed, or authorize a stable App Store release.
