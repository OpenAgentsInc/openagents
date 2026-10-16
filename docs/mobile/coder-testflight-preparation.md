# Coder TestFlight preparation

On 2026-10-16, #11850 prepared **OpenAgents Coder 1.1 (1000)** for the existing
`com.openagents.coder` App Store Connect record. No upload was performed.

The fork's [release guide](https://github.com/OpenAgentsInc/coder-desktop/blob/main/apps/ios/release/README.md)
contains the archive command, signing setup, metadata, and exact owner screens.
[Test notes](https://github.com/OpenAgentsInc/coder-desktop/blob/main/apps/ios/release/test-notes.md)
list verified actions and the remaining physical-device checks. The
[app-specific privacy notice](https://github.com/OpenAgentsInc/coder-desktop/blob/main/apps/ios/release/privacy.md)
supplements the general OpenAgents policy with phone-sync behavior.

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

The local archive and neighboring JSON proof are retained at the path recorded
in workspace `NEEDS_OWNER.md`, commit
`501a7f9456f3c650145267c50d939fa38128fd54`. That entry gives signing, candidate
version/build confirmation, App Privacy/export compliance review, upload,
TestFlight processing, and dedicated-device steps. The archive command never
exports or uploads.

The processed TestFlight build remains an owner step, as explicitly requested
in the epic. The candidate version/build must be checked against the existing
record before the owner signs and uploads. `sync.openagents.com` still reports
`CertificatePending`; the exact DNS record is also in `NEEDS_OWNER.md`.

See [the desktop/phone check](coder-desktop-phone-e2e.md) for staging and
production simulator results. Public account chats, cloud computers, and push
notifications are subsequent epic items, not claims made by this candidate.
