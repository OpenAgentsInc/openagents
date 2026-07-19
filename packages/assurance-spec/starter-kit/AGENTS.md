# Agent instructions

## AssuranceSpec

`assurance/` holds `.assurance-spec.md` proof-design artifacts.
They state the evidence that can prove each ProductSpec criterion.
They are not tests, verdicts, admission, release authority, or permission to change intent.

Before you implement or claim governed work, pin a session with
`assurance-spec session begin <file>`.
Work by obligation ID and report every status axis.
Do not increase the reported status.
`not_run` is a normal, honest state.
Run the owned-runner verification before handoff.
Never edit an oracle, falsifier, digest, lifecycle state, or gate only to make work pass.
