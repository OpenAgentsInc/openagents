# Abstention-floor selection for evidence relevance, and a failed held-out confirmation

On 2026-09-22, hosted Jev (`jev-1.13.0`) answered the evidence-relevance
question set over a ten-item development suite built to select an
`abstain_below` floor for the function's policy. The floor the
development data nominates — anywhere from 0.70 to 0.80, which rescues
the one below-floor error and loses nothing — **fails held-out
confirmation**: correct held-out picks land as low as 0.47, so every
floor that rescues an error also discards correct answers. No
abstention floor is declared. This is the threshold-selection half of
openagents#9503's sixth acceptance item: selected on development data,
checked against the retained held-out run, and rejected.

## What was asked

| Artifact | Digest |
| --- | --- |
| `openagents.evidence-relevance.v1` | `f827ab947d875a714e83cc66a04e5fa51951bfdb30605d73503907f369b1beb0` |
| `evidence-select-dev-v1` suite | `5a1714660ccf6c625ab97e5aa1711d8e93a54ad60e828015c8f9ab6e1d8b448d` |
| `evidence-select-v1` held-out suite | `2f72d3e41cf851b87c49448a89b6efc8cfab4342d530fed1447a1b66c0eabebc` |

The development suite is
[`crates/gym/suites/evidence-select-dev-v1.json`](../../../crates/gym/suites/evidence-select-dev-v1.json):
ten author-labeled task-plus-candidate records disjoint from the held-out
suite — a clear pick among distractors, a no-relevant abstention, a
refused path-only truth, truncated spans, a duplicated path under two
spans, a search hit, an omitted candidate, and coverage-partials. It ran
through the same
[`select_eval`](../../../crates/coder/examples/select_eval.rs) driver the
held-out suite ran, against the same door. The raw exchange is retained
at [`2026-09-22-evidence-select-dev.raw.jsonl`](2026-09-22-evidence-select-dev.raw.jsonl).

## Development results

8 of 10 correct, both misses instructive:

- `dev-04` (gate 0.68): the truth was a refused path-only candidate and
  the model preferred a file it could read. Wrong at moderate
  confidence — the case an abstention floor exists to catch.
- `dev-09` (gate 0.92): the model named the file defining the header
  constants rather than the search-hit file where the metrics request
  is built. A confident error — and an honestly ambiguous item, since
  the named file is genuinely related. No floor below 0.93 touches it.

## Floor sweep, development data

| Floor | Correct kept | Errors rescued | Confident errors left | Correct lost |
| --- | --- | --- | --- | --- |
| 0.50–0.60 | 8 | 0 | 2 | 0 |
| 0.70–0.80 | 8 | 1 | 1 | 0 |
| 0.85 | 7 | 1 | 1 | 1 |
| 0.90 | 6 | 1 | 1 | 2 |
| 0.93 | 6 | 2 | 0 | 2 |
| 0.95 | 4 | 2 | 0 | 4 |

The development set nominates 0.70–0.80: the only window that rescues
`dev-04` while keeping every correct pick.

## Held-out confirmation

Re-scored against the retained
[held-out raw log](2026-09-21-evidence-select.raw.jsonl) without new
dispatches:

| Floor | Correct kept | Errors rescued | Confident errors left | Correct lost |
| --- | --- | --- | --- | --- |
| 0.50 | 15 | 0 | 2 | 1 |
| 0.60 | 13 | 1 | 1 | 3 |
| 0.70–0.80 | 10–11 | 1 | 1 | 5–6 |
| 0.85–0.93 | 10 | 1 | 1 | 6 |
| 0.95 | 8 | 1 | 1 | 8 |

Held-out correct answers occupy 0.47 (`es-11`) through 1.00, and both
held-out errors sit at 0.58 and 0.98 — inside and above the correct
band. There is no floor that rescues an error without discarding more
correct answers than it saves.

## Conclusion

The development and held-out distributions disagree about where a floor
would sit, and the held-out set — the one the floor was selected to
serve — shows the gate probability is not separable enough on this
model to carry a nonzero `abstain_below`. The set's policy therefore
declares evidence and no abstention floor: the abstention mechanism is
implemented and tested, the measurement says do not use it here. A
floor set on the development window alone would have looked free and
silently cost five or six correct picks in the workflow that matters.

`dev-09` also bounds what a floor can ever do on this set: confident
errors exist above any plausible cut, and abstention is not a
correctness guarantee — consistent with the policy's wording that a
probability is not a correctness claim.
