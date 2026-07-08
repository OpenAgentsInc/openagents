# Autopilot Token Accounting And Leaderboards

Date: 2026-06-03

## Summary

Autopilot now keeps a first-party token usage ledger in Cloudflare D1 and
builds token leaderboards from that ledger.

The accounting source is the Autopilot runner callback stream:

```text
SHC Autopilot runner
  -> POST /api/omni/agent-runs/:runId/events/ingest
  -> agent_run_events
  -> autopilot_token_usage
  -> /api/autopilot/token-leaderboards
  -> logged-in OpenAgents dashboard
```

This keeps token accounting attached to the durable run/event record instead of
making the dashboard infer usage from UI state.

As of 2026-06-08, OpenAgents product surface also has a canonical cross-system token usage event
ledger for Probe, OpenAgents product surface-hosted provider brokerage, and future Stats dashboard
rollups:

```text
Trusted producer
  -> POST /api/stats/token-usage/events
  -> token_usage_events
  -> GET /api/stats/token-usage/aggregate
  -> Stats dashboard totals, drilldowns, and leaderboards
```

The older `autopilot_token_usage` table remains the run-callback ledger for
Autopilot mission events. The new `token_usage_events` table is the broader
Stats source of truth for usage that may originate outside an Autopilot run,
including direct Probe provider calls and OpenAgents product surface provider-broker calls.

## OpenCode Usage Shape

The OpenCode reference reports usage in two closely related forms.

OpenCode session projection tracks per-session totals from `step-finish` parts:

```ts
{
  cost: number
  tokens: {
    input: number
    output: number
    reasoning: number
    cache: {
      read: number
      write: number
    }
  }
}
```

OpenCode's inference/stat pipeline uses the analytics names:

```text
tokens_input
tokens_output
tokens_reasoning
tokens_cache_read
tokens_cache_write_5m
tokens_cache_write_1h
```

Autopilot mirrors that vocabulary in D1:

```text
input_tokens
output_tokens
reasoning_tokens
cache_read_tokens
cache_write_5m_tokens
cache_write_1h_tokens
total_tokens
```

## Total Token Definition

`total_tokens` is the provider-reported aggregate when the runner event includes
one. That is the authoritative value for billing and ranking.

When a payload only exposes buckets, Autopilot computes a fallback total from
all known generation buckets:

```text
input_tokens
+ output_tokens
+ reasoning_tokens
+ cache_read_tokens
+ cache_write_5m_tokens
+ cache_write_1h_tokens
```

For Codex and OpenAI-style payloads where cached input is a detail/subset of
the input bucket (`cached_input_tokens`, `input_tokens_details.cached_tokens`,
or `prompt_tokens_details.cached_tokens`), cache read is still stored
separately but is not added a second time to the fallback total. For Gemini
`usageMetadata`, `cachedContentTokenCount` is split out of `promptTokenCount`
before the fallback total is calculated.

## Accepted Runner Payloads

The extractor accepts the current expected shapes:

```json
{
  "type": "runner.usage",
  "provider": "openai",
  "model": "gpt-5-codex",
  "tokens": {
    "input": 100,
    "output": 50,
    "reasoning": 20,
    "cache_read": 30,
    "cache_write_5m": 10,
    "cache_write_1h": 40
  }
}
```

```json
{
  "part": {
    "type": "step-finish",
    "cost": 0.01,
    "tokens": {
      "input": 100,
      "output": 50,
      "reasoning": 20,
      "cache": { "read": 30, "write": 40 }
    }
  }
}
```

It also accepts OpenAI-compatible usage:

```json
{
  "response": {
    "usage": {
      "prompt_tokens": 100,
      "completion_tokens": 50,
      "total_tokens": 150,
      "prompt_tokens_details": { "cached_tokens": 20 },
      "completion_tokens_details": { "reasoning_tokens": 10 }
    }
  }
}
```

It also accepts Codex JSONL usage from `codex exec`:

```json
{
  "type": "turn.completed",
  "usage": {
    "input_tokens": 20,
    "cached_input_tokens": 5,
    "output_tokens": 8,
    "reasoning_output_tokens": 2
  }
}
```

The SHC callback currently wraps many Codex JSONL events inside a `runner.log`
payload:

```json
{
  "type": "runner.log",
  "dataJson": "{\"detail\":\"stdout: {\\\"type\\\":\\\"turn.completed\\\",\\\"usage\\\":{...}}\"}"
}
```

The extractor unwraps `dataJson` and `stdout:` before reading usage.

It also accepts Codex app-server token notifications:

```json
{
  "type": "thread.token_usage.updated",
  "tokenUsage": {
    "last": {
      "totalTokens": 30,
      "inputTokens": 20,
      "cachedInputTokens": 5,
      "outputTokens": 8,
      "reasoningOutputTokens": 2
    },
    "total": {
      "totalTokens": 300,
      "inputTokens": 200,
      "cachedInputTokens": 50,
      "outputTokens": 80,
      "reasoningOutputTokens": 20
    }
  }
}
```

When both `last` and cumulative `total` are present, Autopilot ledgers `last`.
This prevents repeated app-server notifications from inflating per-run totals.

Other accepted provider shapes include:

- Anthropic `usage.input_tokens`, `usage.output_tokens`,
  `usage.cache_read_input_tokens`, and `usage.cache_creation.*`.
- Gemini `usageMetadata.promptTokenCount`,
  `usageMetadata.candidatesTokenCount`, `usageMetadata.thoughtsTokenCount`,
  `usageMetadata.cachedContentTokenCount`, and
  `usageMetadata.totalTokenCount`.

## Idempotency

Each usage row has:

```text
UNIQUE(run_id, source_ref)
```

`source_ref` is:

```text
external_event_id if the runner supplied one
otherwise run_id:sequence
```

That means SHC can retry a callback without inflating totals. The event table
already ignores duplicate run/sequence rows; the token ledger now follows the
same retry boundary.

The canonical `token_usage_events` ledger uses a wider retry boundary:

```text
id TEXT PRIMARY KEY
idempotency_key TEXT NOT NULL UNIQUE
```

Trusted producers must send the same `eventId` and `idempotencyKey` when
retrying the same observed usage event. Repeated submissions return the stored
event with `inserted: false` and do not inflate totals.

## Canonical Event Contract

Canonical event ingestion accepts:

```json
{
  "schemaVersion": "openagents.token_usage_event.v1",
  "eventId": "token_event_probe_20260608_1",
  "idempotencyKey": "probe:run:attempt:usage",
  "observedAt": "2026-06-08T12:00:00.000Z",
  "producerSystem": "probe",
  "sourceRoute": "probe_direct_provider",
  "actor": {
    "userId": "user_chris",
    "teamId": "team_openagents_core"
  },
  "sourceRefs": {
    "anonymizedSourceRef": "probe-session-hash:abc123",
    "runRef": "probe-run:artanis-gepa-1"
  },
  "provider": "google_gemini",
  "model": "gemini-2.5-pro",
  "backendProfile": "direct_api",
  "tokenCounts": {
    "inputTokens": 100,
    "outputTokens": 40,
    "reasoningTokens": 15,
    "cacheReadTokens": 25,
    "cacheWrite5mTokens": 0,
    "cacheWrite1hTokens": 0,
    "totalTokens": 180
  },
  "usageTruth": "exact",
  "privacy": {
    "leaderboardEligible": true,
    "privacyOptOut": false
  },
  "safeMetadata": {
    "providerRequestStatus": "succeeded"
  }
}
```

Accepted producer systems are `probe`, `openagents`, `provider_broker`,
`shc_runner`, `manual`, and `unknown`.

Accepted source routes are `probe_direct_provider`, `probe_local_model`,
`openagents_provider_broker`, `openagents_hosted_gemini`, `shc_runner_callback`,
`manual`, and `unknown`.

`usageTruth` is `exact`, `estimated`, or `unknown`. Exact means the provider
or runtime reported the bucket counts. Estimated means OpenAgents product surface derived them from
available runtime counters. Unknown is allowed only when the event still needs
to preserve a durable missing-usage signal without pretending exactness.

The ingestion boundary rejects raw prompts, completions, provider payloads, API
keys, bearer/callback/OAuth material, tool args, raw source, private repo
paths, local filesystem paths, and customer/private material before writing to
D1.

## OpenAgents product surface-Hosted Gemini

OpenAgents product surface-hosted Gemini calls through:

```text
POST /api/provider-accounts/google-gemini/models/<model>:streamGenerateContent?alt=sse
```

record canonical token usage when the upstream Gemini response includes
`usageMetadata`. OpenAgents product surface writes `producerSystem: "openagents"` and
`sourceRoute: "openagents_provider_broker"`, maps Gemini token buckets exactly, and
stores only safe request/response metadata. Successful provider responses and
provider failures are both eligible for recording when `usageMetadata` exists.

Probe calls made directly with a local Gemini key do not pass through OpenAgents product surface and
must be submitted by Probe through `POST /api/stats/token-usage/events` if they
should appear in the OpenAgents product surface Stats dashboard. Hosted OpenAgents product surface broker calls are
recorded by OpenAgents product surface itself.

## API

Authenticated browser users and programmatic agents can read leaderboards:

```text
GET /api/autopilot/token-leaderboards
```

The endpoint returns:

```ts
type AutopilotTokenLeaderboardsResponse = {
  authenticated: true
  actor: {
    kind: 'human' | 'agent'
    userId: string
    // human and agent metadata fields vary by actor kind
  }
  leaderboards: {
    generatedAt: string
    global: TokenUsageTotals
    currentUser: TokenUsageTotals
    teams: TokenLeaderboardTeam[]
    users: TokenLeaderboardUser[]
    currentUserTeams: TokenLeaderboardTeam[]
    missingUsageSignals: number
    recentRuns: TokenUsageRunSummary[]
  }
}
```

All rows use the same totals object:

```ts
type TokenUsageTotals = {
  inputTokens: number
  outputTokens: number
  reasoningTokens: number
  cacheReadTokens: number
  cacheWrite5mTokens: number
  cacheWrite1hTokens: number
  totalTokens: number
  usageEvents: number
}
```

Unauthenticated calls return `401`.

Trusted Probe/OpenAgents product surface producers ingest canonical events with the existing admin
bearer token:

```text
POST /api/stats/token-usage/events
Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN
```

Admin browser sessions can read aggregate Stats drilldowns:

```text
GET /api/stats/token-usage/aggregate
GET /api/stats/token-usage/aggregate?since=2026-06-08T00:00:00.000Z
GET /api/stats/token-usage/aggregate?since=...&until=...
GET /api/stats/token-usage/aggregate?provider=google_gemini&model=gemini-3.5-flash
GET /api/stats/token-usage/aggregate?producerSystem=openagents&sourceRoute=openagents_provider_broker
GET /api/stats/token-usage/aggregate?actorUserId=...&actorTeamId=...
GET /api/stats/token-usage/aggregate?leaderboardEligible=true&usageTruth=exact
```

Supported query filters are:

- `since` and `until` ISO-compatible observed-at timestamps
- `provider` and `model`
- `producerSystem`
- `sourceRoute`
- `actorUserId`, `actorTeamId`, and `accountRef`
- `leaderboardEligible`
- `privacyOptOut`
- `usageTruth`

The aggregate response includes:

- global totals;
- provider/model breakdowns;
- producer/source-route breakdowns;
- actor/team/account drilldowns with anonymous rows preserved;
- exact/estimated/unknown usage-truth buckets;
- safe source-reference drilldowns for anonymized source, repository, run,
  session, and task refs;
- recent safe event metadata; and
- the filter window used for the query.

Opted-out events remain in global totals. Leaderboard projections must exclude
or anonymize them rather than deleting them from accounting.

Admin browser sessions can read opt-out-aware canonical leaderboards:

```text
GET /api/stats/token-usage/leaderboards
GET /api/stats/token-usage/leaderboards?window=today
GET /api/stats/token-usage/leaderboards?window=7d
GET /api/stats/token-usage/leaderboards?window=30d
GET /api/stats/token-usage/leaderboards?window=all
```

The response includes:

- `globalTotals`, which always includes eligible, opted-out, and anonymous
  usage for accounting completeness
- `anonymousTotals`, which groups anonymous, privacy opted-out, and
  leaderboard-excluded events without implying identity
- `topUsers`, excluding anonymous events, `privacyOptOut` events,
  `leaderboardEligible: false` events, and users with a private/opted-out
  preference
- `topTeams`, excluding opted-out/private users, teams, and accounts
- `topRuns` and `topProjects`, using safe refs only and excluding
  opted-out/private identities
- `topProviderModels`, which is identity-independent and includes global usage
  by provider/model

Signed-in users can read or update their own leaderboard preference:

```text
GET /api/stats/token-usage/leaderboard-preference
PUT /api/stats/token-usage/leaderboard-preference
```

The update body is:

```json
{
  "leaderboardParticipation": "eligible",
  "leaderboardVisibility": "internal"
}
```

Use `leaderboardParticipation: "opted_out"` and
`leaderboardVisibility: "private"` to disable identified leaderboard
participation. This does not stop token accounting and does not remove prior
events from global totals.

## Stats Page

Admin users can inspect the canonical ledger at:

```text
/stats
```

The page loads `GET /api/stats/token-usage/aggregate` and renders:

- global total, input, output, reasoning/thought, cache-read, and cache-write
  token counts
- estimated-event and unknown-event counts from `usageTruth`
- provider/model, source-route, usage-truth, actor, and safe source-reference
  breakdown tables
- recent event rows with observed time, provider/model, source route, actor or
  anonymous label, privacy/leaderboard status, safe source refs, and safe
  metadata snippets
- filters for date range, provider/model, source system, source route, actor
  user/team, usage truth, leaderboard eligibility, and leaderboard window
- opt-out-aware top users, teams, runs, projects, and provider/model rankings
- the current user's leaderboard participation/visibility preference
- empty, loading, and error states for operators

The page must not render raw prompts, completions, provider payloads, source
code, private repository paths, API keys, bearer tokens, callback/OAuth
material, or provider secrets. Event rows use normalized ledger fields plus
safe primitive metadata only; anonymous or privacy opted-out rows are labeled
`Anonymous/anonymized source`.

## Usage Page

Logged-in users now have a dedicated `/usage` page. It shows:

- current user totals
- global totals
- input/output/reasoning/cache breakdowns
- recent mission usage rows
- top teams by `totalTokens`
- top people by `totalTokens`
- `missingUsageSignals`, the count of runner events where SHC explicitly
  emitted `usage.unavailable`

The sidebar `Usage` item and footer `Tokens` row show the current user's compact
token total.

## Operational Notes

Apply the D1 migration before expecting live runner callbacks to persist usage:

```bash
cd workers/api
bunx wrangler d1 migrations apply openagents-autopilot --remote
```

Then deploy:

```bash
bun run --cwd workers/api deploy
```

Smoke checks:

```bash
curl -i https://openagents.com/api/autopilot/token-leaderboards
```

Without a browser session this should return `401`. In an authenticated browser,
the same endpoint should return the JSON leaderboard.

To test ingestion manually with a runner callback token, post a runner event
with a stable `externalEventId` and usage payload to:

```text
POST /api/omni/agent-runs/:runId/events/ingest
```

Then refresh the leaderboard endpoint. Reposting the same `externalEventId`
should not change totals.

## Current Limitations

- Historical runs without token-bearing events remain at zero until backfilled.
- Production SHC runs have emitted `usage.unavailable` events for
  subscription-backed Codex sessions. OpenAgents product surface now treats that as missing producer
  telemetry, not as an acceptable accounting state. Accurate counts require SHC
  to forward either Codex `turn.completed.usage` JSONL or Codex app-server
  `ThreadTokenUsageUpdated` payloads.
- Billing now records a coarse Codex debit from token totals in
  `billing_ledger_entries`. OpenCode's richer provider/model cost fields still
  need a real price table before we should treat displayed costs as final.
- Deployment token usage is not counted yet. This ledger is scoped to Autopilot
  agent runs.
- Team attribution uses `agent_runs.team_id`. Runs without a team still count
  globally and per user, but they do not appear in a team leaderboard.

## Recommended Next Step

Update the SHC runner to always emit a final token event after each Autopilot
run:

```json
{
  "externalEventId": "shc-run-id:usage-final",
  "sequence": 9000,
  "type": "runner.usage",
  "source": "shc",
  "summary": "Autopilot token usage finalized.",
  "status": "completed",
  "provider": "openai",
  "model": "gpt-5-codex",
  "tokens": {
    "input": 0,
    "output": 0,
    "reasoning": 0,
    "cache_read": 0,
    "cache_write_5m": 0,
    "cache_write_1h": 0
  }
}
```

If SHC also streams incremental step-finish events, use a unique
`externalEventId` per step and do not send a duplicate final aggregate unless
the final event is explicitly an aggregate replacing the step events.
