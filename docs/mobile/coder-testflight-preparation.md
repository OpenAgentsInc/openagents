# Coder TestFlight preparation

On 2026-10-16, the owner authorized TestFlight upload and requested
**OpenAgents Coder 1.0.0 (1000)** for the existing `com.openagents.coder` record.
Fork commit `f8788f1594632237a3702677bb5b68d00d26341e` sets matching app and
extension versions and adds explicit distribution-certificate selection.
The signed Release archive passed verification. Apple accepted the package;
App Store Connect reports the build as `VALID`, uploaded at
`2026-10-16T06:48:55-07:00`.

The fork's [release guide](https://github.com/OpenAgentsInc/coder-desktop/blob/main/apps/ios/release/README.md)
contains the archive command, signing setup, metadata, and exact owner screens.
[Test notes](https://github.com/OpenAgentsInc/coder-desktop/blob/main/apps/ios/release/test-notes.md)
list verified actions and the remaining physical-device checks. The
[app-specific privacy notice](https://github.com/OpenAgentsInc/coder-desktop/blob/main/apps/ios/release/privacy.md)
supplements the general OpenAgents policy with phone-sync behavior.

[Submission receipt](evidence/11850-testflight-1.0.0-1000.json) records the
processed build and remaining checks. The existing Internal Testers group has
access to all builds, but compliance must clear before installation. Updating
What to Test returned an Apple HTTP 500; the source notes remain in the fork.
Workspace owner steps: `8bb2ae8cec04fe15c38586df038de8243213a9d8`.

## Verification

- Fork source `0a18cc7328a2b09d0b7208ab2006f9c6655fbc8b`: unsigned iPhone
  Release archive succeeded with the Rust `mobile-dist` profile and
  `CODE_SIGNING_ALLOWED=NO`. Mach-O platform is iOS, minimum iOS 26.0.
- Bundle identity, icon assets, attribution, extension bundle/version match,
  privacy declarations, and upstream endpoint scan passed. No simulator
  credential fixture or notification permission request is in the app binary.
- Six disposable-bundle release tests passed: valid metadata, wrong bundle,
  extension version mismatch, missing collection disclosures, tracking, and
  fixture/push permission code rejection.
- Source metadata check passed; collected-data type names were checked against
  Apple's published definitions. Push entitlement is absent. No signing team
  is committed, and no owner signing identity was used.
- Fork `9d477de3b88f64e52e761c6ce0221762bba12ea6` adds the public privacy notice
  and its release metadata URL; application/archive inputs are unchanged.

## Signed submission

The signed archive is retained at
`~/.openagents/scratch/claude-code-8b7600c6-d8dd-458a-9c69-605683782c65/11850-Coder-1.0.0-1000.xcarchive`.
The neighboring JSON records the source and archive checks. The exported IPA
includes the Live Activity extension and debug symbols. Both bundles use the
OpenAgents distribution identity and App Store profiles; `get-task-allow`
and push entitlements are absent. Strict signature verification passed.
Six release tests and the source metadata check passed again.

Xcode's upload stalled before transferring the package. The official upload
API accepted all six chunks. Its finalization request returned HTTP 500,
but subsequent reads reported `PROCESSING` for upload
`a5432e8a-033f-40ef-90d5-302ba3b02b26`. The upload receipt is separate from the
archive proof: the archive command itself never uploads. Apple's build record now confirms `VALID` for 1.0.0 (1000).
Internal testing is blocked by `MISSING_EXPORT_COMPLIANCE`; the owner must
complete Apple's declaration before testers can install it. No exemption
answer was guessed or submitted.

The earlier unsigned 1.1 archive remains historical verification evidence.
The owner requested 1.0.0 before the signed archive was built. Workspace
`NEEDS_OWNER.md` tracks the physical-device check and any compliance question.
`sync.openagents.com` has its required DNS-only CNAME, but Google's certificate
still reports `CertificatePending`. Both run.app services remain available.

See [the desktop/phone check](coder-desktop-phone-e2e.md) for staging and
production simulator results. Public account chats, cloud computers, and push
notifications are subsequent epic items, not claims made by this candidate.
