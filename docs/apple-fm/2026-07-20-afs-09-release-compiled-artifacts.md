# AFS-09 release of compiled answer and route artifacts

Date: 2026-07-20

Status: implementation record for work packet AFS-09. This record is evidence.
It is not release authority. It is not a product promise.

Audience: human and agent.

Authority: the plan
`docs/sol/2026-07-20-apple-fm-router-to-full-agent-system-plan.md` owns the
names, the bounds, and the rules. This record states what AFS-09 delivered. The
`@openagentsinc/dse` package (AFS-08) owns the compile, evaluate, artifact, and
promotion contracts.

## 1. What AFS-09 delivered

AFS-09 replaces the hand-written Apple FM prompts one signature at a time with
released DSE artifacts. AFS-09 changes no live behavior. Every release opens in
shadow mode, so the host serves the hand-written baseline until an explicit
promotion.

AFS-09 delivered these items:

1. Two compiled signatures: `AppleFm/HonestChatReply.v1` for the honest answer
   and `AppleFm/TurnRoute.v1` for the route recommendation.
2. A two-sided route metric that penalizes both a false delegation and a false
   refusal.
3. Production-shaped local fixtures with disjoint train, validation, and
   holdout splits.
4. An offline compile that emits immutable artifacts, evaluation reports, an
   independent review, a released pointer, and an uncertainty record.
5. A gated-activation release channel with shadow, canary, active, and
   rolled-back modes.
6. The compiled-artifact path behind the release gate in the Apple FM provider,
   with the hand-written prompt kept as the shadow baseline and the rollback
   target.

## 2. Signatures and metric

`AppleFm/TurnRoute.v1` is the compiled successor to the hand-written route
prose. Its output names the decision, the recommended candidate, the preserved
task summary, and any claimed action. The route metric scores every dimension
the plan requires by name:

- correct local answer.
- correct provider recommendation.
- needless provider recommendation (a false delegation).
- false local answer for provider work (a false refusal).
- recommendation of an unavailable or disallowed provider.
- unsafe or false action claim.
- task-summary preservation.
- data-destination and cost-policy compliance.
- latency, memory, thermal, and cancellation behavior.

Correctness has precedence over resource savings. The reward bundle lets a
resource cost only discount the score. Provider-token savings cannot buy back a
wrong or unsafe route.

## 3. Compile and evidence

The offline compile lives in `apps/openagents-desktop/src/turn/dse/`. A
deterministic proxy model fills the DSE model port. The proxy reproduces the two
hand-observed on-device failures: a false action claim without the compiled
honesty instruction, and a refusal spiral without the compiled routing
instruction. The compiler selects the compiled instruction on validation and
scores it on holdout. The holdout labels stay inaccessible to the search.

The checked-in bytes live in
`apps/openagents-desktop/src/turn/dse/artifacts.generated.ts`. The generator is
`apps/openagents-desktop/scripts/compile-dse-artifacts.ts`. A repeated compile
reproduces the same digests.

Measured holdout results:

| Signature | Baseline holdout | Candidate holdout | Delta |
| --- | --- | --- | --- |
| `AppleFm/HonestChatReply.v1` | 0.190 | 0.950 | 0.760 |
| `AppleFm/TurnRoute.v1` | 0.570 | 0.784 | 0.214 |

Each artifact beats its frozen baseline on validation and holdout. The fixtures
are small, so an uncertainty record accompanies each artifact and records a
small-sample note instead of a confidence interval. A larger holdout is required
before a strong claim.

Promotion uses the independent-evaluator role. A reviewer identity distinct from
the producer admits each candidate. The producer cannot admit its own
obligation.

## 4. Gated activation

The release channel lives in `packages/dse/src/contract/activation.ts` and
`packages/dse/src/runtime/activation.ts`. A channel resolves which artifact a
request serves:

- shadow: the hand-written baseline, for every request. No dispatch and no
  user-visible substitution.
- canary: the released artifact for a bounded, deterministic, sticky population.
  The canary plan names the population fraction, the maximum duration, the
  error-rate abort threshold, and the regression abort rule.
- active: the released artifact for every request.
- rolled-back: the hand-written baseline again.

A rollback restores the previous released artifact without an application
rebuild. When no previous release exists, the rollback restores the hand-written
baseline. The channel state is portable data. The Apple FM provider enacts the
decision. The checked-in default channel is shadow, so the compiled path changes
no live behavior until an explicit promotion.

## 5. Verification

- `pnpm --dir packages/dse typecheck` and the DSE unit tests pass.
- `pnpm --dir apps/openagents-desktop typecheck` passes.
- `pnpm --dir apps/openagents-desktop check:ide-boundaries` passes.
- `pnpm --dir apps/openagents-desktop check:afs-boundaries` passes.
- The AFS-09 desktop tests
  (`src/turn/dse/compile.test.ts`, `route-metric.test.ts`,
  `activation-gate.test.ts`) pass.

## 6. Boundaries

The DSE package imports no Apple FM adapter, no Desktop, no provider SDK, and no
Node host. The route metric, the fixtures, the proxy model, and the compile
script live in the Desktop app, not in the DSE package. The runtime resolves the
checked-in bytes offline and never links the compiler.
