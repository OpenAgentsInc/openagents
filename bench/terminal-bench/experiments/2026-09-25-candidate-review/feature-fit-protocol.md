# Fit a combined evidence model with task cross-validation

The attributed observation/report union failed the reused comparison: 4/7 fail
precision and 4/60 recall. Retain that negative result. The attribution and repair
stream fixes stand independently of the model's failure to generalize.

Rather than another Boolean threshold, fit a ridge logistic model on runtime
features. Use only the original 132 calibration trials. Regenerate the five
original report judgments through the corrected report adapter and retain their
full inputs, answers, and self-report detector result. Do not use task names,
agent/model identities, policy names, or official verifier observations as
predictor features. Task identity is only a grouping variable for splits and
weights; official reward is only the training/evaluation label.

Compare three fixed feature families:

1. The five report judgments and deterministic self-report admission.
2. Those features plus the three execution and three concrete-report judgments.
3. Those features plus retained Astra source score and whole-task readiness score.

For every nullable feature, fill missing values with 0.5 and add a presence bit.
Give each task equal total training weight, scaled to the number of rows. Fit
ridge penalties 0.1, 1, and 10, with an unpenalized intercept and 3,000 gradient
steps at learning rate 0.5. Produce out-of-fold predictions by leaving out one
whole task at a time. Consider fail cutoffs 0.50 through 0.95 in steps of 0.01.
Require at least five out-of-fold failure calls on at least three tasks, with
90% observed precision. Choose most correct detections, then fewer false alarms,
then the cheaper feature family, stronger regularization, and higher cutoff.
If none qualifies, do not run a comparison arm. Otherwise refit that fixed model
on all calibration tasks, freeze its coefficients and cutoff, and then apply it
to the comparison and prospective cohorts without fitting on either.

Report all out-of-fold alternatives and the final in-sample fit separately.
The historical comparison is repeatedly reused and follows multiple negative
arms; its intervals do not correct for that adaptive selection. Keep prospective
grades unopened until frozen predictions are retained. Unknown inputs cannot
become pass calls. This experiment generates fail or unknown only, keeping the
existing three-state interface without interpreting absence of failure as success.
