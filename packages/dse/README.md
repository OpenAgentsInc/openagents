# @openagentsinc/dse

The Effect v4 DSE successor (AFS-08). DSE is the offline compile side for Apple
FM prompts and policies. It turns a hand-written prompt into a typed, evaluated,
versioned, and immutable artifact.

DSE is offline and portable. It imports no Apple FM adapter, no Desktop, no
Pylon, no Blueprint, no provider SDK, no cloud client, and no Node host. Its
output is a checked-in compiled artifact and a release record. A runtime
resolves that artifact offline from its bytes.

This package is the compile side only. It does not dispatch a provider, open a
network connection, or activate an artifact at runtime. The live Apple FM path
is a later packet (AFS-09).

## Subpath exports

- `@openagentsinc/dse/contract` — portable Effect schemas: references,
  signatures, Prompt IR, datasets and splits, metrics and evaluation reports,
  budgets and search plans, candidate artifacts, released pointers, rollback and
  predict receipts, promotion contracts, and the generated signature catalog.
- `@openagentsinc/dse/runtime` — resolve and verify a released artifact offline,
  and run `Predict` through the injected model port. This subpath has no compile
  or promotion authority.
- `@openagentsinc/dse/optimizer` — the offline compiler, evaluator, and search.
  A runtime application must not import this subpath.
- `@openagentsinc/dse` (root) — the contract plus the runtime. The root export
  does not include the optimizer.

## Module surface

- Signatures. `DseSignature<I, O>` binds an Effect Schema input and output, a
  default Prompt IR, and a serializable contract export. `makeSignature`
  derives the contract deterministically.
- Predict. `predict` renders the compiled program, calls the model, decodes the
  result with the output schema, runs one bounded repair on a first decode
  failure, and writes an append-only predict receipt.
- Datasets. `makeDatasetRevision` builds an immutable, content-addressed
  revision. `buildDatasetSplit` fails closed: a missing holdout fails, and train
  can never become holdout by omission.
- Metrics. A `Metric` scores one example. `rewardBundle` combines quality and
  resource components. Correctness has precedence over resource savings.
- Optimizers. `generateCandidates` produces a deterministic, deduplicated,
  capped candidate set for the instruction grid, greedy few-shot pool, joint
  search, and knob grids. The default cap is 128. MIPRO, GEPA, COPRO, Pareto,
  and a generic module graph are out of scope.
- Compile. `compileSignature` scores every candidate on validation, selects the
  winner, and scores only the winner on holdout. It emits an immutable
  candidate artifact and the two reports. Holdout labels stay inaccessible to
  the search.
- Artifacts. A `CandidateArtifact` is content-addressed by a digest over all of
  its bytes. A `ReleasedArtifactPointer` binds the frozen `ReleasedArtifact`
  shape from `@openagentsinc/agent-runtime-schema`.
- Promotion. `promote` admits a candidate only when an independent reviewer
  (distinct from the producer) admits it and the holdout delta clears the floor.
  The producer can never admit its own obligation.
- Rollback. `rollback` restores a prior released artifact and records a receipt.
- Resolution. `resolveReleasedArtifact` verifies a released artifact offline. A
  missing, altered, unreviewed, or incompatible artifact fails closed.

## Generated signature catalog

The signature catalog is derived from the signature registry, never written by
hand. `deriveSignatureCatalog` produces it, `SIGNATURE_CATALOG` is the
checked-in value, and `catalogClaimHolds` proves the two agree. A drift between a
signature contract and the catalog fails the claim. The first admitted signature
is `AppleFm/HonestChatReply.v1`.

## Verification

- `pnpm --dir packages/dse run typecheck`
- `pnpm --dir packages/dse run test`
- `node --import tsx scripts/check-afs-boundaries.ts` (from the repository root)
