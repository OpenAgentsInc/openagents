# Agreement between complementary assessments

The whole-task assessor failed its standalone calibration bar: even 0.95 gave
only 5/6 correct failure calls. Keep that negative result and its $22.33042 of
known native model cost. Source-counterexample review and completion assessment
make different errors on calibration: several confident completion false alarms
have no grounded source finding. Their scores are complementary features,
although both use Astra and must not be assumed statistically independent.

Before reviewing the 185 comparison candidates with the completion assessor,
evaluate one joint family on calibration only:

- Fail when the concrete-report audit reaches its already-frozen 0.5 cutoff.
- Otherwise, fail only when both source-counterexample score and completion score
  reach their selected cutoffs.
- Otherwise, remain unknown.

Select the two cutoffs from the original five-by-five grid (0.5, 0.7, 0.8, 0.9,
0.95). Require at least five union failure calls and 90% observed precision.
Maximize correct failures, then minimize false alarms, then prefer higher source
and completion cutoffs. No per-task exception, new prompt, new model response,
or comparison result participates in selection. Freeze the resulting rule before
completion assessment of comparison rows and before joining fresh grades. The
historical comparison remains reused validation; the eight fresh labels remain
unopened. Report the full cost of producing both reviews, including unknown
usage, and retain every rejected arm. Agreement is an empirical signal, not proof.
