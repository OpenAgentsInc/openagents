# openagents.probe_worker_attachment.v1

Status: ratified scaffold for Cloud MVP issue `CND-012`

This contract is the local adapter boundary between managed Cloud workrooms and
Probe coding-agent runtime.

## Worker Attachment

`openagents.probe_worker_attachment.v1` carries:

```text
workroom_id
program_id
worker_id
workspace_root
capability_names
raw_secret_access
secret_refs
artifact_dir
receipt_sink
updated_at_ms
```

The attachment is valid only when `raw_secret_access=false`. `secret_refs` are
broker references, not raw token values. The worker is scoped to one workroom
workspace root and one explicit capability list.

## Closeout Receipt

`openagents.probe_closeout_receipt.v1` carries:

```text
receipt_id
workroom_id
worker_id
status
artifact_refs
receipt_digest
emitted_at_ms
```

Closeout receipts must cite at least one artifact ref and must not contain raw
secret-looking values. `oa-node status --json` projects local closeout receipt
digests into `openagents.cloud_node.v1` `evidence.artifact_receipts`.

## Fixture Set

The executable fixture set lives in `fixtures/probe_worker_attachment_v1/`:

- `workroom-probe.json`

The `openagents-cloud-contract` crate parses and validates the fixture.
