# oa-node Psionic Worker Attachment

Status: Cloud MVP scaffold for `CND-011`

`oa-node psionic attach` imports local Psionic worker readiness:

```bash
oa-node psionic attach \
  --file fixtures/psionic_worker_attachment_v1/mixed-readiness.json \
  --json
```

The file is copied into the node state directory as:

```text
psionic-workers.json
```

## Readiness Projection

The attachment uses `openagents.psionic_worker_attachment.v1`. `oa-node
status --json` projects workers into the matching Cloud node capability lane:

- `inference` workers become `capabilities.inference_products`;
- `training` workers become `capabilities.training_products`;
- `sandbox` workers become `capabilities.sandbox_profiles`.

A crashed worker sets only that product/profile to `backend_ready=false` and
`eligible=false` with a `worker_crashed` summary. It does not mark unrelated
products or the whole node degraded.

## Execution Receipts

Psionic execution receipts are appended with:

```bash
oa-node psionic receipt append \
  --product psionic.managed.inference \
  --worker psionic.inference.local \
  --assignment forge.assignment.workroom.echo \
  --evidence-digest sha256:psionic-execution-evidence \
  --status succeeded \
  --json
```

Receipts are written to:

```text
psionic-execution-receipts.jsonl
```

Each `openagents.psionic_execution_receipt.v1` receipt includes both the local
receipt digest and the upstream `psionic_evidence_digest`. Status projects local
receipt digests into `evidence.job_receipts`; the receipt log is the source of
truth for which Psionic evidence digest was cited.

Sandbox Psionic receipts must also pass `--profile-digest <sha256:...>` so the
execution receipt can be matched back to the declared sandbox profile.

The current adapter is file-backed. A later live Psionic service adapter can
replace the file source while preserving these contract fields.
