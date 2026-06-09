# Effect TypeScript Schema Audit

Date: 2026-06-02

## Summary

The direct Zod dependency has been removed from `workers/api`. Runtime validation
for the Worker-owned external boundaries now uses `effect/Schema`.

Converted in this pass:

- OpenAuth `user` subject schema.
- GitHub `/user`, `/user/emails`, and OAuth token response parsing.
- ChatGPT/Codex device login, device token, and OAuth token response parsing.
- Programmatic agent registration request validation.

The Worker still has plain TypeScript record types for internal repository
rows, public DTOs, and view models. I am intentionally not converting all of
those in one sweep because most are not trust boundaries; they are local shapes
fed by typed SQL projections, rendering helpers, or test fakes. The better next
step is to move package-by-package toward shared Effect Schema contracts at the
actual system edges.

## Current State

### Effect-Native Surfaces

- `packages/sync-schema` already models sync protocol envelopes with
  `effect/Schema`.
- `packages/provider-account-schema` already models provider account public
  projections and redaction helpers with `effect/Schema`.
- `apps/web` Foldkit models, messages, routes, and session types are already
  Effect Schema-first.
- `workers/api/src/index.ts` now validates OpenAuth/GitHub external payloads
  through Effect Schema.
- `workers/api/src/provider-accounts.ts` now validates OpenAI/Codex external
  payloads through Effect Schema.
- `workers/api/src/agent-registration.ts` now validates programmatic agent
  registration through Effect Schema.

### Remaining Plain TypeScript

These are acceptable for now:

- D1 row projection types such as `ProviderAccountRow`, `AgentRunRow`, and
  `TeamChatMessage` backing SQL reads.
- Repository interfaces used for dependency injection in tests.
- UI-only rendering DTOs inside the Foldkit chat page.
- Small helper return types for dispatch, grants, and SHC run state.

These should move next:

- Worker API request bodies that still use `readJsonObject` plus manual
  `optionalString` / `optionalInteger` extraction.
- SHC dispatch and callback payloads in `workers/api/src/omni-runs.ts`.
- GitHub write connection records and grant resolution payloads in
  `workers/api/src/github-write-connections.ts`.
- Public dashboard DTOs for teams, chat, provider accounts, and mission
  history.

## Recommendation

Do not convert every local TypeScript type just because it exists. Convert
schemas at the boundaries first:

1. HTTP request bodies.
2. Third-party API responses.
3. Queue, Durable Object, and future OpenAgents Sync messages.
4. D1 JSON columns and serialized metadata.
5. Public DTOs consumed by browser UI.

Plain TypeScript can remain for private helper types and SQL row projections
until those projections cross a process, storage, or network boundary.

## Follow-Up Plan

1. Add `packages/api-schema` for Worker request/response schemas shared by
   browser, Worker tests, and future agent clients.
2. Move `omni-runs.ts` SHC payload parsing to Effect Schema.
3. Move `github-write-connections.ts` public/grant payloads to Effect Schema.
4. Replace generic `Record<string, unknown>` JSON metadata with branded
   metadata schemas per feature.
5. Add schema decode tests for malformed external API payloads.

## Result

Direct app code no longer imports Zod. Any remaining Zod in `bun.lock` is
transitive through development tooling, not part of OpenAgents application
validation.
