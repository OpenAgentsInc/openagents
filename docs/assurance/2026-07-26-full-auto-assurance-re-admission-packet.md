# Full Auto AssuranceSpec revision 7 re-admission packet

Issue: <https://github.com/OpenAgentsInc/openagents/issues/9254>

Target: `specs/desktop/full-auto.assurance-spec.md`

## Purpose

The `omega-effectd` extraction moved six executable Full Auto oracle files.
The prior admitted revision named the old Desktop paths. The old admission
receipt binds the old target bytes. It cannot admit this revision.

Revision 7 changes only oracle artifact paths and the two renamed Claude test
paths in the repository inventory. It does not change a criterion, an
obligation, a proof rung, a gate, a risk, an evidence tier, or a release claim.
The revision is `proposed` until an independent reviewer completes the action
in this packet.

## Bound state

The target keeps the same ProductSpec revision and subject criterion set:

- ProductSpec: `specs/desktop/full-auto.product-spec.md`, revision 14.
- Criteria: `FA-AC-01` through `FA-AC-76`.
- Review tiers: 61 executable, 2 smoke-gated, 5 receipt-backed, and 8
  designed-only.

The moved executable oracle paths are:

- `packages/omega-effectd/src/engine/full-auto-control-server.test.ts`
- `packages/omega-effectd/src/engine/full-auto-lane.test.ts`
- `packages/omega-effectd/src/engine/full-auto-liveness.test.ts`
- `packages/omega-effectd/src/engine/full-auto-provider-handoff.test.ts`
- `packages/omega-effectd/src/engine/full-auto-run-report.test.ts`

The Desktop inventory now names `claude-local-runtime.test.ts` and
`claude-local-runtime-caps.test.ts`. These are the renamed files. They do not
arm a revision-7 executable obligation.

## Reproduction

From a clean checkout at the candidate commit, run the Desktop batch:

```sh
cd apps/openagents-desktop
./node_modules/.bin/vp test --run --root ../.. \
  src/codex-local-runtime.test.ts \
  src/full-auto-retry-rotation-model.test.ts \
  src/full-auto-run-analyzer.test.ts \
  src/full-auto-run-control-server.test.ts \
  src/full-auto-run-handoff-control-server.test.ts \
  src/full-auto-run-liveness-control-server.test.ts \
  src/provider-lane.test.ts \
  src/renderer/react-composer.test.tsx \
  src/renderer/react-full-auto-surface.test.tsx \
  src/renderer/shell.test.ts \
  src/spec-lane-workflow.test.ts \
  tests/full-auto-guardrails.test.ts \
  tests/full-auto-registry.test.ts \
  tests/full-auto-restart.e2e.test.ts \
  tests/full-auto-run-registry.test.ts \
  tests/full-auto-thread-pressure.e2e.test.ts
```

Observed in the re-admission preparation pass: 16 files passed, 437 tests
passed, and 11 tests skipped.

Run the engine and AssuranceSpec batch from the repository root:

```sh
./apps/openagents-desktop/node_modules/.bin/vp test --run --root . \
  packages/assurance-spec/test/assurance-spec.test.ts \
  packages/omega-effectd/src/engine/full-auto-control-server.test.ts \
  packages/omega-effectd/src/engine/full-auto-lane.test.ts \
  packages/omega-effectd/src/engine/full-auto-liveness.test.ts \
  packages/omega-effectd/src/engine/full-auto-provider-handoff.test.ts \
  packages/omega-effectd/src/engine/full-auto-run-report.test.ts
```

Observed in the re-admission preparation pass: 6 files passed and 77 tests
passed.

Then run:

```sh
node --import tsx packages/assurance-spec/src/cli.ts validate specs/desktop/full-auto.assurance-spec.md
node --import tsx packages/assurance-spec/src/cli.ts coverage specs/desktop/full-auto.assurance-spec.md
pnpm exec vp test --run packages/assurance-spec/test/review-admit.test.ts
```

The reviewer must reproduce these commands independently. Preparation output
is not verification evidence.

## Independent review action

The target authority allows an owner or an owner-designated independent
reviewer. The current target names Euler as the owner-designated reviewer.
The producer of this re-admission packet must not run this action.

After independent reproduction is green, the reviewer runs:

```sh
node --import tsx packages/assurance-spec/src/cli.ts review-admit \
  specs/desktop/full-auto.assurance-spec.md \
  --reviewer euler_owner_designated_independent_reviewer_2026_07_24 \
  --producer assurance_re_admission_packet_producer_2026_07_26 \
  --trigger issue.9254.omega_effectd_oracle_binding_re_admission \
  --program program.full_auto_release \
  --evidence docs/assurance/2026-07-26-full-auto-assurance-re-admission-packet.md
```

The command must write a new authority decision receipt under
`docs/assurance/receipts/`, bind its exact target digest in the target
frontmatter, and flip the target lifecycle from `proposed` to `admitted`.
The reviewer must record the command result and receipt reference on issue
#9254. If any batch fails, the reviewer must refuse admission and retain the
target as `proposed`.

## Non-claims

This packet does not execute the two-process smoke criteria. It does not
upgrade the five owner-real receipt-backed criteria. It does not make the eight
MemoHarness criteria executable. It does not authorize a release, a public
claim, or a promise transition.
