# Forensic prompt optimization governance

The first forensic prompt optimizer is a bounded offline compiler integrated
with Blueprint governance. It does not grant runtime authority and it does not
claim an optimizer implementation that is not present.

## Candidate identity

Each immutable candidate digest covers its parent prompt, Prompt IR, typed
finding and hypothesis schemas, tool policy, examples, parameters, every
dataset revision, optimizer configuration and kind, and the frozen metric,
T5, censoring, and eligibility definitions. Human curation, ablation, grids,
and retained-failure replay retain those honest names. DSPy or GEPA labels are
accepted only with an independently tested integration receipt.

## Evaluation boundary

Training and development examples are optimizer-visible. Vulnerable and clean
holdouts are evaluator-only, disjoint, and absent from candidate input. A
candidate is eligible for comparison only when:

- its baseline and candidate scorecards use the same untouched holdout,
  matched population, and frozen metric revision;
- all hard gates pass before a Pareto status is recorded;
- the candidate improves holdout discovery without a clean-holdout hit;
- generation and evaluation use distinct identities;
- at least two distinct fresh OpenAgents Cloud worker placements and source
  states are recorded; and
- independent mechanical evidence is attached.

Evaluation cannot rewrite the candidate's frozen metric definitions after
results are known.

## Promotion and rollback

Compiler output is an unpromoted Blueprint optimizer candidate. Promotion
requires an explicit passing Blueprint release gate and a human/operator
decision by an identity distinct from the generator. The release gate,
evaluation, and candidate digests must agree. Activation creates an append-only
transition with its prior active digest and rollback anchor. Rollback creates a
new transition rather than editing history.

The implementation lives in
`apps/openagents.com/workers/api/src/blueprint/services/forensic-prompt-compiler.ts`
with contracts in the adjacent `schemas/forensic-prompt-optimization.ts` file.
Its tests cover canonical identity, holdout isolation, honest optimizer naming,
fresh independent evaluation, self-promotion denial, activation, and rollback.
