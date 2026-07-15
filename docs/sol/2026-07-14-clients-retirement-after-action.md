# Final `clients/` retirement — after-action analysis

Date: 2026-07-14  
Decision authority: owner direction in the active session  
Pre-removal recovery commit: `bbb450df1c3158178c93039540969b1ed629734f`

## Decision

All three remaining applications under `clients/` are retired and removed:

- `clients/khala-cli`
- `clients/khala-ios`
- `clients/khala-mobile`

This supersedes the earlier parity-before-deletion hold. Git is the archive;
the working tree is no longer a museum for superseded product shells.

## Why the cut was broader than three directories

The client trees still owned active authority outside their directories. The
Khala CLI was packaged and advertised by install pages and the product-promise
registry. The old React Native app still had QA/nightly and visual-baseline
lanes. Pylon and the control-protocol package still pointed at an already
missing iOS build-and-submit script. The Worker emitted `khala://` notification
links even though the supported app registers `openagents://`.

Deleting only the folders would therefore have left false install copy, broken
scheduled commands, and a notification deep link no supported app could open.

## What changed

- Removed all client workspace importers, root scripts, lockfile importers, and
  the Khala public CLI artifact.
- Replaced public terminal onboarding with `@openagentsinc/pylon`,
  `pylon auth codex`, and `pylon accounts list --json`.
- Withdrew stable promise `khala.cli_terminal_client.v1`; its historical ID and
  evidence remain, with Pylon named as successor.
- Removed client-exclusive mobile nightly, Maestro/emulator, and visual-tier
  code. The supported OpenAgents mobile typecheck/test gate replaces the dead
  pre-push path.
- Removed the obsolete EAS/native rebuild planners. Pylon keeps owned OTA
  publishing for `apps/openagents-mobile`, while native rebuild decisions now
  escalate to the current release owner instead of executing a guessed script.
- Moved icon and identity evidence to `apps/openagents-mobile`, removed the
  `khala://auth` rollback redirect, and changed notification links to
  `openagents://thread/...`.
- Preserved shared Effect Native, Khala Sync, runtime, protocol, and QA
  packages that still have supported consumers.

## Deliberately retained

- Dated audits, receipts, transcripts, and frozen/admitted AssuranceSpec bytes.
  They describe real historical paths and are not runtime dependencies.
- Stable data taxonomy such as historical `khala-cli` source labels where
  changing stored provenance would corrupt interpretation.
- The `openagents-khala-mobile` OAuth client-id string used by installed
  OpenAgents mobile builds. The identifier is compatibility data; the removed
  `khala://` redirect was the retired-client behavior.
- Negative legacy markers in release guards, including
  `clients/khala-mobile`, so removed UI cannot re-enter a packaged artifact.

## Capability tradeoff

The deleted old mobile tree contained its only copies of the local Apple
Foundation Models and push-to-talk STT native modules. The supported mobile app
does not consume them. This change intentionally gives up those unported
implementations instead of retaining an entire deprecated app as a speculative
component library. Any future capability must enter through an owned shared
package and a current-app contract.

## Recovery and successors

Recover historical bytes without restoring product authority:

```sh
git show bbb450df1c3158178c93039540969b1ed629734f:<path>
```

Current destinations:

- terminal/local Codex capacity: `apps/pylon` / `@openagentsinc/pylon`
- mobile: `apps/openagents-mobile`
- desktop: `apps/openagents-desktop`
- web and public promise authority: `apps/openagents.com`

The supported mobile production procedure is
[`docs/deploy/openagents-mobile-production-release.md`](../deploy/openagents-mobile-production-release.md).

## Verification and deploy boundary

The retirement guard, QA matrix/pre-push checks, Sol document policy, affected
package typechecks, OpenAgents mobile (126 tests), Start (180 tests), Pylon
(2,157 tests), behavior contracts (36 tests), and the focused API
promise/auth/push suite (47 tests) passed on Node 24.13.1.

The initial after-action incorrectly described a retired edge-provider quota as
a production migration blocker. That was false: Google Cloud was already the
production authority, and Cloud SQL was the live database. The attempted edge
deployment was itself the error; its staging upload was deleted and was never a
valid production promotion path.

The corrected deployment boundary is the Google Cloud Run monolith. The
repository now rejects edge-provider packages, configs, deploy commands, and
active SHC references before the Cloud Run build begins. SHC is recorded only
as a retired limited pilot, never as primary infrastructure.
