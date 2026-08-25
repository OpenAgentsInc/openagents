# @openagentsinc/coder-effectiveness

The effectiveness suite for `openagents coder`
([#34](https://openagents.com/OpenAgentsInc/openagents/issues/34)). It grades a
completed Harbor job of coder runs, counts the outcomes a verifier accepted,
and reports **cost per accepted outcome** — with unpriced lanes marked unknown
rather than zero.

## Where it sits

The execution half already exists. `bench/run-suite.sh` packs the working-tree
CLI, runs a suite through Harbor against the installed-agent adapter from
[#35](https://openagents.com/OpenAgentsInc/openagents/issues/35), and leaves a
job directory on disk. This package reads that directory and answers the
question the run was for.

```sh
# 1. Run the suite. Harbor grades the trials.
bench/run-suite.sh bench/suites/tb2-cross-section.txt \
  --model openai/gpt-5.6-luna --jobs-dir /tmp/gym-jobs-run

# 2. Score it.
pnpm run effectiveness:report -- /tmp/gym-jobs-run/<job-dir> \
  --suite tb2-cross-section --lane proxy \
  --thresholds packages/coder-effectiveness/thresholds/tb2-cross-section.json
```

Nothing new has to be produced by the coder for this to work. The suite reads
the artifacts the run already leaves: each trial's `result.json` for the
verifier's decision, the coder's own ATIF `trajectory.json` for tokens and tool
calls, and `coder.txt` for the thread the trial ran in.

## The metric

**Cost per accepted outcome is the run's total cost divided by the outcomes a
verifier accepted, failures included.** You pay for the trials that fail, so an
agent whose success rate halves doubles what an accepted outcome costs you even
if every individual attempt got cheaper. That is the same definition the
benchmark report already uses
(`apps/openagents.com/workers/api/src/inference/benchmark/report.ts`,
`costPerAcceptedOutcomeMsat`).

Reported alongside it, from the same run: success rate, ungraded count, prompt
and output tokens, cached reads, tool calls, and wall clock.

## Unpriced lanes

**A lane with no published rate reports `unknown`. It is never priced at zero,
and it never borrows a fallback rate.**

This is not a hypothetical. `gpt-5.6-luna` is the lane the graded runs use
most, and the forge model catalog
(`OpenAgentsInc/openagents.com` `config/config.exs`) deliberately gives it no
`pricing` key. The config says why: _"This entry deliberately omits `pricing`,
so a grant pinned to it records no estimated cost rather than a made-up zero."_
The worker pricing table leaves it out for the same reason.

There is a tempting fallback nearby and it is the wrong tool. The worker's
`UNKNOWN_MODEL_COST` exists so an un-tabled model is not _under-charged at the
till_, and the source marks it "not a measured rate". Charging conservatively
and measuring honestly are opposite jobs. Borrowing that number here would
fabricate the exact figure this suite exists to report, so this package does
not import it.

Four dispositions, each `usd: null`:

| Disposition            | Meaning                                                                                        |
| ---------------------- | ---------------------------------------------------------------------------------------------- |
| `unpriced_model`       | The catalog carries this id and deliberately declines to price it (`gpt-5.6-luna`).            |
| `unknown_model`        | The catalog has never heard of this id. A weaker finding than the one above.                   |
| `unmetered_local_lane` | An `ollama:` lane bills no metered tokens, so no per-token rate applies. Not free — unmetered. |
| `unknown_usage`        | A rate exists but the trial reported no token counts, or counts that contradict each other.    |

And the case a careless aggregate gets wrong: when **some** trials price and
others do not, the run reports `cost_partial` and withholds the number.
Summing the priced trials and dividing by every accepted outcome yields a
real-looking figure that is too low by whatever the unpriced trials cost. The
coverage (`2 of 3 trials priced`) is printed instead.

### Placeholder rates

The two ids that _do_ carry rates — `gemini-3.7-flash` and `ox-alpha` — carry
rates the forge config marks itself: _"Placeholder: the operator must set real
provider rates before accepting any spend."_ A number derived from them is
arithmetically sound and economically provisional. It is reported, and it
carries `rateBasis: "operator_placeholder"` everywhere it travels, so a
threshold can refuse to score against it.

## The gate

A thresholds file declares the floors. `thresholds/tb2-cross-section.json` is
the checked-in set for the cross-section suite.

A criterion is `passed`, `failed`, or `unverifiable`, and the third one is the
point. If a thresholds file declares a cost ceiling and the run happened on an
unpriced lane, the honest answer is not "under budget" — nothing was measured.
So the criterion is `unverifiable`, the gate is `unverifiable`, and the CLI
exits **2**. A scheduled job that only checks for a zero exit cannot read
silence as green.

The same rule covers placeholder rates: scoring a dollar ceiling against rates
the config calls provisional is scoring against a guess, so it is
`unverifiable` unless the thresholds file opts in with
`acceptPlaceholderRates: true`. That is a reasonable thing to do for a relative
regression check, and an unreasonable thing to do quietly.

A measured breach outranks an unmeasurable criterion: a run that fails the
success floor and cannot be priced is `failed`, because something _was_
measured and it broke.

| Exit | Meaning                                     |
| ---- | ------------------------------------------- |
| 0    | Every applicable floor passed.              |
| 1    | A floor was breached.                       |
| 2    | The gate could not be verified. Not a pass. |

## Grading

A trial is `accepted` only when a verifier **ran** and returned a positive
reward. A trial whose verifier never ran is `ungraded` — neither a pass nor a
failure, kept out of the success-rate denominator, counted and reported, and
capped by its own floor.

This is not pedantry. Terminal-Bench images are amd64 and their verifier
segfaults under qemu on Apple Silicon, so a crashed grader is a routine local
outcome. Folding those trials into either bucket would move the headline number
for a reason that has nothing to do with the coder.

## Tests

Every case reads a checked-in fixture Harbor job under `fixtures/`. No model is
called, no Docker image runs, and no clock is read.

| Fixture            | What it proves                                                    |
| ------------------ | ----------------------------------------------------------------- |
| `priced-lane`      | A fully priced run produces a real number.                        |
| `unpriced-lane`    | A `gpt-5.6-luna` run reports unknown, never zero.                 |
| `mixed-lane`       | A partly priced run withholds the number.                         |
| `crashed-verifier` | Ungraded trials stay out of both buckets.                         |
| `regressed-lane`   | A regression raises cost per accepted outcome and trips the gate. |

```sh
pnpm --dir packages/coder-effectiveness test
```

## Rate catalog

`src/pricing.ts` pins a snapshot of the forge catalog rates, following the
precedent in `packages/rlm-recall-eval/src/price-catalog.ts`: the forge config
is not a module this package can import, a pinned version keeps a graded run
reproducible from a clean checkout, and cost provenance has to be auditable
rather than implied. Bump `CODER_RATE_CATALOG_VERSION` when the rows change.

To score against the catalog a deployment actually serves, capture
`GET /api/v1/models` and pass `--models <file>`. A model the served catalog
leaves unpriced stays unpriced here — that omission is the signal.

## Not yet done

- **Per-model cost from the coder's own trajectory.** The ATIF exporter writes
  `total_prompt_tokens` and `total_completion_tokens` and no cost, and cached
  reads survive only per step as `metrics.extra.cache_read_input_tokens`, which
  this package sums. Once
  [#36](https://openagents.com/OpenAgentsInc/openagents/issues/36) lands the
  per-model-family seam, the token-economy delta it measures is readable from
  this suite's rows without a change here.
- **Two consecutive scheduled runs and a caught live regression.** Issue #34's
  acceptance needs real Harbor runs on amd64 hardware; the fixture runs prove
  the grading and the arithmetic, not the schedule.
- **Appending results to `bench-results` with receipts.** The report is
  `--json`-shaped and ready for it; the store is not wired.
