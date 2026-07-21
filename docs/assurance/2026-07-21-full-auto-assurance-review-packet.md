# FA-AS-01 independent-review proof packet (Full Auto AssuranceSpec)

Issue: <https://github.com/OpenAgentsInc/openagents/issues/8978>
AssuranceSpec under review: `specs/desktop/full-auto.assurance-spec.md`
(assurance_revision 6, `lifecycle_state: proposed`).
Bound ProductSpec: `specs/desktop/full-auto.product-spec.md` (spec_revision 14,
digest `sha256:5da9eba0601be1b2fe849dce96260e8a785d16590a60298d8180faf9442dba47`).

## Purpose

This packet gives an independent reviewer a direct, runnable map from each Full
Auto acceptance criterion to the exact executable oracle or retained receipt
that stands as its evidence. The reviewer can run each command and read
green or red without folklore. This packet is proof packaging only. It does
NOT admit the AssuranceSpec and it does NOT change any release or public-claim
state.

The producer of an assurance obligation cannot verify or admit that same
obligation. This rule is a hard invariant (root `AUTHORITY.md`
`grant.independent_assurance`, `docs/assurance/ASSURANCE_SPEC.md` Law 10, and
the issue text: "the implementation/analyzer cannot self-admit"). The agent
that assembled this packet is a producer. Admission stays with the owner or an
owner-designated independent reviewer who is distinct from the producer.

## Status summary

The AssuranceSpec binds all 76 `FA-AC-*` criteria to a fully designed
obligation (validate and coverage both report 76/76 ready, 0 need design). This
packet separates design readiness from executable observation and reports the
honest current status of the evidence behind each obligation.

| Status | Count | Meaning |
| --- | --- | --- |
| executable-green | 61 | The named oracle is a real test that this pass ran and observed GREEN. |
| executable-red | 0 | No oracle ran red in this pass. |
| smoke-gated | 2 | The oracle is the real two-OS-process restart smoke. It is gated at `GATE-DEV-TWO-PROCESS`, unsigned dev-mode, and was not re-run in this packet pass. |
| receipt-backed | 5 | Evidence is a retained owner-real development-tier receipt, not a re-runnable test. It awaits independent admission and a signed-package upgrade. |
| designed-only | 8 | The named production and oracle seam does NOT exist in the repository yet. The obligation is a design only. |

The 8 designed-only criteria are the MemoHarness cluster (FA-AC-69 through
FA-AC-76) added by ProductSpec rev 14. Their named oracle file
`apps/openagents-desktop/src/full-auto-harness-policy.test.ts` is absent from
the repository, confirmed by direct file check. The AssuranceSpec discloses
this same fact in its `custom-formal-model-status` and Objective sections. This
packet does not paper over that gap and does not manufacture a stand-in test
for a seam that does not exist.

## How to reproduce the 61 executable-green oracles

Setup from a clean checkout at current `origin/main`:

```sh
git fetch origin main
git worktree add --detach /tmp/oa-review-8978 origin/main
cd /tmp/oa-review-8978
pnpm install --config.confirmModulesPurge=false
```

Run the 21 Desktop oracle files that carry 60 of the 61 green criteria in one
command (observed this pass: 21 files passed, 516 tests passed, 11 skipped):

```sh
cd apps/openagents-desktop
./node_modules/.bin/vp test --run --root ../.. \
  src/renderer/react-composer.test.tsx \
  src/codex-local-runtime.test.ts \
  src/renderer/shell.test.ts \
  tests/full-auto-restart.e2e.test.ts \
  tests/full-auto-registry.test.ts \
  src/full-auto-lane.test.ts \
  src/full-auto-retry-rotation-model.test.ts \
  src/renderer/react-full-auto-surface.test.tsx \
  src/full-auto-control-server.test.ts \
  src/provider-lane.test.ts \
  src/spec-lane-workflow.test.ts \
  tests/full-auto-run-registry.test.ts \
  tests/full-auto-thread-pressure.e2e.test.ts \
  src/full-auto-run-control-server.test.ts \
  src/full-auto-run-report.test.ts \
  tests/full-auto-liveness.test.ts \
  src/full-auto-run-liveness-control-server.test.ts \
  src/full-auto-run-analyzer.test.ts \
  src/full-auto-provider-handoff.test.ts \
  src/full-auto-run-handoff-control-server.test.ts \
  tests/full-auto-guardrails.test.ts
```

The 61st green criterion is FA-AC-66 (this AssuranceSpec is itself complete and
well formed). Run its oracle from the repository root (observed this pass: 1
file passed, 11 tests passed):

```sh
./apps/openagents-desktop/node_modules/.bin/vp test --run --root . \
  packages/assurance-spec/test/assurance-spec.test.ts
```

Confirm the AssuranceSpec structure and coverage directly:

```sh
node --import tsx packages/assurance-spec/src/cli.ts validate specs/desktop/full-auto.assurance-spec.md
node --import tsx packages/assurance-spec/src/cli.ts coverage specs/desktop/full-auto.assurance-spec.md
node --import tsx packages/assurance-spec/src/cli.ts ledgers  specs/desktop/full-auto.assurance-spec.md
```

Observed this pass: `validate` reports `ok`. `coverage` reports 76/76
obligations ready, 0 need design. `ledgers` reports 76/76 criteria bound to
obligations and `execution 0/76 obligations executed (receipt source: none)`,
because the AssuranceSpec document carries no Evidence Index that binds executed
receipts. This packet is the runnable evidence map the document does not embed.

## How to reproduce the smoke-gated oracles (FA-AC-07 and FA-AC-29)

These two criteria assert that an enabled Full Auto thread resumes on its own
across a real application quit and relaunch. Their oracle is the two-OS-process
smoke, not an in-process test. The reviewer runs (macOS, needs a prior build,
launches the real Electron application twice):

```sh
cd apps/openagents-desktop
node --import tsx scripts/build.ts
pnpm run smoke:full-auto-restart
```

Honest bound: the smoke conditionally falls back to unsigned dev-mode
`electron .` when no local Forge package exists at
`out/OpenAgents-darwin-arm64/OpenAgents.app`. It is therefore development-tier,
not a signed and notarized release artifact. The repository pre-push full gate
(`OPENAGENTS_PRE_PUSH_FULL_GATE=1`) runs this smoke. This packet pass did not
re-run it, so its status here is smoke-gated rather than executable-green. The
in-process portions of the restart and resume behavior (FA-AC-04, FA-AC-08,
FA-AC-13, FA-AC-14, FA-AC-15, FA-AC-17) run GREEN through
`tests/full-auto-restart.e2e.test.ts` in the command above.

## How to review the receipt-backed criteria (FA-AC-33, FA-AC-62 through FA-AC-65)

Evidence is the retained owner-real acceptance receipt at
`docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json` (schema
`openagents.desktop.full_auto_real_acceptance_receipt.v1`, source `3123d926a3`).
The receipt records six named rows, all with disposition PASS, plus one
same-pass provider rotation PASS, on an owner-real macOS arm64 build with real
Codex app-server and Claude Agent SDK lanes.

Honest bound: the receipt `identity.packagingMode` is `dev` and its
`profileClass` is `owner_real`. This is development-tier evidence. It is not a
signed-package proof and it is not independently admitted. The reviewer reads
the receipt rows directly:

```sh
python3 -c "import json; d=json.load(open('docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json')); [print(r['testId'], r['disposition'], r['title']) for r in d['results']]; print('rotation', d['automaticSamePassRotation']['from'], '->', d['automaticSamePassRotation']['to'])"
```

## Per-criterion evidence map

| Criterion | Risk cluster | Evidence path | Proof rung | Status |
| --- | --- | --- | --- | --- |
| FA-AC-01 | (supporting) | `apps/openagents-desktop/src/renderer/react-composer.test.tsx` | local_unit | executable-green |
| FA-AC-02 | (supporting) | `apps/openagents-desktop/src/codex-local-runtime.test.ts` | local_unit | executable-green |
| FA-AC-03 | (supporting) | `apps/openagents-desktop/src/renderer/shell.test.ts` | local_unit | executable-green |
| FA-AC-04 | (supporting) | `apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts` | local_dev_two_process_unsigned | executable-green |
| FA-AC-05 | (supporting) | `apps/openagents-desktop/src/renderer/shell.test.ts` | local_unit | executable-green |
| FA-AC-06 | exactly-once | `apps/openagents-desktop/tests/full-auto-registry.test.ts` | local_unit | executable-green |
| FA-AC-07 | packaged quit/relaunch resume | `apps/openagents-desktop/scripts/full-auto-restart-smoke.ts` | local_dev_two_process_unsigned | smoke-gated |
| FA-AC-08 | packaged quit/relaunch resume | `apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts` | local_unit | executable-green |
| FA-AC-09 | (supporting) | `apps/openagents-desktop/src/renderer/shell.test.ts` | local_dev_two_process_unsigned | executable-green |
| FA-AC-10 | (supporting) | `apps/openagents-desktop/src/full-auto-lane.test.ts` | local_unit | executable-green |
| FA-AC-11 | (supporting) | `apps/openagents-desktop/tests/full-auto-registry.test.ts` | local_unit | executable-green |
| FA-AC-12 | (supporting) | `apps/openagents-desktop/tests/full-auto-registry.test.ts` | local_unit | executable-green |
| FA-AC-13 | workspace/objective fail-closed | `apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts` | local_unit | executable-green |
| FA-AC-14 | workspace/objective fail-closed | `apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts` | local_unit | executable-green |
| FA-AC-15 | exactly-once | `apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts` | local_unit | executable-green |
| FA-AC-16 | exactly-once | `apps/openagents-desktop/src/full-auto-retry-rotation-model.test.ts` | local_unit | executable-green |
| FA-AC-17 | exactly-once | `apps/openagents-desktop/tests/full-auto-restart.e2e.test.ts` | local_unit | executable-green |
| FA-AC-18 | (supporting) | `apps/openagents-desktop/tests/full-auto-registry.test.ts` | local_unit | executable-green |
| FA-AC-19 | (supporting) | `apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx` | local_unit | executable-green |
| FA-AC-20 | (supporting) | `apps/openagents-desktop/src/renderer/shell.test.ts` | local_unit | executable-green |
| FA-AC-21 | (supporting) | `apps/openagents-desktop/src/renderer/react-composer.test.tsx` | local_unit | executable-green |
| FA-AC-22 | (supporting) | `apps/openagents-desktop/src/full-auto-control-server.test.ts` | local_unit | executable-green |
| FA-AC-23 | (supporting) | `apps/openagents-desktop/src/full-auto-control-server.test.ts` | local_unit | executable-green |
| FA-AC-24 | (supporting) | `apps/openagents-desktop/src/full-auto-control-server.test.ts` | local_unit | executable-green |
| FA-AC-25 | (supporting) | `apps/openagents-desktop/src/full-auto-control-server.test.ts` | local_unit | executable-green |
| FA-AC-26 | (supporting) | `apps/openagents-desktop/src/full-auto-control-server.test.ts` | local_unit | executable-green |
| FA-AC-27 | (supporting) | `apps/openagents-desktop/src/full-auto-control-server.test.ts` | local_unit | executable-green |
| FA-AC-28 | (supporting) | `apps/openagents-desktop/src/full-auto-control-server.test.ts` | local_unit | executable-green |
| FA-AC-29 | packaged quit/relaunch resume | `apps/openagents-desktop/scripts/full-auto-restart-smoke.ts` | local_dev_two_process_unsigned | smoke-gated |
| FA-AC-30 | (supporting) | `apps/openagents-desktop/src/provider-lane.test.ts` | local_unit | executable-green |
| FA-AC-31 | (supporting) | `apps/openagents-desktop/src/full-auto-lane.test.ts` | local_unit | executable-green |
| FA-AC-32 | (supporting) | `apps/openagents-desktop/src/full-auto-control-server.test.ts` | local_unit | executable-green |
| FA-AC-33 | real-provider dogfood | `docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json` | owner_real | receipt-backed |
| FA-AC-34 | (supporting) | `apps/openagents-desktop/src/spec-lane-workflow.test.ts` | local_unit | executable-green |
| FA-AC-35 | (supporting) | `apps/openagents-desktop/src/full-auto-lane.test.ts` | local_unit | executable-green |
| FA-AC-36 | (supporting) | `apps/openagents-desktop/src/spec-lane-workflow.test.ts` | local_unit | executable-green |
| FA-AC-37 | (supporting) | `apps/openagents-desktop/src/spec-lane-workflow.test.ts` | local_unit | executable-green |
| FA-AC-38 | workspace/objective fail-closed | `apps/openagents-desktop/tests/full-auto-run-registry.test.ts` | local_unit | executable-green |
| FA-AC-39 | exactly-once | `apps/openagents-desktop/tests/full-auto-run-registry.test.ts` | local_unit | executable-green |
| FA-AC-40 | exactly-once | `apps/openagents-desktop/tests/full-auto-run-registry.test.ts` | local_unit | executable-green |
| FA-AC-41 | workspace/objective fail-closed | `apps/openagents-desktop/tests/full-auto-run-registry.test.ts` | local_unit | executable-green |
| FA-AC-42 | workspace/objective fail-closed | `apps/openagents-desktop/tests/full-auto-thread-pressure.e2e.test.ts` | local_unit | executable-green |
| FA-AC-43 | pause/stop/cap terminal + fencing | `apps/openagents-desktop/tests/full-auto-run-registry.test.ts` | local_unit | executable-green |
| FA-AC-44 | pause/stop/cap terminal + fencing | `apps/openagents-desktop/src/full-auto-run-control-server.test.ts` | local_contract_http | executable-green |
| FA-AC-45 | pause/stop/cap terminal + fencing | `apps/openagents-desktop/src/full-auto-run-control-server.test.ts` | local_contract_http | executable-green |
| FA-AC-46 | report truth/redaction | `apps/openagents-desktop/src/full-auto-run-report.test.ts` | local_unit | executable-green |
| FA-AC-47 | liveness/stall recovery | `apps/openagents-desktop/tests/full-auto-liveness.test.ts` | local_unit | executable-green |
| FA-AC-48 | liveness/stall recovery | `apps/openagents-desktop/src/full-auto-run-liveness-control-server.test.ts` | local_contract_http | executable-green |
| FA-AC-49 | (supporting) | `apps/openagents-desktop/tests/full-auto-run-registry.test.ts` | local_unit | executable-green |
| FA-AC-50 | (supporting) | `apps/openagents-desktop/tests/full-auto-run-registry.test.ts` | local_unit | executable-green |
| FA-AC-51 | report truth/redaction | `apps/openagents-desktop/src/full-auto-run-report.test.ts` | local_unit | executable-green |
| FA-AC-52 | report truth/redaction | `apps/openagents-desktop/src/full-auto-run-report.test.ts` | local_unit | executable-green |
| FA-AC-53 | (supporting) | `apps/openagents-desktop/src/full-auto-run-analyzer.test.ts` | local_unit | executable-green |
| FA-AC-54 | (supporting) | `apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx` | local_unit | executable-green |
| FA-AC-55 | (supporting) | `apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx` | local_unit | executable-green |
| FA-AC-56 | (supporting) | `apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx` | local_unit | executable-green |
| FA-AC-57 | (supporting) | `apps/openagents-desktop/src/renderer/react-full-auto-surface.test.tsx` | local_unit | executable-green |
| FA-AC-58 | provider handoff disclosure | `apps/openagents-desktop/src/full-auto-provider-handoff.test.ts` | local_unit | executable-green |
| FA-AC-59 | pause/stop/cap terminal + fencing | `apps/openagents-desktop/src/full-auto-run-handoff-control-server.test.ts` | local_contract_http | executable-green |
| FA-AC-60 | provider handoff disclosure | `apps/openagents-desktop/src/full-auto-provider-handoff.test.ts` | local_unit | executable-green |
| FA-AC-61 | provider-support claim freshness | `apps/openagents-desktop/src/renderer/react-composer.test.tsx` | local_unit | executable-green |
| FA-AC-62 | real-provider dogfood | `docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json` | owner_real_development | receipt-backed |
| FA-AC-63 | real-provider dogfood | `docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json` | owner_real_development | receipt-backed |
| FA-AC-64 | real-provider dogfood | `docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json` | owner_real_development | receipt-backed |
| FA-AC-65 | real-provider dogfood | `docs/sol/evidence/2026-07-18-full-auto-real-owner-acceptance.json` | owner_real_development | receipt-backed |
| FA-AC-66 | (supporting) | `packages/assurance-spec/test/assurance-spec.test.ts` | local_unit | executable-green |
| FA-AC-67 | exactly-once | `apps/openagents-desktop/src/full-auto-retry-rotation-model.test.ts` | local_unit | executable-green |
| FA-AC-68 | guardrail non-override | `apps/openagents-desktop/tests/full-auto-guardrails.test.ts` | local_unit | executable-green |
| FA-AC-69 | MemoHarness (rev 14) | `apps/openagents-desktop/src/full-auto-harness-policy.test.ts` | local_contract_model | designed-only |
| FA-AC-70 | MemoHarness (rev 14) | `apps/openagents-desktop/src/full-auto-harness-policy.test.ts` | local_contract_model | designed-only |
| FA-AC-71 | MemoHarness (rev 14) | `apps/openagents-desktop/src/full-auto-harness-policy.test.ts` | local_contract_model | designed-only |
| FA-AC-72 | MemoHarness (rev 14) | `apps/openagents-desktop/src/full-auto-harness-policy.test.ts` | local_contract_model | designed-only |
| FA-AC-73 | MemoHarness (rev 14) | `apps/openagents-desktop/src/full-auto-harness-policy.test.ts` | local_contract_model | designed-only |
| FA-AC-74 | MemoHarness (rev 14) | `apps/openagents-desktop/src/full-auto-harness-policy.test.ts` | local_contract_model | designed-only |
| FA-AC-75 | MemoHarness (rev 14) | `apps/openagents-desktop/src/full-auto-harness-policy.test.ts` | local_contract_model | designed-only |
| FA-AC-76 | MemoHarness (rev 14) | `apps/openagents-desktop/src/full-auto-harness-policy.test.ts` | local_contract_model | designed-only |

## Gaps the reviewer must weigh before admission

1. **Independent admission is unmet.** `lifecycle_state` is `proposed`. No
   recorded review or admission event exists for this exact revision. Admission
   is an owner or independent-reviewer action, distinct from the producer.
2. **MemoHarness cluster is designed-only.** FA-AC-69 through FA-AC-76 have no
   executable oracle because the production seam
   `apps/openagents-desktop/src/full-auto-harness-policy.ts` and its named test
   do not exist yet. The design is present. The observation is absent.
3. **Packaged resume evidence is unsigned and development-tier.** FA-AC-07 and
   FA-AC-29 have real two-OS-process evidence, but no signed and notarized
   packaged-build resume proof was produced. The release-artifact rung stays
   BLOCKED.
4. **Owner-real dogfood evidence is development-tier and not admitted.** The
   `2026-07-18` receipt proves the six rows and same-pass rotation in an
   owner-real development profile. It is neither a signed-package proof nor
   independently admitted.
5. **No composed formal model exists.** The repository has no TLA+ model of the
   combined lifecycle, lease, retry, and provider-switch state space. The
   bounded exhaustive enumeration in `full-auto-retry-rotation-model.test.ts`
   and `full-auto-run-registry.test.ts` is real but narrower.
6. **The AssuranceSpec carries no Evidence Index.** The `ledgers` command
   reports `execution 0/76`, because the document binds designs, not executed
   receipts. This packet supplies the runnable evidence map. A future reviewed
   revision may embed an Evidence Index that binds these exact receipts.

## Non-admission statement

This packet maps 61 criteria to executable oracles that this pass observed
GREEN, 2 criteria to a gated two-OS-process smoke, 5 criteria to a retained
development-tier receipt, and 8 criteria to design only. The AssuranceSpec
stays `proposed`. No promise state changed. Issue #8978 stays OPEN. Admission
requires an independent reviewer who is distinct from the producer.
