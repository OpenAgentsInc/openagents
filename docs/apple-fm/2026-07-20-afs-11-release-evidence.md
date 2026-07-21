# AFS-11 packaged and release evidence for the Apple FM router version one

Date: 2026-07-20
Status: evidence record. This document records the runnable evidence that
landed for AFS-11 (GitHub issue #9089) and the one owner-reserved step that
stays open. It is a factual record, not a product promise.

Update 2026-07-20: under explicit owner authorization to use the stored signing
secrets, the signing and notarization ceremony was completed for OpenAgents RC
0.1.0-rc.25 (arm64). The app and the DMG are signed with the Developer ID,
notarized by Apple, stapled, and green on Gatekeeper. Section 6 records that
receipt. The one step that stays owner-reserved is now narrower: the
interactive installed-application version-one acceptance journey.

Audience: human and agent.

## 1. What AFS-11 asks for

AFS-11 asks for the complete packaged and release evidence of the Apple FM
router version-one system, and it asks that every product claim stays at its
honest rung. The version-one cut line is in the plan
`docs/sol/2026-07-20-apple-fm-router-to-full-agent-system-plan.md`. The
version-one system is AFS-00 through AFS-06 plus the applicable packaged proof
in AFS-11, and it has these capabilities:

- One canonical local Desktop turn graph.
- Apple FM local answers.
- Apple FM route recommendations.
- Host-selected delegation to one ready `codex-local` lane.
- A local running, done, failed, refused, or cancelled card.
- A local right-pane message chain.
- Editor context and answer candidates while the file stays visible.
- IDE-08 proposals for all requested file changes.
- IDE-10, IDE-11, and IDE-12 adapters for actions and evidence.
- Private local turn, card, message-chain, and recovery storage.
- No D1, R2, Worker, Cloudflare, or other OpenAgents cloud dependency.

## 2. The claim-to-rung-to-proof ledger

The typed ledger is
`apps/openagents-desktop/src/afs-11-claim-evidence.ts`. It maps each
version-one claim to the proof that supports it and to the honest rung the
claim stands on. There are three achieved evidence strengths, from weakest to
strongest:

1. `unit-tested`.
2. `integration-proven`.
3. `packaged-proven`.

A fourth value, `owner-signing-pending`, is not an evidence strength. It is a
reserved status that means no achieved evidence yet, for the installed, signed
application journey that an agent cannot run. A numeric consumer must read the
reserved status as the weakest, not the strongest. An earlier audit found the
prior ordering ranked the reserved status above `packaged-proven`, which made
the least-proven claim read as the most-proven, so the ledger now gives the
reserved status strength zero.

The test `apps/openagents-desktop/src/afs-11-claim-evidence.test.ts` is the
mechanical, structural guard. It does not re-run the cited proofs. It fails when
the ledger drops a version-one capability, when a capability claim text is not
present in the plan cut line, when a cited proof file is absent, when a runnable
proof is not a sweep file or check script, when a claim stands above the
strongest proof kind it records as passing, when the reserved signed-release
outcome asserts a passing signing proof, or when the reserved-step reference
does not resolve to a real in-repository file. The pass or refuse verdicts of
the runnable proofs come from the normal `pnpm run check` gate that executes the
cited test files and check scripts. The ceremony verdicts come from this
committed evidence record. The rule is that no claim sits above its honest rung.

Each version-one capability claim stands at `integration-proven`. Every one
cites host or service tests that run and pass in the normal test sweep. The
packaging and staging claim stands at `packaged-proven`. The signed-release
outcome stands at `owner-signing-pending` and is blocked on the owner.

## 3. The runnable proofs that were run

These proofs ran for #9089 without any owner secret. Their results are the
evidence behind the ledger rungs.

### 3.1 Release preflight

`apps/openagents-desktop/scripts/release-preflight.ts` ran against the built
artifact set. Eight oracles passed: `clean_origin_main`, `version_monotonic`,
`attribution_intact`, `app_identity_stable`, `artifact_set_complete`,
`no_upstream_updater_remnants`, `no_legacy_ui_entrypoints`, and
`no_source_checkout_paths`. One oracle, `signing_credentials_present`, refused,
because the owner-held Developer ID identity and notary credentials are absent.
This refusal is the correct fail-closed behavior. There is no unsigned release
fallback. The preflight proves the release contract is green up to the owner
signing ceremony.

### 3.2 Isolated-app proof

`apps/openagents-desktop/src/isolated-app-proof.test.ts` ran in isolation and
passed all ten tests. It proves the double-gated isolated-app-proof profile
scopes the application data and history under the operating-system temporary
directory and never reads the operator's real history. This profile uses
Chromium's mock keychain and no real signing, so it can never prove
authenticated Sync or a signed release.

### 3.3 The claim-evidence ledger test and the boundary checks

The ledger test passed all seven tests. The Desktop `typecheck`,
`check:ide-boundaries`, and `check:afs-boundaries` checks passed. The
`check:afs-boundaries` result is also proof for the no-cloud claim, because it
refuses a cloud client, a provider SDK, a SQL driver, or an application import
in the AFS root packages.

## 4. The signed-release proof and the remaining residual

The AFS-11 outcome is to prove the complete system from an installed, signed
application. The signing part of this outcome needs the owner-held Apple
Developer ID identity `HQWSG26L43` and the notary credentials, which an agent
must never read. This section is the in-repository record for that boundary, and
the ledger reserved-step reference resolves to it. The workspace owner action
ledger `NEEDS_OWNER.md`, under the AFS-11 entry, and the release signing runbook
`apps/oa-updates/docs/release-signing-runbook.md` remain the owner ceremony
sources.

On 2026-07-20 the owner authorized use of the stored signing secrets, and the
signing, notarization, staple, and Gatekeeper steps were completed. Section 6
records that receipt.

On 2026-07-21 the interactive version-one acceptance journey was completed on a
packaged application built from `origin/main` at `fe89a057cb`, which includes
the #9155 signature-authoritative helper check (`0919324b30`, `957e646f70`).
Section 8 is the disposition. The BOOT SEQUENCE reached **4 agents ready**
(including Apple FM), and a host-selected `codex-local` delegation card moved
from running to done.

The honest residual that remains is **not** the interactive journey. The stock
stable 0.1.0 install still ships the pre-#9155 digest-first check, so Apple FM
stays unavailable on that binary until the next signed Desktop release includes
the fix. The fixed verify path accepts the stock signed helper today; only the
shipped asar is stale.

## 5. Files

- `apps/openagents-desktop/src/afs-11-claim-evidence.ts`. The typed ledger and
  the pure validators.
- `apps/openagents-desktop/src/afs-11-claim-evidence.test.ts`. The mechanical
  guard.
- `apps/openagents-desktop/scripts/release-preflight.ts`. The release oracle
  set, run for the packaged proof.
- `apps/openagents-desktop/src/isolated-app-proof.ts` and its test. The
  double-gated no-signing profile, run for the isolated proof.
- `apps/oa-updates/docs/release-signing-runbook.md`. The owner signing runbook.
- `NEEDS_OWNER.md` in the workspace root. The owner action ledger.

## 6. 2026-07-20 signing ceremony receipt

Under explicit owner authorization to use the stored signing secrets, the
signing and notarization ceremony was completed. This section is the receipt.
It carries no secret value.

- Source revision: `origin/main` at the run, a clean detached worktree.
- Product and version: OpenAgents RC `0.1.0-rc.25`, channel `rc`, target
  `darwin-arm64`, bundle identifier `com.openagents.desktop.rc`.
- Signing identity: `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`,
  hardened runtime, secure timestamp.
- Release preflight: all nine oracles green with the credentials loaded,
  including `signing_credentials_present`.
- Build command: `pnpm --dir apps/openagents-desktop run make:mac`, exit 0. The
  maker signed the app, notarized it, notarized the DMG, and stapled both.
- Apple notarization: Accepted for the app and Accepted for the DMG. Each
  staple and validate action worked.
- Independent Gatekeeper assessment, all green:
  - `codesign --verify --deep --strict` on the app is valid on disk and
    satisfies its Designated Requirement.
  - `spctl --assess --type execute` on the app is accepted, source
    `Notarized Developer ID`.
  - `spctl -a -t open --context context:primary-signature` on the DMG is
    accepted, source `Notarized Developer ID`.
  - `xcrun stapler validate` works on the app and on the DMG.
- Signed artifact boot: the signed application binary boots through the
  packaged smoke in the double-gated isolated profile, and every smoke check
  passes.
- Version-one capability tests: the 70 tests behind the AFS-11 ledger pass.
- Artifact: `OpenAgents-0.1.0-rc.25-rc-darwin-arm64.dmg`.

What remained after this ceremony was the interactive version-one journey. That
journey is now recorded in section 8.

## 7. Scope boundary of this evidence

This record is honest about what it does not certify. An earlier audit found the
release ledger risked over-certifying, so the boundary is stated here.

- This evidence covers the version-one local turn graph and the version-one
  capabilities in section 1. It does not certify the `agent_turn_receipt.v1`
  per-turn receipt, the mandatory `TurnDisclosure`, or the `TurnCostClass`
  contract. Those are the AFS-00 and AFS-01 packets, and they keep their own
  acceptance. A later packet must add a claim, a rung, and a proof for that
  per-turn contract before a release claims it.
- The no-cloud claim `C-NO-CLOUD` is proven at the package-import boundary. The
  boundary check refuses a cloud client, a provider SDK, a SQL driver, or an
  application import in the AFS root packages. It does not certify what the
  shipped route policy discloses at run time. The Desktop route policy records a
  remote-provider data destination and a metered-provider cost class on a
  provider lane, and the privacy and cost disclosure of that lane is a later
  packet, not part of this no-cloud evidence.
- The interactive version-one journey is recorded in section 8. That journey was
  proven on a packaged application that includes the #9155 fix. It does not
  claim that the stock stable 0.1.0 asar already contains that fix.

## 8. 2026-07-21 packaged version-one disposition

Disposition document:
`docs/apple-fm/2026-07-21-afs-11-packaged-disposition.md`.

Receipt directory:
`docs/apple-fm/receipts/2026-07-21-afs-11-packaged-disposition/`.

Summary:

- Source revision: `fe89a057cb` on `origin/main` (includes #9155 commits
  `0919324b30` and `957e646f70`).
- Package command: `pnpm --dir apps/openagents-desktop run package:mac`.
- Packaged smoke: `[openagents-desktop smoke] OK` (smoke mode keeps Apple FM
  off by design).
- Live packaged launch (no smoke): BOOT SEQUENCE **4 agents ready**, including
  Apple FM `apple-foundation-model`.
- Delegation: Apple FM turn completed, then `provider.codex.local` completed
  with 104 progress events; UI showed a Codex subagent card running then done,
  and a promoted answer with `via Codex subagent`.
- Residual: stock `/Applications/OpenAgents.app` 0.1.0 still lacks the #9155
  asar fix; ship it in the next signed Desktop release.
