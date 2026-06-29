# Pylon W-7 Dogfood Proof

Date: 2026-06-18

This is a Pylon-managed dogfood proof artifact for issue #5383. It records the
public-safe command shape and acceptance chain for the W-7 proof task without
including secrets, private prompts, provider payloads, or customer-private data.

## Public-Safe Refs

- Primary sessionRef: `session.pylon.control.263da397645a497bc17e4e7f`
- Result ref: `result.pylon.control_session.73d5452382899868c46b5640`
- Artifact ref: `artifact.pylon.control_session.proof.c0d367767e87372ea5ea7984`
- Workspace ref: `workspace.pylon.control_session.e7471ed86b24291cd725f2fc`
- Source commit: `93130b858525e305ffd08ff9caf56014248e950e`

## Command Shape

The recorded proof run uses this command shape:

```sh
pylon sessions exec --managed-worktree --lane local --approval-policy auto --verify "test -f docs/launch/2026-06-18-pylon-w7-dogfood-proof.md"
```

The verify command was:

```sh
test -f docs/launch/2026-06-18-pylon-w7-dogfood-proof.md
```

The auto approval policy was selected for the run; no pending approvals were
needed.

## Acceptance Chain

- W-1
- W-3
- W-4
- W-5
- W-6
- W-7

## W-4 Batch Pass

- Batch schema: `openagents.pylon.sessions_batch_result.v0.1`
- ok: `true`
- taskCount: `2`
- concurrency: `2`
- Task `w7-batch-plan`: sessionRef
  `session.pylon.control.cfd09a23932bebf5aaf4ec16`, resultRef
  `result.pylon.control_session.cd3ed831903e27e2f2f24d65`, artifactRef
  `artifact.pylon.control_session.proof.cb70fff2a3f838f566795c45`; verify
  passed with a clean changeset.
- Task `w7-batch-runbook`: sessionRef
  `session.pylon.control.0e121291cfdc231f1c0fa297`, resultRef
  `result.pylon.control_session.dd2910f3725dc0abc016acff`, artifactRef
  `artifact.pylon.control_session.proof.83102ebef3b65e056970d4cb`; verify
  passed with a clean changeset.
- failures: empty

No product or day-to-day coding claim is green until this proof and the batch
pass are reviewed.
