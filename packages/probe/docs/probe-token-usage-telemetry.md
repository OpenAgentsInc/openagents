# Probe Token Usage Telemetry

Probe emits redacted token-usage events to OpenAgents product surface so the OpenAgents product surface Stats dashboard can include Probe local, direct-provider, and managed-assignment inference in global token totals and opt-out-aware leaderboards.

## Event Contract

Probe sends `openagents.token_usage_event.v1` JSON to:

`POST /api/stats/token-usage/events`

The event matches OpenAgents product surface's canonical token ledger contract:

- `producerSystem`: always `probe`.
- `sourceRoute`: `probe_direct_provider`, `probe_local_model`, or `openagents_hosted_gemini`.
- `provider` / `model` / `backendProfile`: safe provider and backend identifiers.
- `tokenCounts`: normalized input, output, reasoning, cache read, cache write, and total counts.
- `usageTruth`: `exact`, `estimated`, or `unknown`.
- `sourceRefs`: safe run/session/task refs and hashed repository scope when available.
- `safeMetadata`: Probe version, backend kind/profile, agent surface, route, and command where relevant.
- `privacy`: leaderboard eligibility and privacy opt-out flags.

Probe does not send prompts, completions, tool arguments, provider payloads, API keys, bearer tokens, callback tokens, source files, or raw repository paths.

## Usage Mapping

Gemini usage metadata is mapped through Probe's existing `ProbeLlmUsage` shape:

- `promptTokenCount` to `inputTokens`.
- `candidatesTokenCount + thoughtsTokenCount` to `outputTokens`.
- `thoughtsTokenCount` to `reasoningTokens`.
- `cachedContentTokenCount` to `cacheReadTokens`.
- `totalTokenCount` to `totalTokens`.
- Gemini reported counts are marked `exact`; missing counts are `unknown`.

Apple FM usage preserves the bridge-reported truth:

- `promptTokens` to `inputTokens`.
- `completionTokens` to `outputTokens`.
- `totalTokens` to `totalTokens`.
- `truth` is forwarded as `exact`, `estimated`, or `unknown`.

## Destination And Opt-Out

Local Probe is offline by default. Telemetry sends when either of these is set:

- `PROBE_TOKEN_USAGE_OPENAGENTS_BASE_URL`
- `PROBE_OPENAGENTS_BASE_URL`

Managed backend assignments default to `https://openagents.com` when no base URL is configured. Send failures are best-effort and do not fail the inference.

Auth uses `PROBE_TOKEN_USAGE_BEARER_TOKEN` first, then `PROBE_OPENAGENTS_BEARER_TOKEN`.

Disable sending entirely with either:

- `PROBE_TOKEN_USAGE_TELEMETRY=off`
- `PROBE_TOKEN_USAGE_TELEMETRY_DISABLED=true`
- `PROBE_TOKEN_USAGE_OPT_OUT=true`

Disable leaderboard participation while still sending aggregate telemetry with:

- `PROBE_TOKEN_USAGE_PRIVACY_OPT_OUT=true`
- `PROBE_PRIVACY_OPT_OUT=true`

Optional actor refs:

- `PROBE_TOKEN_USAGE_ACCOUNT_REF`
- `PROBE_TOKEN_USAGE_TEAM_ID`
- `PROBE_TOKEN_USAGE_USER_ID`

## Safe Source Refs

Managed assignments include:

- `runRef`: `probe.assignment.<assignmentId>`
- `sessionRef`: `probe.runner_session.<runnerSessionId>`
- `taskRef`: `probe.task.<assignmentId>`
- `repositoryRef`: `repo.commit.<commit>` when a commit is supplied, otherwise `repo.sha256.<digest>`

Repository URLs, local paths, and branch names are never sent raw. They are hashed into the repository ref when a commit is unavailable.

## Testing With A Fake OpenAgents product surface Endpoint

Use a local server or test fetch that accepts:

`POST /api/stats/token-usage/events`

Then run:

```sh
PROBE_TOKEN_USAGE_OPENAGENTS_BASE_URL=http://127.0.0.1:8787 \
GOOGLE_GENERATIVE_AI_API_KEY=... \
bun run --cwd packages/runtime probe -- backend gemini smoke
```

For local development without sending:

```sh
PROBE_TOKEN_USAGE_OPT_OUT=true bun run --cwd packages/runtime probe -- backend gemini smoke
```

Relevant tests:

```sh
bun test packages/runtime/tests/token-usage-telemetry.test.ts
bun test packages/runtime/tests/backend-assignment.test.ts -t "runs an Apple FM assignment"
bun test packages/runtime/tests/gemini-cli.test.ts -t "falls back to an authenticated OpenAgents product surface Gemini broker"
```
