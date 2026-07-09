# oa-node Forge Assignment Adapter

Status: Cloud MVP scaffold for `CND-010`

`oa-node forge assignment receive` is the local adapter for bounded Forge
assignments.

```bash
oa-node forge assignment receive \
  --file fixtures/forge_assignment_v1/workroom-assignment.json \
  --json
```

The adapter reads `openagents.forge_assignment.v1`, validates that the assignment
carries template, capability, budget, artifact, and receipt policy, and emits an
`openagents.forge_assignment_receipt.v1` receipt.

## Decisions

The MVP adapter accepts only the scaffold workroom sidecar capability when the
node is initialized and desired mode is `online`:

```text
assignment_kind = workroom
capability_id = workroom.sidecar.scaffold
```

It refuses:

- invalid assignments;
- assignments targeting another node id;
- uninitialized nodes;
- degraded or quarantined nodes;
- nodes whose desired mode is not `online`;
- `open_ended_labor`, which must route back through Forge/Probe;
- capabilities not yet attached by later Cloud MVP issues.

The accepted decision means typed local intake only. Workroom lifecycle,
artifact closeout, Psionic/Probe attachments, and execution are owned by later
CND issues.

## Receipts

Receipts are appended to:

```text
forge-assignment-receipts.jsonl
```

Each receipt records:

- `decision`: `accepted` or `refused`;
- a stable reason;
- `assignment_digest`;
- `receipt_digest`;
- `emitted_at_ms`.

The admin store updates `receipt_cursors.job_receipt_cursor` to the latest
receipt digest. `oa-node status --json` projects receipt digests into
`evidence.job_receipts`.

This makes refusal observable to Forge without pretending the node launched a
workroom or worker before those lifecycle surfaces exist.
