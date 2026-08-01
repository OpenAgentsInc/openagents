# Omega Coldcard forensic practice-run runbook

Status: **private development and acceptance runbook for the checked-in
Coldcard benchmark.** It uses known historical material, synthetic fixtures,
and owner-authorized evidence. It is not a disclosure plan, a live-value search,
or evidence that an unknown vulnerability can be found.

Date: 2026-08-01

Read the system-level
[Omega forensic implementation and operator guide](../loupe/2026-08-01-omega-forensics-implementation-and-operator-guide.md)
first. This document narrows that system to the Coldcard practice sequence.

## 1. Purpose and success condition

The practice run teaches the system to find and explain the known Coldcard RNG
failure without learning a brittle string match. It must distinguish source,
artifact, generator, owned-fixture, public-chain fingerprint, entity, movement,
and identity claims.

The first qualified source result must explain all six causal links:

1. wallet seed creation consumes `ngu.random.bytes()` as secret entropy;
2. the board defines `MICROPY_HW_ENABLE_RNG` with value zero;
3. libNgU tests macro definedness rather than value and calls global
   `rng_get()`;
4. the vulnerable board object does not export the required provider;
5. MicroPython tests the value and compiles its Yasmarang fallback; and
6. that fallback can reach the wallet-seed sink, while final linked-artifact
   selection remains unproven at source tier.

Generic advice to improve randomness is not a hit. Matching only a macro name,
symbol, path, or published explanation is not a hit. A source-level hit is
still unverified until an independent verifier and later evidence rungs support
it.

## 2. Frozen inputs

The benchmark manifest is
[`fixtures/forensics/coldcard/benchmark-manifest.v1.json`](../../fixtures/forensics/coldcard/benchmark-manifest.v1.json).
The reproduction manifest is
[`fixtures/forensics/coldcard/reproduction-manifest.v1.json`](../../fixtures/forensics/coldcard/reproduction-manifest.v1.json).

Important pins:

| Role | Commit |
| --- | --- |
| Vulnerable Coldcard firmware | `bcc2c382a324690a2fcf972c0bac3b79bf923f7b` |
| Fixed Coldcard firmware | `ca72463709f4e3f8964952039d5caf955f566a87` |
| libNgU | `537519a829259622ea6b0334fbafd6cae852852f` |
| MicroPython | `4107246f8a080807b62c3b4838e71e812ea68b6f` |
| ckcc-protocol | `3d1dfa858beb58b8dac37d8c66d7aed2909812f2` |
| mpy-qr | `11347d83f4eb325b10676a4eb8e17deccfe0df44` |
| Independent postmortem | `47d8f5543812c8244fa95ed90db957ddcc05200c` |

Do not replace these with a branch name or the current default branch. Validate
the recorded tree and subordinate-file digests before interpreting a result.
The source manifest records `7abc9a4c680b5623fc8a64f70555dd2d3802e488`
as the Git **tree** for vulnerable firmware commit `bcc2c382…`; it is not a
second target commit. Keep commit identity and tree identity in their distinct
fields when preparing or reviewing a run.

The benchmark's development caps are one concurrent turn, 900 seconds, 200,000
tokens, and the exact cost bound in the manifest. Runtime authority can impose
lower caps; prompt prose cannot raise them.

## 3. Benchmark arms

Run all five arms as a set. A vulnerable-only run demonstrates memorization as
easily as capability.

Omega's first benchmark selector exposes **Vulnerable**, **Incomplete**,
**Fixed**, and **Clean**. The structural-variant population is part of the
checked-in benchmark and matched matrix, but it is not a fifth button in the
current native selector. Dispatch it through the benchmark/matrix evaluator
until Omega exposes that population directly.

| Arm | Purpose | Expected disposition |
| --- | --- | --- |
| Incomplete clone | Prove missing dependencies are detected before inference. | `completed_incomplete`, not scored, hypothesis tier, model call blocked. |
| Complete vulnerable | Find the known six-link source defect. | `source_hit_unverified`, source-observed, final artifact link not proven. |
| Fixed clone | Prove the historical defect is absent after the fix. | Historical finding absent. |
| Structural variants | Resist exact macro, symbol, syntax, dependency, and path matching. | Semantic variant hit with the same six-link causal structure. |
| Clean controls | Measure false positives and incomplete-evidence honesty. | No false positive, or `not_proven` for the intentionally unavailable dependency. |

The structural variants rename the macro and provider, move files, reverse the
guard shape, and move the fallback dependency. The clean controls include a
correct hardware provider, a deliberately unavailable provider dependency,
and a deterministic PRNG used only for non-secret UI behavior.

## 4. Pre-run checklist

Before a cloud run:

- [ ] The target commit and every required dependency pin match the manifest.
- [ ] The source materializer reports a complete canonical private bundle for
      every arm except the deliberately incomplete arm.
- [ ] The worker placement is `openagents_managed` on the admitted Google Cloud
      GCE profile, with the expected image and profile digests.
- [ ] Broker-only networking, one active turn, budget, lease, and scoped source
      and artifact capabilities are visible.
- [ ] The active prompt digest, finding schema, hypothesis schema, tool surface,
      model, effort, and scan profile are frozen.
- [ ] Discovery and verifier identities are distinct.
- [ ] Reporting is `manual_no_reporting`; there is no GitHub, email, or public
      reporting destination.
- [ ] Cleanup ownership is recorded before provisioning.
- [ ] The benchmark is labelled development data, not a holdout.

If any required source is absent, stop the qualified run. Preserve the
incomplete result and its spend; do not fill the missing evidence from the
postmortem or from model memory.

## 5. Practice sequence

### Stage A — validate contracts and fixtures

Run:

```sh
pnpm --filter @openagentsinc/forensic-contract test
pnpm --filter @openagentsinc/forensic-contract typecheck
pnpm --filter @openagentsinc/forensic-loupe-adapter test
pnpm --filter @openagentsinc/forensic-loupe-adapter typecheck
```

This validates schema strictness, canonical digests, benchmark splits, claim
gates, lifecycle laws, false-green fixtures, adapter boundaries, and verifier
ordering. It does not run firmware or contact a node.

### Stage B — prove the incomplete arm fails honestly

Select **Incomplete** in Omega. Required dependency paths include
`external/libngu`, `external/micropython`, `external/ckcc-protocol`, and
`external/mpy-qr`.

Expected result:

- coverage is visibly incomplete;
- the model call is blocked by the benchmark policy;
- the operator may acknowledge a research result, but it remains
  `completed_incomplete` and outside the qualified denominator; and
- no comprehensive source or artifact claim is possible.

A productive-looking scan with empty submodules is a test failure.

### Stage C — run complete vulnerable discovery

Select **Vulnerable**, confirm complete coverage, prepare the intent, and launch
one admitted worker. Record `T0` through `T5`, token and cost exactness, source
and prompt digests, all driver events, and cleanup ownership.

The qualified finding must contain all six ordered causal links and exact
citations to the seed sink, both board configurations where applicable,
libNgU's guard, the board provider surface, and MicroPython's fallback. It must
say that the final linked artifact remains not proven at source tier.

Anything else is a miss or hypothesis, even if the prose sounds correct.

### Stage D — verify independently

Hand the immutable source result to the distinct verifier. The verifier first
locks `confirmed`, `dismissed`, or `inconclusive`. Only then may it apply and
execute a PoC or control.

Confirmation requires:

- valid source-reference receipts;
- observed macro-value and symbol-provider receipts;
- a PoC or invariant check applied on the admitted worker;
- failure observed on the vulnerable target;
- success observed on the fixed target; and
- verifier identity distinct from discovery.

An applicable diff, a successful build, or a model's assertion is not an
executed verification receipt.

### Stage E — run fixed, structural, and clean controls

Run Fixed and Clean from the native selector. Run the structural-variant
population through the benchmark/matrix evaluator. Give every arm isolated
writable disk, provider session, auth home, environment, and hidden worker
state.

Reject a candidate that:

- reports the historical defect on the fixed target;
- misses a semantic structural variant;
- relies on an exact Coldcard token or path;
- reports the non-secret UI PRNG as a seed vulnerability; or
- treats the unavailable-dependency control as clean rather than `not_proven`.

### Stage F — witness code to artifact

Use the artifact-witness contracts and fixtures to inspect preprocessed macro
values, compiler inputs, linked symbol providers, sink reachability, retained
widths, truncation, and fail-closed fault builds.

This stage can raise the source claim to artifact reality only when the exact
firmware artifact and toolchain receipts are present. Missing build artifacts
stay `not_proven`. Statistical output tests are not entropy-provenance evidence.

### Stage G — reproduce the generator on an owned fixture

Use the independent uint32 Yasmarang implementation and frozen mutation vectors
to exercise fallback state, provider combination, 32-bit reseed truncation,
uniform retry, keypad shuffle traces, entropy assumptions, and work-factor
calculations.

The fixture must be synthetic or explicitly owner-authorized. Persist only
digests of public material. Return no mnemonic, xprv, seed phrase, or live-value
oracle result.

The expected claim is `owned_fixture_recovered`, not third-party wallet access.

### Stage H — replay the historical program fingerprint privately

First validate the synthetic historical bundle and its positive and negative
controls. For an admitted private node run, use the independent tools in
[`tools/coldcard/`](../../tools/coldcard/) against a full, unpruned Bitcoin Core
node.

```sh
cd tools/coldcard
CC_SELFTEST_ONLY=1 python3 cc-fingerprint-scan.py 960367 960367 selftest
python3 cc-fingerprint-scan.py 960180 960599 incident
python3 cc-fingerprint-stratify.py
```

The scanner is append-only and resumable. It fails on missing prevout or fee
data, records completed-height checkpoints, and uses exact satoshi arithmetic.
The cheap fingerprint alone has a high base rate and establishes only program
similarity. Keep transaction identifiers private in Omega; display only bounded
counts and the non-reportable boundary.

See [Our Bitcoin node as forensic capability](2026-08-01-bitcoin-node-forensic-capability.md)
for measured base rates and the limits of public-chain inference.

### Stage I — derive the evidence graph and reconcile claims

Build graph nodes only from typed victim-report, published-address, or
program-fingerprint seeds. Co-generate every edge and explanation from the same
versioned rule. Preserve every source when nodes converge.

Report transaction, address, UTXO, and report counts separately. Victim count,
unknown collectors, and unpooled operators stay unavailable unless independent
evidence supports them. Reconcile independently derived and published figures
as `MATCH`, `DRIFT`, or `UNAVAILABLE`, retaining both originals and their
precision bounds.

Promotions and corrections append digest-linked claim events. Never rewrite an
earlier finding or replace a derived value with a published one.

### Stage J — compare and clean up

Build a matched run matrix across prompt, model, and scan-profile candidates.
Check quality and safety gates before time or token improvement. Then delete
every worker and require zero-residue cleanup.

Do not accept a benchmark run with an uncleaned worker, an unrevoked capability,
a gapped event stream, an unavailable usage value rendered as zero, or a
recovery-required resource left without an owner.

## 6. Claim ladder and stopping rules

Omega displays nine rungs. Each rung has its own evidence gate and explicit
non-implication.

| Rung | Required evidence | It does not prove |
| --- | --- | --- |
| Source flaw | Exact source references and six-link causal path. | Which provider the shipped firmware linked. |
| Artifact reality | Preprocessed source, compiler inputs, symbol provider, firmware digest, and fault build. | Practical wallet recovery. |
| Generator behavior | Generator vectors, exact call trace, and verifier receipts. | The complete real-device state space. |
| Exploitability | Explicit assumption model, entropy interval, sensitivity, and work factor. | That any third-party wallet is recoverable. |
| Owned-fixture recovery | Authorization, generator trace, and public-material match. | Permission to enumerate live value. |
| Program fingerprint | Immutable chain snapshot, controls, and measured base rate. | Vendor, person, victim, intent, or theft. |
| Entity grouping | Evidence graph and versioned pooling edges. | A natural-person identity. |
| Unauthorized movement | Victim testimony and exact transaction evidence. | The actor's identity. |
| Identity attribution | Independent identity evidence and review. | Anything beyond the stated evidence and scope. |

Stop at the highest supported rung. A missing rung remains visible and cannot
be inferred from a downstream pattern.

## 7. Metrics and acceptance

Record these milestones for every eligible run:

| Milestone | Meaning |
| --- | --- |
| `T0` | Admission accepted. |
| `T1` | Exact worker ready. |
| `T2` | Source bundle and terminal coverage ready. |
| `T3` | Model work started. |
| `T4` | First strict typed candidate accepted. |
| `T5` | First rubric-qualified six-link identification. |
| `T6` | Independent verification locked and completed. |
| `T7` | Deletion and zero residue observed. |

Primary improvement measures are time and tokens to `T5`, with identification
rate, causal-link coverage, reference validity, false positives, clean-control
rate, verification rate, reviewer seconds, cost, cancellation latency, cleanup
latency, and zero-residue rate as hard companions.

For every candidate report:

- hit, miss, sample, and censor counts;
- p50 identification time and tail status;
- every identification-time and token observation;
- total tokens and cost with exactness;
- all six causal links and evidence tiers;
- false positives and clean-control outcomes;
- active reviewer time;
- contributing run, event, receipt, source, prompt, worker, and fixture refs;
  and
- cleanup count and residual state.

Small samples are provisional. An eligible miss keeps its spent tokens and
right-censor boundary. A faster candidate cannot win if it weakens any evidence
or safety gate.

## 8. How to inspect the result in Omega

Read the workbench in this order:

1. **Target and managed worker:** confirm exact repository, commit, coverage,
   GCE placement, image/profile digests, budget, and network posture.
2. **Prompt artifact:** record the active digest and candidate lineage.
3. **Lifecycle and metrics:** confirm ordered events, exactness, censoring, and
   cleanup state.
4. **Finding or hypothesis:** require the right typed lane and all source refs.
5. **Evidence ladder:** find the first missing or provisional rung and stop the
   claim there.
6. **Source → artifact → generator trace:** verify that these are separate
   derivations rather than one model narrative.
7. **Entropy sensitivity:** inspect every assumption and resulting bounded
   interval.
8. **Historical scan:** check boundary, range, restart state, controls,
   throughput exactness, candidate funnel, and missing-data failures.
9. **Provenance and reconciliation:** inspect missing refs, published bounds,
   `MATCH`/`DRIFT`/`UNAVAILABLE`, and append-only corrections.
10. **Run matrix:** compare qualified populations only after every hard gate.

The first Omega renderer places these sections in one long dock. Exact refs can
be visually truncated; use the retained projection or source-opening action for
the full value. Keep the private boundary, run state, and cleanup status in view
manually until the planned section navigation and sticky status bar exist.

## 9. Failure interpretation

| Observation | Interpretation |
| --- | --- |
| Incomplete arm reports complete coverage | Materializer or projection defect; invalidate the run. |
| Complete vulnerable arm misses | Detection miss; retain spent usage and censor boundary. |
| Vulnerable arm finds only one file or link | Hypothesis or partial source observation, not a qualified hit. |
| Fixed or clean control hits | False positive; block candidate promotion. |
| Structural variant misses | Likely exact-match overfitting; block promotion. |
| Verifier reuses discovery identity | Circular verification; result is not independently verified. |
| PoC applies but was not executed | Prepared evidence only. |
| Artifact or provider receipt is absent | Stop below artifact reality. |
| Node scan lacks a control or prevout | Incomplete scan; do not interpret a zero or rate. |
| Reconciliation drifts | Preserve both values and investigate; do not overwrite either. |
| Usage is unavailable | Keep it unavailable; never report zero. |
| Cleanup cannot be proved | `recovery_required` or cleanup failure; run is not successfully complete. |

## 10. Run record

Retain one private run record containing:

- benchmark, fixture, source bundle, tree, prompt, model, tool, worker image,
  profile, metric registry, and evaluator digests;
- every lifecycle event and canonical milestone;
- typed findings, hypotheses, verifier results, PoC/control receipts, metrics,
  decisions, and corrections;
- matrix population and split identity;
- evidence-rung status, assumptions, non-implications, and provenance health;
- historical-scan checkpoints, control results, base-rate strata, and only the
  private refs permitted by the boundary; and
- deletion, capability revocation, and zero-residue receipts.

The run record remains private until a separate disclosure scope, reviewer,
maintainer channel, and publication authority are established.
