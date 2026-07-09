# openagents.psionic_worker_attachment.v1

Status: ratified scaffold for Cloud MVP issue `CND-011`

This contract is the local adapter boundary between Psionic execution substrate
truth and private `oa-node` capacity advertising.

## Worker Attachment

`openagents.psionic_worker_attachment.v1` carries:

```text
schema_version
updated_at_ms
workers
```

Each worker carries:

```text
product_id
worker_id
worker_kind
ready
crashed
evidence_digest
detail
```

`worker_kind` is one of:

- `inference`
- `training`
- `sandbox`

Ready workers are projected into the matching `openagents.cloud_node.v1`
capability lane. Crashed workers degrade only their own product row; they do not
force whole-node degradation unless the local attachment file itself is corrupt.

## Execution Receipt

`openagents.psionic_execution_receipt.v1` carries:

```text
receipt_id
assignment_id
product_id
worker_id
status
profile_digest
psionic_evidence_digest
receipt_digest
emitted_at_ms
```

The receipt digest is local `oa-node` evidence. The `psionic_evidence_digest`
field cites the upstream Psionic runtime evidence, proof bundle, execution
receipt, or refusal receipt that actually describes the execution result.
Sandbox execution receipts must also cite `profile_digest` so Cloud can verify
the execution against the declared sandbox profile.

## Fixture Set

The executable fixture set lives in `fixtures/psionic_worker_attachment_v1/`:

- `mixed-readiness.json`

The `openagents-cloud-contract` crate parses and validates the fixture.
