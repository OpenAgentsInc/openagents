# Local conversation → public /trace/{uuid} ingest

Date: 2026-07-19
Scope: `apps/openagents.com` trace/ATIF surface, `packages/atif`, `apps/qa-runner`.
Status: implemented. New CLI + skill land in this change.

## Goal

Take a Claude, Codex, or OpenAgents conversation that exists on this computer,
identify it by its conversation/session id, sanitize and redact it for public
release, ingest it through the OpenAgents API, and view the result at
`https://openagents.com/trace/{uuid}`. Make this repeatable and scalable through
a simple CLI and skill.

## Result

A public trace ingest already exists. This change does not add a server route.
It adds the missing client: a CLI that resolves a local conversation id,
converts it to ATIF-v1.7, redacts it, checks it against the public-safety
tripwire, and posts it to the existing `POST /api/traces` endpoint as a public
trace.

- CLI: `apps/qa-runner/src/ingest-conversation-cli.ts`
  (`pnpm --dir apps/qa-runner trace:ingest <id>`).
- Skill: `.agents/skills/trace-ingest/SKILL.md` (repo scope).

## How traces and ATIF work today

### ATIF schema (one canonical source)

`packages/atif` (`@openagentsinc/atif`) is the single home for the Agent
Trajectory Interchange Format.

- `@openagentsinc/atif/trace` (`src/trace-schema.ts`) is the strict,
  pinned public-safe schema. `ATIF_PINNED_SCHEMA_VERSION = "ATIF-v1.7"`.
  `AtifTrajectory` = `{ schema_version, trajectory_id, session_id?,
visibility?, agent, steps[], final_metrics? }`. A step is
  `{ step_id, source: "user"|"agent"|"system", message, reasoning_content?,
model_name?, tool_calls?, observation?, metrics? }`.
- `validateAtifTrajectory` enforces structural rules: at least one step,
  `step_id` sequential from 1, every observation `source_call_id` references a
  tool_call in the same trajectory, and agent-only fields
  (`reasoning_content`/`tool_calls`) appear only on `agent` steps.
- `atifTraceTripwire` scans the serialized trajectory for material that must
  never be stored: secrets/tokens, wallet or payment material, local filesystem
  paths (`/Users/`, `/home/`, `file://`), and email addresses. Model ids are
  allowed (a trace legitimately names the model it ran on).
- `@openagentsinc/atif/redaction` is the redactor engine (`redactValue` /
  `TraceRedactor`) with 25+ categories (home_path, file_url, email, phone,
  bearer, provider_key, aws/google/slack/github keys, mnemonic, wallet, ip,
  long_blob, username, and more). It SCRUBS. The tripwire REJECTS. The two form
  a belt-and-suspenders safety model.

### Ingest API

`apps/openagents.com/workers/api/src/trace-store-routes.ts`
(`makeTraceStoreRoutes`), wired in `index.ts` and `worker-routes.ts`.

- `POST /api/traces` (and alias `/api/traces/upload`) ingests one trajectory.
  It accepts EITHER a registered agent bearer token (`oa_agent_…`) OR a
  signed-in browser session. It requires an `Idempotency-Key` header, caps the
  body at 8 MB and the trajectory at 2000 steps, runs the structural validator
  and the tripwire (422 on a leaky payload, returning finding CODES only),
  applies a per-owner rate limit (120/hour) and content-digest dedup, and
  returns `{ uuid, url: "/trace/{uuid}", visibility, replay }`.
- Visibility is `body.visibility ?? trajectory.visibility ?? "unlisted"`. A
  `public` or `unlisted` trace reads with no auth. `owner_only` requires the
  owning session, an admin, or the owner's read-scope token.
- `GET /api/traces/{uuid}` returns the public-safe projection the viewer
  renders. `PATCH` updates visibility. `POST|GET …/blob/{r2Key}` handle media.

### Storage

`agent_traces` uses the SQLite/D1 store in `trace-store-d1.ts`. Large trajectories offload
to R2 with a pointer in the row). Columns include `trace_uuid`, `owner_user_id`,
`agent_ref`, `schema_version`, `trajectory_id`, `session_id`,
`visibility ("public"|"unlisted"|"owner_only")`, `step_count`, `trajectory_json`
/ `trajectory_r2_key`, `blob_refs_json`, `idempotency_key`, data-market columns
(`training_consent`, `license`, `content_digest`, `upload_source`, inert reward
markers), and demand attribution (`demand_kind`, `demand_source`).

### Viewer

`apps/openagents.com/apps/start/src/routes/trace/$traceUuid.tsx` (restored
2026-07-18) renders `TracePage`, which fetches `GET /api/traces/{uuid}` and
draws the header, verdict, metadata, and a timeline of steps / tool calls /
observations, plus any media blobs. A `public` trace needs no auth.

### Existing producers

`apps/qa-runner` already contains the converters and transport this CLI reuses:
`claude-code-to-atif.ts` (Claude Code `.jsonl` → ATIF), `codex-to-atif.ts`
(Codex rollout → ATIF), and `publish-trace.ts` (redact → `POST /api/traces`
with bearer + idempotency key, returns `{ uuid, url }`). The Pylon/Codex turn
ingest path (`pylon-codex-turn-ingest-routes.ts`) stores `owner_only` traces the
same way for delegated coding work.

## Where local conversations live

| Source             | id form         | resolution                                                                                 |
| ------------------ | --------------- | ------------------------------------------------------------------------------------------ |
| Claude Code        | v4 UUID         | `~/.claude/projects/<slug>/<id>.jsonl` (one file per session)                              |
| Codex              | UUIDv7          | `~/.codex/sessions/YYYY/MM/DD/rollout-*-<id>.jsonl` (one file per session)                 |
| OpenAgents Desktop | upper-case UUID | array element in `~/Library/Application Support/<Profile>/KhalaDesktop/conversations.json` |
| Full Auto host thread | UUID (`threadRef`) | a thread in `<userData>/threads.json` — pass `--source openagents --user-data <userData>`. That store is probed first. The `threads.json` `{version, threads:[{id, title, notes}]}` shape is adapted notes→messages before the same converter runs |

Claude and Codex store one transcript file per id, so their converters are
high-fidelity. OpenAgents Desktop conversations are elements inside a shared
JSON file. OpenAgents coding sessions themselves execute through Codex/Claude
workers, whose per-session logs convert with more detail.

## What this change adds

All new files are in `apps/qa-runner/src`:

- `conversation-source.ts` — resolve an id (optionally a forced `--source`) to
  its on-disk source. It is read-only. It walks `~/.claude`, `~/.codex`, and the desktop
  support dirs, then auto-detects claude → codex → openagents.
- `openagents-conversation-to-atif.ts` — the one missing converter: a desktop
  conversation object → a valid ATIF-v1.7 trajectory. It is defensive, and empty
  conversations yield one explanatory system step).
- `ingest-conversation.ts` — orchestrator: resolve → dispatch to the matching
  converter → return the trajectory. Also `capTrajectorySteps` (keep a valid
  prefix under the 2000-step server cap) and `INGEST_MAX_STEPS`.
- `ingest-conversation-cli.ts` — the CLI: parse args, build, deep-redact,
  run the structural validator and tripwire as a local preflight, then either
  `--dry-run` (write/print the redacted ATIF) or publish through
  `publishTrace`. Default visibility is `public`.
- Tests: `openagents-conversation-to-atif.test.ts`,
  `ingest-conversation.test.ts` (source resolution, per-source builds, the cap,
  and the dry-run CLI path). Added to the package test list.

Package wiring: new `./ingest-conversation*`, `./conversation-source`,
`./openagents-conversation-to-atif`, `./claude-code-to-atif` exports and a
`trace:ingest` script in `apps/qa-runner/package.json`.

## Use

```sh
cd apps/qa-runner
# Dry-run first — build, redact, validate; no upload:
pnpm trace:ingest <conversationId> --dry-run --out /tmp/trace.json --json
# Publish as a public trace:
export OPENAGENTS_AGENT_TOKEN=oa_agent_...
pnpm trace:ingest <conversationId>
# -> https://openagents.com/trace/{uuid}
```

Flags: `--source`, `--visibility`, `--dry-run`, `--out`, `--max-steps`,
`--agent-name`, `--model`, `--base-url`, `--token`, `--json`. See the skill
`.agents/skills/trace-ingest/SKILL.md`.

## Safety

The CLI redacts with the same `@openagentsinc/atif` redactor the ingest API
trusts, then runs the tripwire locally and refuses to upload if anything leaky
survives. The server redacts and tripwires again and rejects unsafe payloads
(422). Publishes are idempotent (the key is a digest of the redacted
trajectory). A public trace is evidence only: it grants no accepted-work,
payout, settlement, or public-claim authority. Never print or commit the agent
token. Prefer `OPENAGENTS_AGENT_TOKEN` in the environment.

## Verification

- `pnpm --dir apps/qa-runner exec tsc --noEmit -p tsconfig.json`: the new files
  typecheck clean.
- `pnpm --dir apps/qa-runner exec vp test --run
src/openagents-conversation-to-atif.test.ts src/ingest-conversation.test.ts`:
  14 tests pass.
- Real Claude session (2.7 MB, 303 steps): 1070 `/Users/` occurrences in the
  raw log. The redactor scrubbed 557 items (452 home_path, 13 email, 13 bearer,
  4 provider_key, 2 mnemonic, and more). The written trajectory has zero
  `/Users/`, is structurally valid, and passes the tripwire.
- Real Codex session (2790 steps): capped to the first 2000 steps with a
  truncation note. Dry-run validates and passes the tripwire.

## Limitations and follow-ups

- Uploads were verified through the dry-run + local-preflight path (redaction,
  validation, tripwire, capping, idempotency). A live `POST /api/traces` needs a
  valid `OPENAGENTS_AGENT_TOKEN`. The transport is the proven `publish-trace.ts`
  path already used by qa-runner.
- OpenAgents Desktop conversations are best-effort: the desktop `messages` shape
  has varied and can be empty. Prefer the underlying Codex/Claude session id for
  full fidelity.
- Very large Codex rollouts (hundreds of MB) can exceed the V8 string limit
  during conversion. Use `--max-steps` or skip them.
- Truncation keeps a prefix (setup + early work). A future option could keep a
  windowed middle/tail while preserving observation→tool_call references.
- The CLI lives in `apps/qa-runner` because the converters, redactor, and
  transport already live there. A standalone `oa-trace` bin could be extracted
  later if the surface deserves its own package.
