# 44. Cases and graders

> **Status: partly implemented.** The `#authority` reach-and-restraint promotion
> gate ([`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax),
> `#13896`, `#14059`) and positive/held-out case harness
> ([`coax/lib/agent/desktop/capability-author.coax`](../../coax/lib/agent/desktop/capability-author.coax))
> run on `cpu`, grounded by [`openagents/examples/capability-cases-gate.coax`](../examples/capability-cases-gate.coax)
> and [`openagents/examples/capability-cases-gate.out`](../examples/capability-cases-gate.out),
> and are consumed by [`coax/lib/agent/desktop/capability-eval.coax`](../../coax/lib/agent/desktop/capability-eval.coax)
> and [`coder/tui/oa-worker.coax`](../../coder/tui/oa-worker.coax). Remaining:
> signed standalone NIP-EVAL suite packaging, typed-decision graders, and
> cross-case state-leakage linting in `openagents/eval/suite.coax`.

## Premise

A claim needs a measurement, and a measurement needs tests. This chapter
builds the test set: cases where a member should act, cases where it should
not, and graders that decide each one.

## Draft

A test set for a member is a list of cases. Each case has an input, an
environment to run in, and a grader that says pass or fail.

Cases come in two kinds. In a should-fire case, the member ought to act and
the work ought to come out better. In a should-not-fire case, the member ought
to stay out of the way. The first kind measures reach. The second measures
restraint. A member with reach and no restraint acts everywhere and breaks
work it should have left alone, so every test set has both kinds.

Graders are deterministic where they can be: a test that runs, a file that
must exist, a value that must match. Where judgment is needed, a typed
decision makes it, with the question pinned by digest. A grader does not ask a
chat model for its opinion in prose.

The old tree kept a list of ways tests go wrong, written as knowledge notes.
The tests come from the same belief as the code. The checks are looser than
the grader. The checks pass only on the given example. Leftover state makes a
check pass. Each of these is a way to get a green result that means nothing,
and the test-set tooling checks for the ones it can.

A test set is a contribution in its own right. It is packaged and signed like
a plugin, it has an author, and it earns credit when others use it.

The project has written tests first since [Episode
6](../../docs/transcripts/006.md). [Episode
252](../../docs/transcripts/252.md) separates the design of a proof from the
run that produces it.

## What we build

A plan for what this chapter ships once the draft is approved:

- A test-set format: cases marked should-fire or should-not-fire, each with an
  environment and a grader.
- Graders: deterministic checks first, typed decisions where judgment is
  needed.
- A linter for the known ways a test set can fool its author.

It builds on what exists today:

- [`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax) (`#authority` reach/restraint held-out case gate, cost/latency budgets, and slug hygiene, `#13896`, `#14059`)
- [`coax/lib/agent/desktop/capability-author.coax`](../../coax/lib/agent/desktop/capability-author.coax) (positive + held-out two-arm shell case runner)
- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/systemone`](../../coax/lib/systemone)
- [`coder/tui/filter_eval.py`](../../coder/tui/filter_eval.py)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Test set, case and grader | `openagents.eval.v1` `Suite`, `Case`, `Grader` ([spec 21.1](../../docs/protocol/openagents-protocol-spec.md#211-openagentsevalv1)) | Nostr kind 3189 (NIP-EVAL) | Spec sketch; no `.proto` file |

### The chapter's check

- **Source:** [`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax),
  [`openagents/examples/capability-cases-gate.coax`](../examples/capability-cases-gate.coax),
  and [`openagents/examples/capability-cases-gate.out`](../examples/capability-cases-gate.out)
  (remaining: standalone signed suite format and state-leakage linter in
  `openagents/eval/suite.coax`).
- **Trace:** evaluates a 6-case fixture (`4` should-fire positive cases + `2`
  should-not-fire held-out restraint cases) where baseline passes `2/6` and
  candidate passes `5/6` while preserving `2/2` on held-out within cost and
  latency budgets (`code=0`, `PROMOTED: candidate passed two-arm eval and held-out gate`).
- **Refusal:** an empty suite refuses with `CAP_FAIL_NO_CASES` (`1`); a suite
  with zero should-not-fire held-out cases refuses with `CAP_FAIL_NO_HELDOUT`
  (`2`); a held-out restraint regression refuses with `CAP_FAIL_HELDOUT_REGRESSED`
  (`4`) even when overall passes rise; zero candidate passes refuse with
  `CAP_FAIL_ZERO_PASS` (`5`); cost/latency overruns refuse with `6`/`7`; and
  unsafe capability slugs (`../escape`, `repo--map`) are rejected.
- **Independent check:** `coax/bin/coax run openagents/examples/capability-cases-gate.coax`
  and `--release` match [`openagents/examples/capability-cases-gate.out`](../examples/capability-cases-gate.out),
  alongside the boundary test table and `#authority` mutation suite in
  [`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax).

Planned paths: `openagents/eval/suite.coax`.

## Sources

- **Episodes:**
  - [Episode 6, *Implementing Agent Data Models via TDD*](../../docs/transcripts/006.md): test-first from the start
  - [Episode 29, *Automating Tests*](../../docs/transcripts/029.md)
  - [Episode 120, *Exploring SWE-bench Verified*](../../docs/transcripts/120.md): studying a benchmark
  - [Episode 121, *SWE-bench Planning*](../../docs/transcripts/121.md): benchmark planning
  - [Episode 252, *Observer, Automated QA*](../../docs/transcripts/252.md): verification obligations and exact receipts
- **Old code at `coder-v1.0.0`, crates:** `ext-eval`, `eval-runner`, `coderbench`, `computer-conformance`
- **NIPs:** [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Reach and restraint"
  - `knowledge/slip.*.md` at `coder-v1.0.0`
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P5, P15

## Source listing

Run the Chapter 44 example in [`openagents/examples/capability-cases-gate.coax`](../examples/capability-cases-gate.coax) (committed transcript [`openagents/examples/capability-cases-gate.out`](../examples/capability-cases-gate.out)):

<!-- source: openagents/examples/capability-cases-gate.coax capability-cases-gate -->
```jai
ccg_run :: () -> bool {
  // 1. 6-case suite: 4 should-fire positive cases + 2 should-not-fire held-out cases.
  suite := CapEvalSummary.{
    total_cases = 6,
    heldout_cases = 2,
    baseline_passed = 2,
    candidate_passed = 5,
    baseline_heldout_passed = 2,
    candidate_heldout_passed = 2,
    candidate_cost_micros = 400,
    max_cost_micros = 500,
    candidate_latency_ms = 850,
    max_latency_ms = 1200,
    atif_verified = true
  };
  v_ok := cap_admit_promotion(suite);
  print("suite total=% heldout=% base=% cand=% code=% label=%\n",
    suite.total_cases, suite.heldout_cases, suite.baseline_passed, suite.candidate_passed,
    v_ok, cap_verdict_label(v_ok));

  // 2. Fail-closed suite & grader refusals.
  s_empty := suite; s_empty.total_cases = 0;
  s_no_ho := suite; s_no_ho.heldout_cases = 0;
  s_ho_reg := suite; s_ho_reg.candidate_heldout_passed = 1; // total still 5 > 2, but restraint regressed!
  s_zero := suite; s_zero.baseline_passed = 0; s_zero.candidate_passed = 0; s_zero.baseline_heldout_passed = 0; s_zero.candidate_heldout_passed = 0;
  s_cost := suite; s_cost.candidate_cost_micros = 501;
  s_lat := suite; s_lat.candidate_latency_ms = 1201;

  v_empty := cap_admit_promotion(s_empty);
  v_no_ho := cap_admit_promotion(s_no_ho);
  v_ho_reg := cap_admit_promotion(s_ho_reg);
  v_zero := cap_admit_promotion(s_zero);
  v_cost := cap_admit_promotion(s_cost);
  v_lat := cap_admit_promotion(s_lat);

  print("refuse empty=% no_heldout=% heldout_regressed=% zero_pass=% cost=% latency=%\n",
    v_empty, v_no_ho, v_ho_reg, v_zero, v_cost, v_lat);

  // 3. Capability slug hygiene.
  slug_good := cap_valid_slug("repo-map-v1");
  slug_bad_path := cap_valid_slug("../escape");
  slug_bad_dash := cap_valid_slug("repo--map");
  print("slugs valid=% reject_path=% reject_double_dash=%\n",
    slug_good, !slug_bad_path, !slug_bad_dash);

  return v_ok == CAP_PASS &&
         v_empty == CAP_FAIL_NO_CASES && v_no_ho == CAP_FAIL_NO_HELDOUT &&
         v_ho_reg == CAP_FAIL_HELDOUT_REGRESSED && v_zero == CAP_FAIL_ZERO_PASS &&
         v_cost == CAP_FAIL_COST_BUDGET && v_lat == CAP_FAIL_LATENCY_BUDGET &&
         slug_good && !slug_bad_path && !slug_bad_dash;
}
```

Output (`coax/bin/coax run openagents/examples/capability-cases-gate.coax`, identical in `--release`):

```text
suite total=6 heldout=2 base=2 cand=5 code=0 label=PROMOTED: candidate passed two-arm eval and held-out gate
refuse empty=1 no_heldout=2 heldout_regressed=4 zero_pass=5 cost=6 latency=7
slugs valid=true reject_path=true reject_double_dash=true
ok=true
```
