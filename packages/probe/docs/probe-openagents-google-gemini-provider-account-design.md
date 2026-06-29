# Probe/OpenAgents product surface Google Gemini Provider Account Design

Date: 2026-06-08

Status: Probe-side provider schema, grant validation, and env materialization
implemented. OpenAgents product surface provider-account storage/grant issuance remains a follow-up.

## Goal

Add a later managed-key path where OpenAgents product surface can lease a Google Gemini provider
account to an authorized Probe runner. This must not block or replace the
local direct API-key path:

- `GOOGLE_GENERATIVE_AI_API_KEY`
- `GEMINI_API_KEY`
- OpenAgents product surface Cloudflare Worker secret `GEMINI_API_KEY` for OpenAgents product surface-owned server code

The managed path should materialize a scoped Gemini API key into a Probe run
only after OpenAgents product surface grants a specific assignment to a specific runner session.

## Boundary

OpenAgents product surface owns:

- provider account CRUD and operator UI;
- encrypted secret storage for raw Gemini API keys;
- Google key inventory metadata, rotation state, and health checks;
- grant issuance and one-time grant resolution endpoints;
- public/operator-safe provider account projections;
- D1 migrations and backfill from existing ad hoc Gemini config, if any;
- audit/operator receipts for key addition, rotation, revocation, and grant
  issuance.

Probe owns:

- provider enum/schema acceptance for `google_gemini`;
- decoding OpenAgents product surface grant responses for Gemini;
- runner authorization checks against assignment refs and runner proof;
- per-run materialization into `GOOGLE_GENERATIVE_AI_API_KEY`;
- redacted materialized/scrubbed receipts;
- scrub-on-closeout and failure cleanup;
- direct Gemini runtime use after the env key is materialized.

Probe must not become the durable store for raw Gemini keys. OpenAgents product surface must not send
raw key material in public assignment projections, issue comments, fixtures, or
receipts.

## Provider Contract

Add a provider id:

```ts
export const GOOGLE_GEMINI_PROVIDER = "google_gemini" as const
```

Extend provider schemas from a single ChatGPT literal to a provider union:

```ts
Provider = "chatgpt_codex" | "google_gemini"
```

Provider account public projections can reuse the existing ref-first shape:

```json
{
  "provider": "google_gemini",
  "providerAccountRef": "provider-account_google_gemini_primary",
  "authMode": "manual_secret_ref",
  "status": "connected",
  "health": "healthy",
  "secretRef": "cloud-secret://openagents/google-gemini/primary",
  "accountLabel": "Primary Gemini",
  "metadata": {
    "projectRef": "gcp-project.openagentsgemini",
    "allowedServices": ["generativelanguage.googleapis.com"],
    "defaultModel": "gemini-3.5-flash",
    "rotationRef": "rotation.google_gemini.primary.2026_06"
  }
}
```

Rules:

- `secretRef` is a hosted secret reference only.
- `metadata` can include public refs and safe labels only.
- Raw API keys, Google bearer tokens, service account keys, refresh tokens,
  request headers, prompts, and provider payloads remain forbidden.
- `canIssueProviderAccountGrant` should allow `google_gemini` only when the
  account is connected, healthy, and has a public secret ref.

## Grant Resolution

OpenAgents product surface should add a provider-specific or provider-generic route that returns the
same grant envelope pattern Probe already uses for ChatGPT/Codex:

```json
{
  "grantRef": "provider-auth-grant_google_gemini_1",
  "provider": "google_gemini",
  "providerAccountRef": "provider-account_google_gemini_primary",
  "providerSecretRef": "cloud-secret://openagents/google-gemini/primary",
  "runnerSessionId": "runner_session_1",
  "expiresAt": "2026-06-08T15:00:00.000Z",
  "status": "used",
  "materialization": {
    "kind": "probe_gemini_api_key",
    "provider": "google_gemini",
    "providerSecretRef": "cloud-secret://openagents/google-gemini/primary",
    "target": {
      "kind": "env",
      "name": "GOOGLE_GENERATIVE_AI_API_KEY"
    },
    "homeIsolation": "per_run",
    "scrubAfterCloseout": true
  }
}
```

Probe should reject:

- materialization into `GEMINI_API_KEY` when OpenAgents product surface intended
  `GOOGLE_GENERATIVE_AI_API_KEY`;
- OpenCode-specific env names;
- raw key content in the grant envelope;
- grants whose provider account, grant ref, runner session, or assignment proof
  do not match.

`GEMINI_API_KEY` remains accepted for local BYO env setup, but OpenAgents product surface-managed
materialization should prefer `GOOGLE_GENERATIVE_AI_API_KEY` so the managed path
is distinguishable in receipts and runner state.

## Runner Authorization

Assignments that use OpenAgents product surface-managed Gemini keys should carry:

```json
{
  "provider": "google_gemini",
  "providerAccountRef": "provider-account_google_gemini_primary",
  "authGrantRef": "provider-auth-grant_google_gemini_1",
  "backend": {
    "kind": "gemini_api",
    "backendProfileId": "gemini-api"
  }
}
```

Required runner capabilities:

- `probe.run`
- `openagents.grant.resolve`
- `probe.backend.gemini_api`

Local Gemini assignments that only select `backend.kind: "gemini_api"` and do
not carry provider refs keep the current behavior: they require
`probe.backend.gemini_api` but not `openagents.grant.resolve`.

## Receipts

Probe receipts should include:

- provider id;
- provider account ref;
- provider secret ref;
- grant ref;
- target env name;
- materialized/scrubbed timestamps;
- backend profile id;
- `contentRedacted: true`.

Probe receipts must not include:

- raw Gemini API key;
- request headers;
- raw prompts;
- tool inputs;
- provider payloads;
- Google project secrets or key strings.

OpenAgents product surface receipts should similarly store refs, health status, rotation refs, and
redacted key status only.

## Migration And Backfill

Probe needs no data migration; it only changes schemas and materialization
logic.

OpenAgents product surface likely needs:

- provider enum migration for `google_gemini`;
- optional seed/backfill for the existing production Gemini Worker secret as a
  provider account record;
- operator UI/API for adding, rotating, disabling, and health-checking Gemini
  basic API keys;
- a grant route for Gemini or a provider-generic grant route;
- tests proving public projections and receipts reject raw key material.

Backfill should store only a secret binding ref and safe metadata. It must not
copy the raw Cloudflare `GEMINI_API_KEY` value into D1.

## Follow-Up Issues

Implementation should be split:

1. Probe provider schema and materializer support for `google_gemini`:
   OpenAgentsInc/probe#199 (implemented).
2. OpenAgents product surface provider-account storage, operator surface, and grant resolution for
   Gemini basic API keys: OpenAgentsInc/openagents#526.
3. End-to-end managed Gemini assignment smoke from OpenAgents product surface grant resolution to
   Probe env materialization to Gemini backend completion:
   OpenAgentsInc/probe#200 (Probe fake E2E and opt-in live smoke implemented;
   production OpenAgents product surface route still depends on OpenAgentsInc/openagents#526).

These issues can proceed after local Gemini API-key support remains stable.
