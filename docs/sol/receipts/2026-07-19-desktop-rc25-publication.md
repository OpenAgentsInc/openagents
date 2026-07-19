# OpenAgents Desktop RC25 signed publication receipt

Date: 2026-07-19

## Outcome

OpenAgents Desktop `0.1.0-rc.25` is public as an immutable signed macOS arm64
GitHub prerelease:

- release: https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.25
- source revision: `1f3d9a9eddcd6c5f5aa20236e6cbabb3b474df8b`
- publication class: `desktop_experimental_prerelease`
- signed Desktop feed promoted: no
- stable channel promoted: no

The source revision was the clean `origin/main` tip when frozen and built.
Later `main` commits do not mutate this tag or either release asset. Rc.24's
unsigned experimental bytes remain unchanged; rc.25 is the strictly newer
signed correction.

## Authority and attribution

- trigger: `owner_direction`
- triggered by: OpenAgents owner request for a tested basic-IDE RC and the
  follow-up direction to use the existing release credentials
- release actor: OpenAgents release operator
- authority profile: `openagents.owner-delegated-autonomy` revision 2
- program: `program.full_auto_release`
- grant: `grant.autonomous_rc_release_and_communication`

This grant admits experimental RC publication and communication. It does not
admit stable promotion or signed-feed promotion.

## Immutable artifact manifest

| Target | Artifact | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| darwin-arm64 | `OpenAgents-0.1.0-rc.25-rc-darwin-arm64.dmg` | 204154374 | `e0788669638737c002d24ab00853e5c649e49aadaa15450ce25394394d8ddcfc` |
| darwin-arm64 | `OpenAgents-0.1.0-rc.25-rc-darwin-arm64.zip` | 206028188 | `1b7b5818e91828ae4b8d151fef4e5f4de0eb874b764e022e74d5dff370dec267` |

GitHub reported the same byte lengths and `sha256:` digests before the draft
was made public. The assets must not be replaced in place.

## Apple signing and notarization

- Developer ID: `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`
- app notarization submission: `880bde4c-bab8-40d3-94c3-eb231e85ccb8`
  (`Accepted`)
- DMG notarization submission: `38e4ef91-0f1c-42ea-a51f-149d1c75b874`
  (`Accepted`)
- nested app: stapled, deep-strict signature valid, and Gatekeeper accepted as
  `Notarized Developer ID`
- DMG: stapled and Gatekeeper accepted as `Notarized Developer ID`

The build loaded the existing scoped environment from
`~/work/.secrets/appstoreconnect.env` and the Developer ID material documented
in `apps/oa-updates/docs/release-signing-runbook.md`. No secret values enter
this receipt.

## Verification evidence

- IDE-06 focused gate: 9 test files and 120 assertions passed; all 17 language
  capabilities, cancellation fencing, supervised restart, zero leaked workers,
  and zero remote requests passed their budgets.
- Desktop corpus: 273 test files and 2,656 tests passed, with 39 explicit
  skips; TypeScript, production builds, compatibility smoke, and React smoke
  passed. The React smoke opened Files, exercised the Pierre tree and Monaco,
  returned to chat, completed a fixture turn, and started/stopped Full Auto.
- IDE-07's deterministic repository oracle had already accepted the exact
  packaged basic-IDE implementation immediately below this release-only
  version/changelog commit. The accepted evidence covers 15 daily-use classes,
  27 performance rows, architecture custody, rollback, and seven lazy chat
  launches.
- The final signed rc.25 app was exercised again after notarization. The
  packaged Monaco/workbench/language journey passed editing, quick open,
  preview pinning, built-in Vim, split views, TypeScript project evidence,
  Problems, Outline, private-scheme/offline loading, recovery, and teardown.
- Seven fresh signed-app chat-only launches passed with zero editor assets,
  renderer workers, Monaco hosts, Pierre trees, language placements, or
  project-index surfaces and zero remaining app processes. Measured shell-ready
  latency was 1620.3 ms p50, 1831.49 ms p95, and 1846.778 ms p99.

## Scope and limitations

- This is the signed macOS arm64 RC path. No new Windows, Linux, or macOS x64
  artifact is claimed by rc.25.
- Integrated terminal, debugger, tasks/tests, cloud language services, inline
  AI editing, and IDE-08+ work remain outside this candidate.
- The release does not promote the stable channel or the signed Desktop update
  feed.

## Public communication

- release notes:
  `docs/changelog/2026-07-19-desktop-0.1.0-rc.25.md`
- public changelog projection: `https://openagents.com/changelog` after the
  ordinary website deployment consumes committed `main`
- release URL:
  `https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.25`
