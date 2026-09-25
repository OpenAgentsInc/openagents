# Retire the reused comparison and evaluate on fresh candidates

The cross-validated fusion model failed the historical comparison: 8/23 failure
precision and 8/60 recall. Its 24/26 out-of-fold calibration precision did not
transfer. The preceding source, readiness, agreement, and observed-execution arms
also failed to establish the required gain. Do not select another historical
winner or describe that repeatedly inspected partition as untouched validation.

Treat all 317 historical trials on 58 tasks as development data from this point.
The eight fresh Luna trials and eight Astra controls remain outside that data;
their official grades have not been opened. Freeze the new model and all fresh
predictions before joining these grades. Keep all attempted trials, unavailable
observations, infrastructure errors, and nonbinary grades explicit.

Use the same task-weighted ridge logistic fitting and leave-one-task-out procedure
as the preceding feature protocol, with the same ridge grid, 3,000 steps, learning
rate, cutoff grid, minimum five calls across three tasks, and 90% out-of-fold
precision target. Compare three fixed feature families:

1. Five corrected report judgments and deterministic admission.
2. Those features plus the three corrected execution and three concrete-report
   judgments.
3. Those features plus the retained Astra source-review score.

Omit readiness from this fit. Acquisition of that feature was conditional in the
former comparison and complete in calibration; using that availability pattern
as a predictor would encode the historical partition's acquisition policy. The
other features were attempted consistently across both partitions. Missing values
and presence bits keep the same representation. No task, model, policy, agent
identity, or grader observation is a predictor. Task names only define folds and
weights. Select by greatest out-of-fold true detections, then fewer false alarms,
cheaper family, stronger regularization, and higher cutoff. If no model qualifies,
retain that result without opening a comparison under a changed criterion.

Report the 16 prospective trials together and separately by executor, with trial
Wilson intervals and paired whole-task bootstrap differences. There are only
eight task groups; a wide or inconclusive interval remains inconclusive. This
protocol does not pre-authorize a claim of general superiority or retrospective
rule changes after those grades are read. Any later larger confirmation must fix
its sample and procedure before acquiring or examining its labels.
