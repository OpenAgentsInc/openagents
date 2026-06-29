# Hosted Gemini Production Receipt Audit

Issue: #7017
Date: 2026-06-29
Promise: `api.hosted_gemini.v1`

## Result

This audit does not clear the hosted Gemini production-receipt blocker and does
not authorize a green promise transition.

Two production requests were run through the public/authenticated
OpenAI-compatible gateway with a live agent bearer token. Only public-safe
metadata is recorded here; raw bearer material, prompts, response text, provider
payloads, and private account data are omitted.

## Public-Safe Receipt Observations

### `openagents/khala` authenticated request

- Path: `POST https://openagents.com/api/v1/chat/completions`
- Requested model: `openagents/khala`
- HTTP result: `200`
- Request id: `chatcmpl_8a86727990eb4f2bbea5be8f2088defd`
- Served model: `accounts/fireworks/models/deepseek-v4-flash`
- Supply lane / worker: `fireworks`
- Usage: prompt `835`, completion `16`, total `851`
- Billing: `mode=no_debit`, `reason=operator_exempt_or_unmetered`,
  `receipt_required=false`
- Verification: `none`

This proves the authenticated production gateway returned a successful
OpenAI-compatible response with usage metadata, but it is not hosted Gemini and
it is not a debit/paid receipt.

### `gemini-3.5-flash` authenticated request

- Path: `POST https://openagents.com/api/v1/chat/completions`
- Requested model: `gemini-3.5-flash`
- HTTP result: JSON error body
- Error: `model_unavailable`

This confirms the raw hosted Gemini model id is not currently available as a
positive public production receipt through this endpoint.

## Promise Decision

`api.hosted_gemini.v1` stays `yellow`.

The route-level harness still covers the env-gated Vertex Gemini binding and
registered-agent metering path in source, but the live production audit above
did not produce a hosted Gemini production receipt. The current blockers remain:

- `blocker.product_promises.hosted_gemini_production_receipt_pending`
- `blocker.product_promises.hosted_gemini_owner_upgrade_signoff_pending`

Green still requires a fresh production Hosted Gemini success receipt that
dereferences from the promise evidence set and an owner-signed transition
receipt recorded through the product-promise transition path.

