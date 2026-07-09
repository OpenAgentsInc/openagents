# GPU media-run closeout checklist (SQ-8 / #8625)

Date: 2026-07-09
Issue: OpenAgentsInc/openagents#8625 (epic #8610)
Source: `docs/sarah/2026-07-09-sarah-quality-next-steps-assessment.md` §P2

Media quality work is GPU-shaped and can quietly leak money. **Every** OAV /
Sarah media run must end with a machine-checkable closeout receipt — not a
chat log saying "looks fine."

## Law (non-negotiable)

1. **Artifact-existence monitor, never log-marker monitor.**
   A line in stdout is not evidence. Every claimed deliverable must be a real
   object (`gs://…` blob or local path that `stat`/`gsutil stat` can see).
2. **Host disposition is explicit.**
   One of: `stopped` | `deleted` | `left_running` **with a reason**.
   Prod render node `sarah-avatar-gpu-1` is allowed `left_running` with reason
   `prod_render_node` (~$0.85/h — revisit sizing under SQ-4). Experimental
   hosts must not stay up by accident.
3. **GCS artifact index updated.**
   Index entry (or receipt listing) records every new object under
   `gs://openagentsgemini-oa-artifacts/sarah-avatar/…`.
4. **Token/GPU cost estimate recorded.**
   Approximate is fine; silence is not. Record wall clock + GPU class +
   estimated USD when known.
5. **No secrets in artifacts.**
   No raw prompts that embed secrets, API keys, private prospect PII, live
   credentials, or customer emails in the receipt or uploaded objects.

## When to run

After every GPU media session that:

- Renders MuseTalk / CosyVoice / LatentSync / opener takes
- Bakes or upgrades models on an L4/GCE host
- Uploads anything under `gs://openagentsgemini-oa-artifacts/sarah-avatar/`

## Operator flow

```bash
# 1. Write a closeout receipt (copy the example)
cp apps/sarah/fixtures/gpu-media-run-closeout.example.json \
   /tmp/closeout-<run-id>.json
# Edit: runId, artifacts, hostDisposition, costEstimate, …

# 2. Validate (offline schema + privacy rules)
bun apps/sarah/scripts/gpu-media-run-closeout.mjs \
  --receipt /tmp/closeout-<run-id>.json

# 3. Optional live checks (needs gcloud/gsutil + SA)
bun apps/sarah/scripts/gpu-media-run-closeout.mjs \
  --receipt /tmp/closeout-<run-id>.json \
  --live-artifacts

# 4. Commit or attach the receipt next to the take (docs/sarah/receipts/ or GCS)
```

Package script:

```bash
cd apps/sarah
bun run closeout:gpu-media -- --receipt path/to/receipt.json
bun run test:gpu-media-closeout
```

## Receipt schema (v1)

`schemaVersion`: `openagents.sarah.gpu_media_run_closeout.v1`

| Field | Type | Rules |
|---|---|---|
| `runId` | string | Non-empty public-safe id (no secrets) |
| `issueRefs` | string[] | e.g. `["#8610", "#8625"]` |
| `startedAt` / `endedAt` | ISO-8601 | `endedAt` ≥ `startedAt` |
| `host` | object | `name`, `project`, `zone`, `machineType`, `gpu` |
| `hostDisposition` | object | `status`: `stopped` \| `deleted` \| `left_running`; if `left_running`, `reason` required (non-empty) |
| `artifacts` | array | Each: `uri` (`gs://…` or absolute `file://` / path), `kind`, `bytes?`, `sha256?` |
| `artifactChecks` | array | Each: `uri`, `method`: **must be** `object_exists` (not `log_marker`) |
| `gcsIndex` | object | `updated`: true, `indexUri?` or `entries[]` |
| `costEstimate` | object | `currency` (`USD`), `gpuHours?`, `estimatedUsd?`, `notes` |
| `privacy` | object | `attestation`: must claim no secrets/PII in artifacts |
| `notes` | string? | Free text, public-safe |

## Forbidden patterns (validator rejects)

- `artifactChecks[].method === "log_marker"` (or missing checks when artifacts claimed)
- Empty `artifacts` when the run claims a media product
- `hostDisposition.status === "left_running"` without `reason`
- Field names / string values matching secret patterns: `api_key`, `Bearer `, `sk-`, `mnemonic`, private emails in obvious forms
- Missing `gcsIndex.updated === true` for GCS-backed runs

## Prod host note

| Host | Default disposition | Reason |
|---|---|---|
| `sarah-avatar-gpu-1` | `left_running` | `prod_render_node` — permanent OAV render node after 2026-07-09 flip |
| Ephemeral experiment VMs | `stopped` or `deleted` | Always — never leave SPOT/STANDARD research boxes up overnight without a written reason |

## Relationship to other SQ issues

- **SQ-1** scoreboard sits *beside* the media artifact; closeout proves the
  artifact exists and the host/cost state is honest.
- **SQ-4** hardens the live renderer; closeout still applies to every offline
  bake and experiment matrix run (SQ-2).
- **OAV-1 receipt** (`2026-07-09-oav1-offline-proof-receipt.md`) is the
  narrative proof; going forward attach a v1 closeout JSON as well.

## Exit for #8625

- [x] Checklist documented here
- [x] Machine-checkable receipt schema + example fixture
- [x] Validator script + unit tests in `apps/sarah`
- [x] npm scripts wired (`closeout:gpu-media`, `test:gpu-media-closeout`)
