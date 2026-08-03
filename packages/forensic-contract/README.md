# OpenAgents forensic contracts

`@openagentsinc/forensic-contract` is the canonical Effect Schema boundary for
the Omega forensic-analysis program. It contains data contracts only. It does
not admit a worker, authorize a scan, execute a target, contact a maintainer,
or publish a claim.

The package owns:

- target, source-bundle, coverage, profile, worker-placement, prompt, run,
  event, finding, hypothesis, receipt, metric-registry, provider-usage,
  evaluator-adjudication, scorecard, and promotion shapes;
- the Coldcard reproduction, generator, entropy, historical-chain,
  transaction-fingerprint, node-scan, evidence-graph, and claim-revision
  shapes;
- the C/C++ artifact-witness capture, assertion, result, and report shapes,
  plus a deterministic evaluator for preprocessed macros, compiler inputs,
  linked symbol providers, secret-sink reachability, retained widths,
  truncation, and fail-closed fault builds;
- an independent uint32 Yasmarang transition, libngu/provider combiner,
  32-bit reseed truncation, target-compatible uniform retry, keypad shuffle
  trace, frozen mutation-vector evaluator, explicit work-factor calculator,
  and synthetic/owner-authorized public-material reproduction receipt;
- immutable historical block bundles, a broker-bound private read-only Bitcoin
  Core capability, exact-integer two-phase fingerprint scanning, self-test and
  negative-control gates, append-only checkpoints, deterministic resume, and
  fee/era/script/revision base-rate projections;
- the five-arm Coldcard benchmark, four suite-manifest, split-isolation, and
  honest historical-import shapes, with the checked-in development pack at
  `fixtures/forensics/coldcard/` in the repository;
- strict boundary decoding with excess-property rejection;
- deterministic canonical JSON and SHA-256 contract digests;
- prompt artifact digests that bind structured content, parent lineage, and
  the optional explicit discovery workflow used by current task compilers;
- fail-closed run-transition, event-sequence, claim-rung, and prior-revision
  laws; and
- a content-digested frozen catalog spanning lifecycle, detection, evidence,
  token, cost, reliability, reviewer-load, and Coldcard reproduction metrics;
- a deterministic scorecard projector that derives T5 from immutable finding
  event bytes plus later frozen adjudication, retains miss/censor truth and
  spent usage, and isolates every split/population; and
- bounded public run and scorecard projections containing only digests,
  aggregate counts, durations, usage/cost totals, exactness, completeness, and
  cleanup truth.

## Contract laws

The implementation deliberately keeps these facts separate:

- incomplete source coverage cannot become a complete result;
- a completed result requires observed zero-residue cleanup;
- source evidence cannot qualify an artifact, fingerprint, theft, or identity
  claim;
- claim revisions append evidence and bind the exact prior claim digest;
- a successful historical scan requires positive and negative controls and no
  missing required data;
- prompt candidates cannot evaluate or promote themselves;
- faster prompt candidates cannot win with a hard-gate or quality regression;
- provider usage and cost retain exactness, and unavailable values contain no
  numeric fields that could be mistaken for zero;
- eligible misses retain their spent usage and a nonzero right-censor boundary;
- fixed, clean, incomplete, development, and holdout populations cannot pool;
- evaluator timing cannot replace the original content-digested finding time;
- scorecards rebuild event and receipt digests from retained evidence; and
- postmortem comparison outputs cannot become derivation or evaluator inputs,
  and Coldcard development arms cannot enter evaluator-only holdouts.
- missing build artifacts become `not_proven`, complete symbol inventories are
  required to prove fallback absence, and statistical output tests are never
  entropy-provenance evidence.
- owned-fixture reproduction never performs a live-value lookup, persists only
  digests of xpub/address material, returns no mnemonic or xprv, and requires an
  explicit synthetic or owner-authorization reference.
- historical scanning fails on missing fee or required prevout data, resolves
  exactly the cheap candidate set, and caps every result at program similarity;
  it never grants identity, intent, theft, victim, or live-wallet claims.
- evidence graphs begin with typed report, publication, and fingerprint seeds;
  preserve every source when nodes converge; co-generate edges and explanations
  from one versioned rule; and report victim totals, unreachable collectors,
  and unpooled operators as unavailable rather than deriving zeros or people
  counts from addresses, transactions, UTXOs, or reports.
- published-figure reconciliation retains both figures and explicit precision
  bounds as `MATCH`, `DRIFT`, or `UNAVAILABLE`, while promotions and corrections
  append digest-linked claim-history events instead of rewriting provenance.

Consumers must decode untrusted input with `strictDecode` or equivalent Effect
Schema options that set `onExcessProperty: "error"`. The schemas do not make a
runtime available and do not raise an evidence tier by themselves.

## Conformance fixtures

The complete v1 corpus is checked in at:

- `fixtures/forensics/positive.v1.json`
- `fixtures/forensics/negative.v1.json`

Every schema registered by `FORENSIC_CONTRACT_SCHEMAS` has one positive and one
negative fixture. The tests also cover semantic false-green cases, including
invalid lifecycle transitions, incomplete coverage, cross-rung claim evidence,
non-dense chain inputs, missing scan controls, optimizer self-promotion,
scorecard count/population drift, adjudication timestamp rewriting, zero-valued
unavailable usage, missing censor boundaries, quality-dominated promotion,
secret aggregate projection, and unknown fields.

Run the package checks with:

```sh
pnpm --filter @openagentsinc/forensic-contract test
pnpm --filter @openagentsinc/forensic-contract typecheck
```
