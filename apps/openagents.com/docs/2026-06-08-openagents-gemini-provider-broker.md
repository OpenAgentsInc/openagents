# OpenAgents product surface Gemini Provider Broker

OpenAgents product surface supports hosted Gemini text inference for Probe through a protected
provider-account broker route.

## Runtime Contract

Probe may call:

```txt
POST /api/provider-accounts/google-gemini/models/<model>:streamGenerateContent?alt=sse
Authorization: Bearer <OpenAgents programmatic agent token>
Content-Type: application/json
```

The Worker reads `env.GEMINI_API_KEY`, forwards the request to:

```txt
https://generativelanguage.googleapis.com/v1beta/models/<model>:streamGenerateContent?alt=sse
```

and streams the provider response back with `cache-control: no-store`.

The route is not public. It requires the same programmatic-agent bearer auth
used by provider-account service routes. Do not add an unauthenticated fallback.

## Token Usage Recording

The broker clones the upstream Gemini response and records `usageMetadata` into
the canonical `token_usage_events` ledger in `ctx.waitUntil`. The response body
stream sent to Probe remains the provider stream.

Recorded events use:

- `producerSystem: "openagents"`;
- `sourceRoute: "openagents_provider_broker"`;
- `provider: "google_gemini"`;
- `backendProfile: "worker_secret_gemini_api_key"`;
- `usageTruth: "exact"` when Gemini supplies `usageMetadata`;
- actor user id from the authenticated programmatic agent; and
- safe metadata containing only provider HTTP status and succeeded/failed
  status.

Gemini `cachedContentTokenCount` is stored as cache-read tokens and subtracted
from prompt tokens before `inputTokens` is written, matching the shared token
extractor contract. `thoughtsTokenCount` becomes `reasoningTokens`.

The broker records both successful provider responses and provider failures
when the upstream response includes `usageMetadata`. It does not store the
Gemini API key, request body, prompt, response body, provider payload, or tool
arguments.

Probe should send an `Idempotency-Key` header for hosted Gemini calls. OpenAgents product surface
uses that key, the authenticated actor, and the model to derive the canonical
event id and idempotency key. If the header is absent, OpenAgents product surface falls back to a
hash of the request body for retry dedupe without persisting the body.

## Grant Contract

Probe can resolve a redacted Gemini grant at:

```txt
POST /api/provider-accounts/google-gemini/grants/resolve
Authorization: Bearer <OpenAgents programmatic agent token>
Content-Type: application/json
```

The response uses:

- `provider: "google_gemini"`;
- `providerSecretRef: "provider-account://google-gemini/worker-secret/GEMINI_API_KEY"`;
- `materialization.kind: "probe_gemini_api_key"`;
- `materialization.target.name: "GOOGLE_GENERATIVE_AI_API_KEY"`.

The grant response is a public-safe reference contract. It does not contain the
raw Gemini API key. Live hosted inference still goes through the broker route.

## Secret Boundary

`GEMINI_API_KEY` must remain a Cloudflare Worker secret. Do not store Gemini
keys in D1 rows, public sync projections, issue comments, docs, logs, or
browser-visible payloads. API keys should be restricted to
`generativelanguage.googleapis.com`.

## Probe Fallback

Probe uses local Gemini keys first. If no local Gemini key is set, Probe can use
OpenAgents product surface by setting:

```sh
PROBE_OPENAGENTS_BASE_URL=https://openagents.com
PROBE_OPENAGENTS_BEARER_TOKEN=<OpenAgents programmatic agent token>
```

The Probe Gemini client then uses the OpenAgents product surface broker as its Gemini base URL and
sends the bearer token to OpenAgents product surface instead of sending `x-goog-api-key`.

Direct/local Probe Gemini calls remain Probe's responsibility to ledger through
the canonical token usage ingestion API. OpenAgents product surface can only record calls that pass
through the hosted broker route.
