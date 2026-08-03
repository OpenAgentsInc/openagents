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
record whose seven mechanical gates all pass is eligible to authorize an
`independent_verification` product mode in a later integration. Thus adding the
verifier library does not silently upgrade existing discovery callers.

### What the verifier decides, and what it is told

Three boundaries exist because a producer must not be able to hand the verifier
a conclusion:

- **Evidence provenance.** A plan declares `conformance_vector` or
  `admitted_worker_run`. A conformance vector exercises the schemas and can
  never leave this library as `confirmed` or `independently_verified`, however
  well formed it is. Under `admitted_worker_run` every evidence receipt —
  mechanical included — must cite a worker receipt that the injected lifecycle
  authority resolves to an exact, still-admitted receipt bound to one of the
  plan's enumerated admitted workers, at that worker's exact resource
  generation and placement.
- **Derived control outcomes.** Control evidence reports how the control test
  process terminated, not what that means. `deriveControlTestOutcome` decides:
  a non-zero exit is a failure, a zero exit with the result artifact it should
  have produced is a success, and a zero exit with nothing kept is
  `not_observed` — never a pass.
- **A durable first verdict.** `commitInitialVerdict` is a required
  compare-and-set boundary. It returns whatever verdict is durably stored for
  the verification, and a candidate that disagrees with it refuses rather than
  relocking. Exactly-once is a property of that ledger, not of one call stack.

The plan enumerates `admittedWorkers` rather than pinning one sandbox, because
an honest vulnerable/fixed control pair runs in more than one isolated sandbox
and a single pinned ref cannot describe it.

### Live acceptance evidence

`test/verifier.test.ts` is a conformance vector and says so. The acceptance
evidence is `test/verifier-live.test.ts`, which replays
`fixtures/forensics/coldcard/loupe-verification-live-run.v1.json`: the pinned
vulnerable Coldcard MK4 tree built on an admitted `live_gce` sandbox, the
initial verdict written to
`fixtures/forensics/coldcard/loupe-first-verdict-ledger.v1.json`, and only then
the provider-provenance detector executing inside two further admitted
sandboxes — exit 1 on the vulnerable target, exit 0 on the fixed one.
Reproduce with `scripts/cloud/coldcard-loupe-verification-live.ts`, which is
owner-gated and default-off.

Two limits are worth stating plainly. The lifecycle authority and the verdict
ledger are still injected, so a caller that lies in both places is not caught
here; the provenance gate narrows what an honest caller may claim, it does not
authenticate the caller. And the detector answers a provenance question over
measured link and symbol evidence — it is not a statistical test of generator
output, which this contract never accepts as entropy-provenance proof.

Run the package checks with:

```sh
pnpm --filter @openagentsinc/forensic-loupe-adapter typecheck
pnpm --filter @openagentsinc/forensic-loupe-adapter test
```
