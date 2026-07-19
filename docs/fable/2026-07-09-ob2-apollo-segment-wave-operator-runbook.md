# OB-2 (#8559) Apollo Segment Wave Operator Runbook

Date: 2026-07-09  
Status: operator runbook for loading Apollo segment waves into
`business_pipeline_rows`. Apollo stays a **mirror**. The pipeline is the
system of record (BF-9.2). No prospect names, domains, emails, phones, or
raw Apollo payloads may enter this repo or public-safe operator receipts.

Related:

- Issue: [#8559](https://github.com/OpenAgentsInc/openagents/issues/8559)
- Plan: [`2026-07-03-apollo-outbound-sales-plan.md`](./2026-07-03-apollo-outbound-sales-plan.md)
- Subject identity: PR #8617 (`subject_ref` on `business_pipeline_rows`)
- Fixture + dry-run runner:
  - `apps/openagents.com/workers/api/src/business-pipeline-apollo-wave-fixture.ts`
  - `apps/openagents.com/workers/api/scripts/apollo-wave-runner.ts`
- Ingest route: `POST /api/operator/business/pipeline/apollo-waves`

## 0. Exit gate (what “done” means)

#8559 closes only when **both** are true:

1. **Two live segment waves** loaded with **≥100 prospects each**, each row
   carrying LG-6 `sourceRef` attribution and a public-safe `subjectRef`.
2. Operator can re-run a wave and prove:
   - **Idempotent replay** (same `pipelineRef` / `subjectRef` → duplicates, 0 inserts)
   - **Subject-level dedupe** (new `pipelineRef`, same `subjectRef` → duplicate, 0 inserts)
   - **Suppression** (suppressed `subjectRef` never appears in the queue)

The fixture/dry-run tier below proves the machinery without Apollo MCP. It
does **not** close the issue by itself.

## 1. Prerequisites

| Prerequisite | Notes |
| --- | --- |
| Owner Apollo MCP OAuth | Connected on the owner account. Agent session must have the Apollo MCP tools armed |
| Admin API token | `OPENAGENTS_ADMIN_API_TOKEN` for operator routes |
| Migration 0314 applied | `subject_ref` + partial unique index on production D1 |
| OB-1 warm-up cap | Match day’s outbound cap. Do not over-fill the queue past the current send budget |
| Suppression list | Existing customers/partners/active intake already in `business_outreach_suppressions` |

Optional env:

```sh
export OPENAGENTS_BASE_URL=https://openagents.com
export OPENAGENTS_ADMIN_API_TOKEN=…   # never commit
```

## 2. Fixture-tier proof (no Apollo, no production writes)

From the monorepo root or the Worker package:

```sh
cd apps/openagents.com/workers/api

# In-memory D1: two ≥100 waves, suppression, replay, subjectRef dedupe
# (Node is required — dry-run uses node:sqlite like the Worker API tests)
bun run apollo-wave:dry-run -- --count 100
# equivalent:
# node --experimental-strip-types scripts/apollo-wave-runner.ts dry-run --count 100

# Inspect a printable operator body (public-safe synthetic refs only)
bun run apollo-wave:print-fixture -- \
  --segment agencies_seo \
  --count 100 \
  --wave-id fixture

# Vitest store + route coverage
bun run test -- src/business-pipeline-apollo-wave-fixture.test.ts
bun run test -- src/business-pipeline-routes.test.ts
```

Expected dry-run receipt shape (abbreviated):

```json
{
  "ok": true,
  "schemaVersion": "openagents.ob2_apollo_wave_dry_run_receipt.v1",
  "countPerSegment": 100,
  "expectedTotalRows": 198,
  "actualTotalRows": 198,
  "segments": [
    {
      "segmentKey": "agencies_seo",
      "firstPass": { "acceptedCount": 99, "duplicateCount": 0, "suppressedCount": 1 },
      "replay": { "acceptedCount": 0, "duplicateCount": 99, "suppressedCount": 1 },
      "subjectDedupe": { "acceptedCount": 0, "duplicateCount": 99, "suppressedCount": 1 }
    },
    {
      "segmentKey": "legal_small_firm",
      "firstPass": { "acceptedCount": 99, "duplicateCount": 0, "suppressedCount": 1 },
      "replay": { "acceptedCount": 0, "duplicateCount": 99, "suppressedCount": 1 },
      "subjectDedupe": { "acceptedCount": 0, "duplicateCount": 99, "suppressedCount": 1 }
    }
  ]
}
```

(When the fixture suppresses one mid-wave subject per segment, 2 × 99 = 198.)

## 3. Segment config (public-safe)

| Operator key | `segmentRef` | `sourceRef` | Default vertical |
| --- | --- | --- | --- |
| `agencies_seo` | `segment.apollo.agencies_seo` | `apollo_agent_readiness_agency` | `agency` |
| `legal_small_firm` | `segment.apollo.legal_small_firm` | `apollo_model_custody` | `regulated legal` |
| `home_services` | `segment.apollo.home_services` | `apollo_agent_readiness_marketplace` | `home services` |
| `own_your_ai` | `segment.apollo.own_your_ai` | `own_your_ai` | `model custody` |

Exit-gate default pair: **`agencies_seo` + `legal_small_firm`**.

## 4. Live Apollo MCP waves (when MCP is armed)

### 4.1 Discovery order (credit-safe)

1. **Company search first** (domains, no lead credit burn):
   - `apollo_mixed_companies_search` per segment from the outbound plan.
2. **Audit domains** with the agent-readiness prober (OB-3 path). Prefer BAD
   domains only for contact spend.
3. **People search + reveal** only on failing domains:
   - `apollo_mixed_people_api_search` / match tools.
4. **Mirror CRM state in Apollo** if useful for sequences, but **do not**
   treat Apollo as system of record for pipeline stage.

### 4.2 Map Apollo → public-safe pipeline body

For each selected person, emit **only** opaque refs:

| Field | Rule |
| --- | --- |
| `subjectRef` | Stable public-safe id, e.g. `prospect.apollo.<apollo_person_id>` (no email/domain) |
| `pipelineRef` | Unique per intake row, e.g. `biz-pipe-agency-20260709-001` |
| `vertical` | Public-safe descriptor from the segment table |
| `sourceRef` | Segment `sourceRef` from §3 |
| `segmentRef` | Segment `segmentRef` from §3 |
| `waveRef` | Once per wave, e.g. `apollo.wave.agencies_seo.20260709a` |
| quoted band | Optional audit-first band in integer USD cents |

**Before ingest**, suppress existing customers / partners / active intake:

```sh
bun apps/openagents.com/scripts/operator-business-pipeline.ts suppress-outreach \
  --subject-ref prospect.apollo.<id> \
  --reason existing_customer \
  --source-ref crm.suppression.20260709
```

Write the wave body to a **local, never-committed** file, e.g.
`/tmp/apollo-wave-agencies.local.json`:

```json
{
  "waveRef": "apollo.wave.agencies_seo.20260709a",
  "segmentRef": "segment.apollo.agencies_seo",
  "sourceRef": "apollo_agent_readiness_agency",
  "prospects": [
    {
      "pipelineRef": "biz-pipe-agency-20260709-001",
      "subjectRef": "prospect.apollo.abc123",
      "vertical": "agency",
      "quotedBandLabel": "audit first",
      "quotedMinUsdCents": 150000,
      "quotedMaxUsdCents": 500000
    }
  ]
}
```

### 4.3 Ingest wave 1 and wave 2

```sh
cd apps/openagents.com/workers/api

# Wave 1 — agencies / SEO (≥100 after mapping)
bun run apollo-wave:live -- --body /tmp/apollo-wave-agencies.local.json

# Wave 2 — legal solo + small firm (≥100 after mapping)
bun run apollo-wave:live -- --body /tmp/apollo-wave-legal.local.json
```

Equivalent raw curl:

```sh
curl -sS -X POST "$OPENAGENTS_BASE_URL/api/operator/business/pipeline/apollo-waves" \
  -H "authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "content-type: application/json" \
  --data-binary @/tmp/apollo-wave-agencies.local.json
```

### 4.4 Prove idempotency + suppression on live data

1. **Replay the same body** (same `waveRef` / `pipelineRef` / `subjectRef`):
   - Expect `acceptedCount: 0`, `duplicateCount` ≈ previous accepts,
     `suppressedCount` unchanged for still-suppressed subjects.
2. **Second wave, same subjects, new pipelineRefs** (copy body, rename
   `pipelineRef` + `waveRef` only):
   - Expect `acceptedCount: 0` and subject-level duplicates (PR #8617).
3. **List + metrics** (opaque only):

```sh
bun apps/openagents.com/scripts/operator-business-pipeline.ts metrics
curl -sS -H "authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  "$OPENAGENTS_BASE_URL/api/operator/business/pipeline" | head
```

Confirm:

- Each inserted row exposes `subjectRef` (public-safe).
- Suppressed subjects are absent from the list.
- Metrics `sourceRefBreakdown` shows the two Apollo source refs at ≥100
  (or 99 if one suppress hit per wave).
- JSON never contains emails, domains, or raw Apollo payloads.

### 4.5 Staging synthetic live (optional, not an exit receipt)

To exercise the production/staging route without Apollo:

```sh
bun run apollo-wave:live -- \
  --segment agencies_seo \
  --wave-id 20260709a \
  --count 100 \
  --allow-synthetic
```

Treat receipts as **route smoke only**. They do not satisfy the live Apollo
MCP exit gate.

## 5. Receipt checklist for the #8559 issue comment

Paste (redacted) into the issue when both live waves land:

```text
OB-2 live wave receipt
- wave_1: waveRef=… segmentRef=… sourceRef=… accepted=N (≥100 or 99 w/ suppress) suppressed=…
- wave_2: waveRef=… segmentRef=… sourceRef=… accepted=N (≥100 or 99 w/ suppress) suppressed=…
- replay_wave_1: accepted=0 duplicate=N suppressed=…
- subject_dedupe_wave_1b: accepted=0 duplicate=N
- metrics total rows / sourceRefBreakdown: …
- migration 0314 subject_ref visible on listed rows: yes
- no PII in operator JSON: yes
```

## 6. Failure modes

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `subjectRef must be an opaque public-safe ref` | Email/domain slipped into ref | Re-map to `prospect.apollo.<id>` only |
| `sourceRef must be a bounded public-safe token` | Wrong attribution token | Use §3 `sourceRef` values |
| `apollo wave must contain 1-500 prospects` | Empty or oversized body | Cap at 500. Split waves |
| Second wave double-inserts | Pre-#8617 binary / missing 0314 | Deploy migration + subject_ref code |
| Suppressed contact appears in queue | Suppression not seeded / wrong subjectRef | Align suppress `subjectRef` with wave body |
| Credit burn with zero pipeline rows | Apollo reveal before audit / before ingest | Domains first → audit → reveal → map → ingest |

## 7. Explicit non-goals

- No outbound sends here (OB-1 / outreach ledger).
- No public report probing (OB-3).
- No CRM approval batch UI (OB-4).
- No scraping outside Apollo’s terms.
- No committing live wave bodies or Apollo export CSVs to git.
