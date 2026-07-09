# oa-node Probe Worker Attachment

Status: Cloud MVP scaffold for `CND-012`

`oa-node probe attach` imports a scoped Probe worker attachment:

```bash
oa-node probe attach \
  --file fixtures/probe_worker_attachment_v1/workroom-probe.json \
  --json
```

The file is copied into the node state directory as:

```text
probe-worker.json
```

The attachment uses `openagents.probe_worker_attachment.v1` and is valid only
when:

- it names one workroom and one workspace root;
- it lists allowed capabilities explicitly;
- `raw_secret_access=false`;
- secret references are broker references, not raw token values.

## Closeout

Probe closeout receipts are appended with:

```bash
oa-node probe closeout append \
  --workroom workroom.local.echo \
  --worker probe.worker.local \
  --artifact artifact://probe/transcript \
  --artifact artifact://probe/summary \
  --status succeeded \
  --json
```

Receipts are written to:

```text
probe-closeout-receipts.jsonl
```

Each `openagents.probe_closeout_receipt.v1` receipt records status, artifact
refs, a local receipt digest, and emission time. `oa-node status --json`
projects closeout receipt digests into `evidence.artifact_receipts`.

The current adapter is file-backed. A later live Probe service adapter can
replace the file source while preserving the scoped workspace, capability, and
no-raw-secret contract.
