# Hosted Gemini production receipt audit

Date: 2026-06-29. Issue: #7017.

## Result

The production OpenAI-compatible Khala endpoint is live for the authenticated
public model path, but the direct Hosted Gemini selector is not currently a
successful production user/agent path.

Audited path:

- `POST https://openagents.com/api/v1/chat/completions`
- Auth: OpenAgents bearer token, not recorded here.
- Direct Hosted Gemini request model: `gemini-3.5-flash`
- Production result: HTTP `400`, `error: "model_unavailable"`

The public model catalog for the same token listed only `openagents/khala`.

Control request against `openagents/khala` returned HTTP `200` and a normal
OpenAI-compatible completion shape with non-empty usage:

- response object: `chat.completion`
- public response model: `openagents/khala`
- `usage.prompt_tokens`: present
- `usage.completion_tokens`: present
- `usage.total_tokens`: present
- `openagents.requested_model`: `openagents/khala`
- `openagents.served_model`: `accounts/fireworks/models/deepseek-v4-flash`
- `openagents.supply_lane`: `fireworks`
- `openagents.billing.mode`: `no_debit`

This is deliberately not a Hosted Gemini success receipt. It proves the
authenticated production gateway and public-safe usage metadata are reachable,
while the direct Hosted Gemini production receipt remains pending.

## Promise impact

`api.hosted_gemini.v1` stays yellow.

Do not clear:

- `blocker.product_promises.hosted_gemini_production_receipt_pending`
- `blocker.product_promises.hosted_gemini_owner_upgrade_signoff_pending`

Green still requires a successful production Hosted Gemini request through the
intended public/authenticated path plus owner-signed upgrade evidence. The
successful `openagents/khala` control request is not paid-credit evidence and
does not imply a Hosted Gemini lane, paid resale, settlement, or owner-approved
green claim.
