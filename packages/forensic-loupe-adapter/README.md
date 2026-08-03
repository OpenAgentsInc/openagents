# OpenAgents Loupe forensic adapter

`@openagentsinc/forensic-loupe-adapter` compiles structured forensic prompt
artifacts into bounded Loupe-style discovery plans and mediates a pluggable
Linux runtime. It does not provision a worker, mutate a checkout, report a
finding, contact a maintainer, or raise an admitted authority.

The adapter owns five boundaries:

- prompt artifacts are content-digested, recursively immutable, and retain
  parent lineage; their optional discovery-workflow profile explicitly owns
  candidate enumeration, severity order, prior-work search, root-cause
  identity, falsifiers, uncertainty disposition, duplicate continuation,
  finding scope, and conservative severity;
- execution plans inherit source coverage, budgets, network policy, worker
  digests, and tool availability from admitted contracts rather than prompt
  prose;
- only `submit_forensic_finding` and `submit_forensic_hypothesis` payloads can
  create typed output envelopes; diagnostic prose creates neither; and
- execution observes the immutable checkout digest before and after the
  backend call, emits bounded driver events, and keeps reporting
  `manual_no_reporting` with private findings; and
- every discovery plan, output binding, and execution result says
  `verificationMode: discovery_only`.

Incomplete coverage is allowed for explicitly degraded research runs, but it
remains bound into every output. Missing tools and dependency paths appear in
the compiled prompt and plan instead of being advertised as available.
Execution-plan v2 also binds the exact focal unit, tranche, bounded domain
direction, visible task bytes, and task digest. The domain field remains
analytic input and cannot expand target, tool, network, budget, reporting,
disclosure, checkout, or mutation authority.

## Independent verifier boundary

The `@openagentsinc/forensic-loupe-adapter/verifier` entry point replaces the
broken split completion path observed in the first Omega Loupe run. One adapter
transaction owns the initial verdict and final completion result; it does not
trust a child-process flush and then ask a second authority to infer whether the
verdict arrived.

The verifier rejects the discovery actor as its verifier, accepts only
source-reference, macro-value, and symbol-provider receipts before locking
exactly one deeply immutable initial verdict, and exposes PoC/control execution
only after that lock. A confirmation requires a successful PoC application on
an admitted worker, an observed failure on the immutable vulnerable target,
and an observed success on the immutable fixed target. Source, artifact, and
executed evidence remain separate. A prepared or merely applicable PoC cannot
claim execution.

`confirmed`, `dismissed`, and `inconclusive` remain distinct. Each verification
result still says `productMode: discovery_only`; only a separate release-gate
record whose six mechanical gates all pass is eligible to authorize an
`independent_verification` product mode in a later integration. Thus adding the
verifier library does not silently upgrade existing discovery callers.

Run the package checks with:

```sh
pnpm --filter @openagentsinc/forensic-loupe-adapter typecheck
pnpm --filter @openagentsinc/forensic-loupe-adapter test
```
