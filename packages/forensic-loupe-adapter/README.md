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

### The verifier drives its own executor

The verifier does not accept a backend. Until 2026-08-03 it took five injected
functions — collect the mechanical evidence, submit the verdict, apply the PoC
and run the controls, resolve worker receipts, commit the first verdict — so
one caller supplied both the evidence and the authority that validated it. A
caller who lied consistently in both places was not caught. That was the last
open acceptance-audit blocker on OFR-007.

`runLoupeVerification` takes three things instead:

- a **`LoupeVerificationSpec`**: which finding, which two immutable revisions,
  which admitted worker profile, which budget. An intent, containing no
  evidence, no receipt, no verdict and no authority;
- a **`LoupeControlPlaneTransport`**: one authenticated managed-sandbox control
  plane, reached through two named routes;
- a **ledger directory** the adapter owns.

Everything else the verifier measures. It creates each sandbox, installs the
source bundle, runs the build, reads the capture, writes its own detector into
the guest, and observes how that detector terminated. It writes every evidence
receipt itself, and it resolves every admitted-worker receipt from the runtime
receipts its own calls returned. `evaluateLoupeVerificationSession` accepts only
a session carrying a brand that `runLoupeVerification` stamps, and that brand is
not reachable from any package export path.

### What the verifier decides, and what it is told

- **Evidence provenance is derived from the transport, never declared.** A
  `conformance` transport is a simulation and yields `conformance_vector`, which
  can never leave this library `confirmed` or `independently_verified` however
  agreeable the simulated sandbox is. A `live` or `recorded` transport yields
  `admitted_worker_run`, and then every evidence receipt — mechanical included —
  must cite a worker receipt that resolves to an exact, still-admitted receipt
  bound to the exact worker the evidence named, at that worker's exact resource
  generation and placement. A plan that claims a provenance its session did not
  observe is refused.
- **Derived control outcomes.** Control evidence reports how the control test
  process terminated, not what that means. `deriveControlTestOutcome` decides:
  a non-zero exit is a failure, a zero exit with the result artifact it should
  have produced is a success, and a zero exit with nothing kept is
  `not_observed` — never a pass.
- **A durable first verdict.** `durableFirstVerdictLedger` is the adapter's own
  compare-and-set over `open(path, "wx")`. `O_CREAT | O_EXCL` is resolved
  atomically by the kernel against every other opener of that path, in this
  process or any other, so a second writer never wins, never partially
  overwrites and never reads a torn record. `commit` returns what it re-read
  from disk on both paths, and a candidate that disagrees with the stored
  verdict refuses rather than relocking. Exactly-once is no longer a property of
  something the caller handed in.
- **What the control plane cannot get away with.** A create that admitted a
  different image or profile than requested, a create that never reported
  readiness, a delete that never reported cleanup, a receipt about a sandbox
  this run did not create, an image digest that changed part way through, a
  cost over budget, and a guest capture of the wrong commit are each refused.

The plan enumerates `admittedWorkers` rather than pinning one sandbox, because
an honest vulnerable/fixed control pair runs in more than one isolated sandbox
and a single pinned ref cannot describe it. `fixedTargetDigest` appears only
once a fixed control has been built, because a verification whose mechanical
tier did not reproduce the finding has no measured fixed target to name.

### Record and replay

`recordingLoupeControlPlane` wraps a live transport and records every exchange
and every clock reading in order. `recordedLoupeControlPlane` replays that
transcript, and enforces both order and request identity: a driver that would
have asked a different question, or the same questions in a different order, is
refused rather than handed the old answer. Because the driver reads every
timestamp from the transport, a replay reproduces its run to the millisecond,
including the verdict it locks.

A transcript is a measurement of one specific run, held to the same standard as
any other checked-in capture. It is not a way to manufacture provenance: it
carries the live origin it was recorded from, and the result records that origin
in `evidenceOriginRef`.

### Live acceptance evidence

`test/verifier.test.ts` runs the real driver against a simulated control plane.
It is a conformance vector and is capped as one. The acceptance evidence is
`test/verifier-live.test.ts`, which replays
`fixtures/forensics/coldcard/loupe-control-plane-transcript.v1.json.gz` — the
recorded wire responses of three admitted `live_gce` sandboxes that built the
pinned vulnerable Coldcard MK4 tree, locked a verdict, and then ran the
provider-provenance detector against the vulnerable and the fixed revisions.
Reproduce with `scripts/cloud/coldcard-loupe-verification-live.ts`, which is
owner-gated and default-off.

Two limits are worth stating plainly. A transport is still an object, so a test
can construct one; what a caller can no longer do is supply the conclusion, the
evidence, or the authority, and a simulated origin is capped at
`conformance_vector` by the transport rather than by anyone's good intentions.
And the detector answers a provenance question over measured link and symbol
evidence — it is not a statistical test of generator output, which this contract
never accepts as entropy-provenance proof.

Run the package checks with:

```sh
pnpm --filter @openagentsinc/forensic-loupe-adapter typecheck
pnpm --filter @openagentsinc/forensic-loupe-adapter test
```
