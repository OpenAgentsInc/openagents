# Score positions, selected answers, and calibration

Issues #9394 and #9419 concern two different quantities: a Score answer's
weighted position and the categorical answer used to measure correctness.
Calibration must preserve the identity of the answer whose confidence it
changes.

## Positions and categorical answers

The public `score` field remains the probability-weighted position,
`Σ i · p_i`, over ordered levels. It can fall between levels. Rounding it
is not equivalent to choosing the most probable level. For example,
`{0: 0.5, 1: 0.375, 2: 0.125}` has a weighted position of `0.625` and a
most probable level of `0`.

The contract applies consistently across doors: `score` promises a weighted
position, not a selected level or measured categorical accuracy. The repository
has not established that this position is an accurate ordinal prediction for
every checkpoint.

| Door | `score` field | Published categorical accuracy |
| --- | --- | --- |
| Lev | Mean of raw sample frequencies, or of the calibrated distribution. | Selected level; calibration preserves its identity. Historical rows without selection provenance use their documented argmax convention. |
| Kev | Probability-weighted mean, rounded to two decimal places. | Argmax of reported probabilities, with the highest tied level selected. |
| Hosted Jev | Weighted position under the public API contract; internal estimation is not inspected here. | Gym selects the reported categorical level when present, otherwise the highest tied maximum of reported probabilities. |

The retained Score accuracy measurements compare a categorical level with a
label. The [ordinality measurement](2026-09-19-score-ordinality.md) now names
that statistic explicitly. They do not establish the quality of the
weighted position as an ordinal prediction.

Without explicit selection provenance, Gym reads Score's categorical answer
from `probabilities`. Tied maxima resolve to the highest level, matching the
ordered rubric and the existing estimator convention. A tie remains an
answered item. Its distribution records the ambiguity; it is not silently
removed as a refusal. The ordinality probe's original first-maximum analysis
is retained and labelled rather than rewritten as a different observation.

Choice already carries its categorical answer in `choice`. Gym reads that
field, including when it differs from the distribution's maximum.

## A calibration map preserves the selected answer

A map calibrates confidence in a fixed answer. It can reduce that answer's
probability below a runner-up without replacing the answer. Correctness
continues to compare the fixed answer with the label.

The standard Noul and Score statistics cannot carry that identity after an
inversion. Lev therefore adds optional `selected` provenance to those answer
shapes. This is an OpenAgents extension; it does not redefine TypeSafe's
`noul` or `score` fields.

| Kind | Selection field | Legacy fallback when absent |
| --- | --- | --- |
| Choice | `choice` | None: the field is required. |
| Noul | `selected`, either `yes` or `no` | `yes` when `noul >= 0.5`, otherwise `no`. |
| Score | `selected`, the canonical decimal level key | Highest level among maxima in `probabilities`. |

Lev writes `selected` on Noul and Score answers, calibrated or raw. Jev's
Rust SDK decodes it and rejects invalid option names. Gym records it and
reads that option's probability when fitting or applying another map.
Absence means no provenance was reported. It does not prove that an external
door is uncalibrated or reveal its internal estimator. In particular, the
hosted Jev implementation is not inspected here.

Older clients can still read the standard numeric fields. They cannot
recover a fixed categorical selection from an inverted distribution unless
they understand `selected`. No threshold or local argmax can reconstruct
that omitted information in general.

## Versioned measurement rows

New rows use `openagents.gym.eval_row.v2`. The `selected` field identifies
the answer, and `raw_top` records the probability reported on that answer.
Despite its historical name, it can now be below the distribution maximum.
This is a semantic change from v1's maximum, so it receives a new schema
identifier. Older store readers reject v2 instead of silently applying the
old interpretation.

The store also accepts historical v1 rows. Their schema, fields, and receipt
chains are not rewritten. When a v1 row lacks a named selection, readers
retain the historical argmax fallback. That fallback cannot reconstruct an
unrecorded non-argmax Choice or prove whether the distribution was calibrated.
Historical claims require their retained provenance, not an assumption based
on an absent field. An old consumer must upgrade before reading new rows.

The gate's policy is separate from its input schema. This fix changes no
threshold, basis, or criterion in `probability-v1` or `ab::Rule::v2`; their
digests do not change. The [re-derivation inventory](../../gym/measurements/2026-09-20-raw-floors-and-mapped-claims.md)
checks the retained records, including their gate verdicts, under the corrected
reader. Unknown historical measurements remain unknown.

## Precision and verification

Kev rounds wire statistics and probabilities to two decimal places. Lev's
raw sample frequencies use a `1/n` grid; calibrated probabilities need not
stay on that grid. Comparisons must use the precision actually reported.
Neither wire rounding nor a tied distribution changes which statistic
`score` represents.

[`calibrated_answer_roundtrip.rs`](../../../crates/lev/tests/calibrated_answer_roundtrip.rs)
checks map application, Lev's public answer shape, JSON, Jev decoding, Gym's
answer selection, stored correctness against a known label, and mapped
observations. Its six cases cover binary and multiclass inversions, a raw
tie, a band-conditioned map, and legacy fallback. Separate SDK tests reject
invalid `selected` values. The retained-store enumeration and metric
re-derivation are in `winner_inversion.rs` and `rederived_floors.rs`.
