# Probe Blueprint Action Submission Boundary

Date: 2026-06-07

Status: implemented for Probe issue #180.

Probe now has a typed proposal-only boundary for external write-side effects.
The rule is:

```text
Program Runs are evidence-only.
Action Submissions are the write-side boundary.
```

Probe-local tool execution is still allowed inside a controlled sandbox under
local policy. Examples include local reads, local sandbox file edits, and
recording redacted evidence refs. Those actions do not create Blueprint Action
Submission proposals by default.

External writes are different. Creating pull requests, deploying, sending
email, posting public claims, spending money, legal-sensitive commitments, and
mutating source-backed business facts all classify as Action Submission
required. A Program Run cannot directly execute those effects, regardless of
model confidence.

The current implementation emits
`probe_blueprint_action_submission_proposal` records. A proposal contains
Program Run refs, evidence refs, source authority refs, context refs, tool refs,
approval policy refs, a summary ref, and a redacted typed intent. It always
sets:

- `approvalRequired: true`
- `directExecution: false`
- `directProgramRunExecutionAllowed: false`
- `modelConfidenceBypassDisabled: true`
- `programRunAuthorityBoundary: "evidence_only"`
- `proposalOnly: true`
- `contentRedacted: true`

The proposal validator rejects private-data-shaped material such as raw emails,
callback URLs, callback tokens, provider payloads, private repo contents,
payment material, wallet material, private keys, mnemonics, and raw customer
data.

Apple FM can now receive `tool.probe.propose_action_submission` as a projected
tool, but it is projected with `approval_required` policy. The callback can
produce a proposal record or an approval-pending transcript; it does not execute
the external effect. Full OpenAgents product surface persistence and execution remain future work.
