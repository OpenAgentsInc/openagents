# Agent instructions

## AssuranceSpec

`assurance/` holds `.assurance-spec.md` proof-design artifacts. They commit
what evidence would justify believing each ProductSpec criterion; they are not
tests, verdicts, admission, release authority, or permission to change intent.

Before implementing or claiming governed work, pin a session with
`assurance-spec session begin <file>`, work by obligation ID, and report every
status axis without rounding up. `not_run` is a normal, honest state. Run the
owned-runner verification before handoff. Never edit an oracle, falsifier,
digest, lifecycle state, or gate merely to make work pass.
