# SBX-09 independent-verifier packet (#9033)

This record helps an independent verifier confirm the SBX-09 managed-sandbox
acceptance without repeating producer folklore. It is a proof-packaging record.
It is not a self-admission, not an issue closure, and not a production rollout.

The producer staging acceptance is already green. Issue #9033 stays open for two
human gates that a producer cannot satisfy alone:

1. An independent verifier records a disposition.
2. The owner records an explicit live-observation disposition.

Production stays default-off in every artifact this record cites. Nothing here
enables a production flag, a public availability claim, or SBX-10.

## Sources of truth

- Producer aggregate:
  [`docs/sol/evidence/2026-07-20-sbx09-live-acceptance.json`](../sol/evidence/2026-07-20-sbx09-live-acceptance.json)
- Accepted plan:
  [`docs/sol/2026-07-19-managed-agent-sandboxes-accepted-plan.md`](../sol/2026-07-19-managed-agent-sandboxes-accepted-plan.md)
- Managed-sandbox invariants:
  [`docs/cloud/INVARIANTS.md`](./INVARIANTS.md)
- Domain contract:
  [`docs/cloud/contracts/openagents.managed_sandbox.v1.md`](./contracts/openagents.managed_sandbox.v1.md)
- ASAP sequence and gate framing:
  [`docs/grok/2026-07-21-open-issues-audit-and-asap-sequence.md`](../grok/2026-07-21-open-issues-audit-and-asap-sequence.md)

## What the producer staging acceptance already proved

The aggregate records `stagingLiveAcceptance: passed` and
`phase1Implementation: passed`. The producer ran the live Google Cloud harnesses
against the staging project with an explicit owner cost gate. The aggregate also
records the honest residual state:

- `independentAssurance: inconclusive`
- `publicReleaseGate: blocked`
- `productionRollout: not_attempted`
- `phase2Activation: blocked`

The aggregate holds public-safe digests, counts, and observations. It does not
hold raw sandbox refs, credentials, topology secrets, prompts, or runtime output.

## The four acceptance claims

SBX-09 makes four top-level acceptance claims. This packet maps each claim to
the exact producer evidence and to a re-confirmation path.

1. Live Google Cloud acceptance (Box SDK compatibility and native lifecycle over
   a real GCE workload).
2. Isolation between tenants and generations.
3. Cleanup and teardown to zero residue.
4. Rollout posture that stays default-off.

Two disposition kinds appear below:

- RUNNABLE means an independent verifier reproduces the evidence now, on any
  developer host, with no live Google Cloud cost and no owner secret. These
  checks confirm the structural contract, the isolation logic, the cleanup
  logic, and the default-off posture.
- NEEDS-OWNER means the live Google Cloud portion is billable and owner-gated.
  A verifier cannot boot a real GCE VM without an owner cost approval. These
  rows require owner live observation or an owner-approved independent re-run.

## RUNNABLE checklist (independently re-confirmable now)

Run every command from a clean checkout at the current `origin/main` after
`pnpm install --config.confirmModulesPurge=false`.

| ID | Claim area | Re-confirm command | Expected result | Producer evidence in aggregate |
| --- | --- | --- | --- | --- |
| R1 | live acceptance (structural), isolation, cleanup | `npx vp test --run packages/managed-sandbox-contract/src` | 5 files, 19 tests pass | contract identity behind `boxCompatibility`, `nativeLifecycle` |
| R2 | isolation, default-off, cleanup receipts | from `apps/openagents.com/workers/api`: `npx vitest run src/managed-sandbox-box-v1-routes.test.ts src/managed-sandbox-provider-broker.test.ts src/managed-sandbox-broker.test.ts src/managed-sandbox-desktop-routes.test.ts src/managed-sandbox-supervision-routes.test.ts src/sarah-managed-sandbox.test.ts` | 6 files, 38 tests pass | `boxCompatibility.proof.*Denied`, `sarahJourney`, `desktopJourney` |
| R3 | isolation (cross-owner denial) | `npx vp test --run packages/authority/src/managed-sandbox-authority.test.ts` | 1 file, 3 tests pass | `crossOwnerDenied`, `crossOwnerDeniedBeforeEffect` |
| R4 | lifecycle and cleanup store | `npx vp test --run packages/khala-sync-server/src/managed-sandbox-store.test.ts` | 1 file, 7 tests pass | `nativeLifecycle`, `reconciliation` |
| R5 | isolation, cleanup, default-off (control plane) | `cargo test -p oa-codex-control managed_sandbox` | 19 unit and 2 integration tests pass, including `managed_sandbox_route_is_authenticated_and_default_off_without_fake_readiness` | `nativeLifecycle`, `faultMatrix`, `runtimeIdentity` |
| R6 | rollout and default-off | `grep -nE "MANAGED_SANDBOX_BROKER_ENABLED\|MANAGED_SANDBOX_BOX_V1_ENABLED\|OA_MANAGED_SANDBOX_IMAGE_DIGEST" apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh` | no match (grep exit 1). The production deploy omits all three enable flags, so the broker, the facade, and the image binding stay off | `verdicts.productionRollout: not_attempted` |
| R7 | rollout and default-off (billing guard) | `grep -nE "OA_MANAGED_SANDBOX_OWNER_GATE\|I_ACCEPT_LIVE_GCP_COST" apps/openagents.com/workers/api/scripts/managed-sandbox-box-live-acceptance.ts apps/openagents.com/workers/api/scripts/managed-sandbox-sarah-live-acceptance.ts apps/openagents.com/workers/api/scripts/managed-sandbox-reconcile-live.ts` | each live harness refuses unless the caller passes `--apply` and sets the owner cost gate. A verifier cannot trigger a billable run by accident | `rollback`, `reconciliation.defaultOff: true` |
| R8 | aggregate consumption and public-safety | read `docs/sol/evidence/2026-07-20-sbx09-live-acceptance.json` and confirm the four blocked or inconclusive verdicts and the `limitations` list. Confirm the file holds only digests, counts, revisions, and observations | verdicts and limitations read as recorded, and no raw ref, credential, prompt, output, or topology secret is present | whole aggregate |

All eight RUNNABLE rows were exercised during the assembly of this packet and
returned the expected result.

### Claim coverage from the RUNNABLE rows

- Claim 1 (live acceptance): R1, R2, R4, R5 confirm the contract, the SDK
  translator, the lifecycle events, and the driver logic in deterministic form.
  They do not boot a real GCE VM. The live boot is O1 and O2.
- Claim 2 (isolation): R1, R2, R3, R5 confirm cross-owner and cross-generation
  denial before effect, and the fail-closed default-off route. The live network
  isolation on a real VM is O3.
- Claim 3 (cleanup): R1, R4, R5 confirm the residue oracle and the teardown
  contract. The live zero-residue scan of the Google Cloud project is O4.
- Claim 4 (rollout and default-off): R5, R6, R7, R8 confirm the posture end to
  end without any live cost. The live staging rollback re-run is O5, and the
  owner disposition itself is O6.

## NEEDS-OWNER checklist (owner live observation or owner-approved re-run)

Each row below is billable and owner-gated. The producer evidence is present in
the aggregate. An independent verifier confirms these only with an explicit
owner cost approval and the owner gate value, or the owner observes them
directly. A verifier who lacks that approval records these rows as observed by
producer evidence and pending owner disposition. Production stays default-off
throughout.

| ID | Claim area | Owner-gated re-run (billable, do not run without owner cost approval) | Producer evidence in aggregate |
| --- | --- | --- | --- |
| O1 | live acceptance (Box SDK) | `pnpm --dir apps/openagents.com/workers/api run accept:managed-sandbox-box-live --apply` with the owner cost gate set | `boxCompatibility` (26 proof bits true, residue all zero) |
| O2 | live acceptance (native lifecycle) | the owner-gated control lifecycle harness that produced the `create`, `probe`, `stop`, `resume`, `delete` journey | `nativeLifecycle` (journey, `finalGeneration`, `cleanupObserved`) |
| O3 | isolation on a live VM | live observation of the no-external-IP workload and the broker-only firewall priority order on the real VM | `faultMatrix.proof.ownerTenantIsolation`, `generationIsolation`, `runtimeIdentity.networkPolicyRef` |
| O4 | cleanup on the live project | `pnpm --dir apps/openagents.com/workers/api run reconcile:managed-sandbox-live --apply --sandbox-ref REF` with the owner gate, then confirm zero project inventory | `reconciliation.observations` (three exact-ref reconciliations to zero) and `faultMatrix.inventoryBeforeAndAfter` |
| O5 | rollout (staging rollback) | live observation of the staging cutover where the authenticated boundary moves 401 to 404 and back with zero inventory throughout | `rollback.before`, `rollback.disabled`, `rollback.restored` |
| O6 | owner disposition | owner observes the Sarah create, dispatch, settle, delete journey and the Desktop journey, then records acceptance | `sarahJourney`, `desktopJourney` |

### Caveat a verifier must read before O4

The aggregate records one honest caveat under `sarahJourney`. The Sarah journey
own-lifecycle proof passed. When the first global inventory oracle ran, it
observed one active sandbox that a separate signed-in Desktop consumer owned.
The Sarah-owned resource was already deleted at that moment. A later exact-ref
reconciliation returned the global inventory to zero. The distinction is
global-scan versus exact-ref reconciliation. A verifier must not read the first
global observation as Sarah residue. The `concurrentResidueDisposition` block
and `reconciliation.observations` hold the exact record.

## The public-safe aggregate the verifier consumes

The single aggregate the verifier reads is
[`docs/sol/evidence/2026-07-20-sbx09-live-acceptance.json`](../sol/evidence/2026-07-20-sbx09-live-acceptance.json).
Its schema is `openagents.managed_sandbox_sbx09_live_acceptance_aggregate.v1`.
It binds the pinned source and deploy revisions, the runtime identity digests,
the Box compatibility proof, the fault and cost matrix, the native lifecycle,
the Sarah and Desktop journeys, the reconciliation observations, the rollback
record, and the assurance disposition. The `limitations` list states the exact
boundary of what the run proves. The verifier treats this file as the public
projection and reconciles each cited digest and count back to the RUNNABLE
checks above.

## Count summary

- Four acceptance claims.
- Fourteen checklist rows across the four claims.
- Eight rows map to independently-runnable evidence and were re-confirmed during
  packet assembly (R1 through R8).
- Six rows need owner live observation or an owner-approved billable re-run (O1
  through O6).

## Default-off guarantee

Nothing in this packet enables a production rollout. The production deploy omits
the broker flag, the facade flag, and the image digest binding (R6). The live
harnesses refuse without the owner cost gate (R7). The aggregate records
`productionRollout: not_attempted` and `phase2Activation: blocked`. The public
release gate stays blocked until an independent verifier and the owner record
the two required dispositions.

## What this packet does not do

- It does not admit SBX-09.
- It does not close #9033.
- It does not enable any production flag.
- It does not run a live Google Cloud deploy.
- It does not print a secret, a raw ref, or a topology detail.
