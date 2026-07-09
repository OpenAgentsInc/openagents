# CND-039 MVP Observability

Status: implemented scaffold

MVP observability uses one trace id across node, workroom, ingress, artifact,
and receipt observations. The goal is not a full metrics platform yet; it is a
repeatable way to prove local and GCP test events are visible, correlated, and
redacted.

## Local Collection

```bash
scripts/collect-mvp-observability.sh --local --output-dir /tmp/oa-cloud-observability
```

The local collector creates:

```text
mvp-observability-events.jsonl
node-init.json
node-health.json
node-status.json
workroom-metadata-init.json
ingress-set.json
ingress-status.json
artifact-policy.json
artifact-upload.json
artifact-status.json
closeout-submit.json
lifecycle-create.json
lifecycle-start.json
lifecycle-closeout.json
workroom-metadata-get.json
redaction-proof.txt
```

Every line in `mvp-observability-events.jsonl` includes:

- `trace_id`
- `event_id`
- `source`
- `kind`
- `state_file`

The collector verifies that node, workroom, ingress, artifact, and receipt
events are present and that the resulting log bundle does not contain raw
secret markers.

## GCP Collection

Dry-run:

```bash
scripts/collect-mvp-observability.sh \
  --project "$PROJECT_ID" \
  --zone us-central1-a \
  --node-name oa-node-dev-01
```

Apply:

```bash
scripts/collect-mvp-observability.sh \
  --project "$PROJECT_ID" \
  --zone us-central1-a \
  --node-name oa-node-dev-01 \
  --apply
```

GCP collection prints or runs:

- recent `openagents-oa-node` journald entries over IAP SSH;
- recent Cloud Logging entries for `oa_node_startup_complete`,
  `openagents-oa-node`, and the selected GCE instance;
- local artifact, receipt, and lifecycle JSONL tails under
  `/var/lib/openagents/workrooms`.

## Redaction Boundary

The collector calls `scripts/verify-redacted-config.sh` and scans its own
output bundle for common raw-secret markers. Observability logs may contain
secret references such as `gcp-secret://...`; they must not contain raw bearer
tokens, API keys, auth JSON, wallet seed material, or private keys.
