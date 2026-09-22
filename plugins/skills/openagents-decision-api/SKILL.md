---
name: openagents-decision-api
description: Authenticate to and call the OpenAgents decision API — typed decisions, batch classification, durable jobs, and the bundled documentation — through the oak CLI, the oak-mcp server, or raw HTTP. Use when a task needs a programmatic judgment, ranking, label, or verification from a deployed door.
---

# OpenAgents decision API

A keyed HTTP service. A caller posts a state plus a map of typed
questions — `noul` (yes/no), `choice`, and `score` — and gets one typed
answer per question with probabilities. Batch classification runs
`openagents.classify.v1` envelopes; durable jobs persist batches.

## What you need first

1. A base URL from the operator — there is no hosted public endpoint
   and no signup flow.
2. An `oak_<id>.<secret>` bearer key issued by the operator. Put it in
   `OPENAGENTS_API_KEY` or the `api_key` field of
   `~/.config/openagents/oak.json` (mode `0600`). Never pass it in a
   URL, a flag, or a tool argument.
3. A door name — `GET /v1/models` with your key lists the doors you may
   name as `model`.

## Ways to call

- **oak CLI** — `oak ask --state state.json --questions questions.json`
  for one call; `oak ask --input lines|ndjson` for a bounded ordered
  batch; `oak models` to list doors; `oak classify` for the
  classification facade.
- **oak-mcp / oak-mcp-http** — MCP over stdio or Streamable HTTP:
  `classify_texts`, `classify_dimensions`, `classify_multi_label`,
  `count_labels`, `review_uncertain`, `decide`, `list_models`, and the
  credential-free documentation tools. The HTTP transport takes the
  caller bearer per request at `POST /mcp`.
- **Raw HTTP** — `POST /v1/systemone`, `POST /v1/classify`,
  `POST /v1/jobs`, `GET /v1/models`, `GET /v1/docs`. See the origin's
  `openapi.yaml` and `api-catalog.json`.

## Reading answers

- Answered questions carry `answer` fields per type; probabilities are
  part of the answer, not a side channel.
- Refusals are typed JSON — a stable `code` naming why, never an HTML
  error. `429` and temporary-capacity codes are retryable; keep the
  `Idempotency-Key` and bump `x-attempt` on retry.
- `GET /v1/docs`, `/v1/docs/{id}`, `/v1/docs/search`, and
  `/v1/docs/examples` read the bundled documentation without a
  credential.

## Limits

- Discovery documents (`/agents.md`, `/auth.md`, `/api-catalog.json`,
  `/.well-known/agent-card.json`) describe implemented behavior only;
  the catalog's `unimplemented` list names what does not exist —
  no OAuth, no streaming, no image input, no self-serve signup.
- Inference consumes quota and can cost money even when nothing on the
  network changes; `GET /v1/balance` exists only where the operator
  enabled monetary admission.
