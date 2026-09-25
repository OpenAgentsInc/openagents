# Reproduce candidate failures before judging them

Static counterexamples and report-based models have not established the required
gain. The pooled cheap model qualifies only at five out-of-fold true detections
out of 122 failures. Its frozen prospective evaluation remains separate.

Before opening any prospective grade, add one fixed evidence-generating arm on
all eight Luna and eight Astra control trials. It receives the public task and
retained candidate, without a writer report, earlier verdict, or official grader.
Use the exact submitted snapshot when it can be attributed; otherwise retain an
unavailable result. Do not substitute an initial or discarded candidate.

Astra high can run at most seven inspection or test commands and then submit up
to three findings. Each command runs inside a disposable task-environment image,
with the submitted workspace mounted read-only, no network, no host credentials,
no Docker socket, a read-only root, and bounded temporary space. The reviewer may
write scratch tests but must not change the candidate. Failed build attempts
caused by read-only enforcement, missing retained state, unavailable services,
optional tools, or timeouts do not establish task failure. Give the session 300
seconds and a $2 soft list-price bound; a response can cross the spend bound.
Each command has a 30-second inner deadline and a 35-second supervised client
bound. The harness removes the container on completion or timeout.

Every finding must quote the public requirement and an actual retained tool
result, identify its tool call, and explain the expected and observed behavior.
Code checks those citations. Jev then judges whether the observed command actually
reproduces the claimed defect in the unchanged candidate, and whether the behavior
is mandatory within the public task's stated domain. Use the minimum score and
freeze the fail cutoff at 0.8, with unknown otherwise. This is a prespecified
experimental cutoff, not a calibrated 80% precision claim. No clean review means
pass. Missing evidence remains unknown.

Retain every request, response, command, output, identity, error, cost, and timing.
Seal all predictions before reading any official outcomes. Measure this arm alone
and keep the previously frozen arms separate. This prospective sample contains
only eight task groups; report uncertainty and do not hide false alarms or
unavailable cases. Later changes require new development and confirmation data.

Preflight corrections before any grade: a composition's one `primary` branch is
its original executor, not evidence of a later writer. The first runner refused
all 15 candidates for that reason without making model calls; its records stay
under `reproduced-preflight-v1`. The corrected runner admits exactly one primary
branch and no escalation, repair, second executor, or persistence. It reuses
pinned public image identities after checking their source manifests.

The current runner supports only a complete `/app` candidate. Collected artifacts
outside that root (including a separately collected patch or installed framework
source) make the review unavailable, rather than silently substituting the public
image's original files. Three of the first 15 candidates have this limitation.
The temporary preflight records describe these exclusions before review starts.

## Quoted-passage recovery before outcome joins

The first four completed reviews exposed a citation-interface mismatch. The
reviewer put exact public quotations inside explanatory text in its `requirement`
field. The original validator compared that entire field with the task, so it
rejected otherwise literal quotations before Jev saw the finding. Preserve that
strict arm unchanged. A separate `reproduced-rejudge` arm accepts either an exact
whole-field quotation or all explicitly double-quoted/backticked passages in that
field, each at least eight characters and present verbatim after whitespace
normalization. Unmatched quotes or invented quoted passages are refused. The
actual-output quotation still must match the named recorded command.

Freeze this recovery before opening any official outcomes. Reuse the exact
native findings and executions; make no new native review. Ask the unchanged
reproduction and mandatory-scope questions at the unchanged 0.8 cutoff. Record
new Jev costs, citation mode, original-record digest, and original candidate
identity. Measure strict and recovered arms separately. This is an implementation
correction discovered from unlabeled records, not permission to tune on grades.
