# AFS-11 packaged and release evidence for the Apple FM router version one

Date: 2026-07-20
Status: evidence record. This document records the runnable evidence that
landed for AFS-11 (GitHub issue #9089) and the one owner-reserved step that
stays open. It is a factual record, not a product promise. The signed and
notarized installed-application proof is an owner ceremony that an agent must
not run.

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
claim stands on. The four rungs, from weakest to strongest evidence, are:

1. `unit-tested`.
2. `integration-proven`.
3. `packaged-proven`.
4. `owner-signing-pending`, the reserved top. This is the installed, signed
   application proof that an agent cannot run.

The test `apps/openagents-desktop/src/afs-11-claim-evidence.test.ts` is the
mechanical guard. It fails when the ledger drops a version-one capability, when
a cited proof file is absent, when a claim stands above the strongest passing
proof, or when the reserved signed-release outcome asserts a passing signing
proof. The rule is that no claim sits above its actual evidence.

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

## 4. The owner-reserved signed-release proof

The AFS-11 outcome is to prove the complete system from an installed, signed
application. This step is owner-reserved. It needs the owner-held Apple
Developer ID identity `HQWSG26L43` and the notary credentials, which an agent
must never read. The exact owner steps are in the workspace owner action ledger
`NEEDS_OWNER.md`, under the AFS-11 entry, and they cite the release signing
runbook `apps/oa-updates/docs/release-signing-runbook.md`. The steps are to
confirm the signing identity, load the notary credentials, pass the release
preflight with the credentials loaded, package and sign and notarize the
application, install and launch it on a clean machine, prove the version-one
capabilities from the installed application, and record the receipt.

Issue #9089 stays open until the owner completes that run. No agent may assert a
signed-release proof that the owner has not run.

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
