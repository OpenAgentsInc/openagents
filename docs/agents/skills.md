# Call the decision API from an agent

What an agent needs to authenticate and call the OpenAgents decision
API, which caller surfaces exist, and the limits the service states
honestly. Everything here describes implemented behavior —
[api-catalog.json](api-catalog.json) is the authoritative route list,
and its `unimplemented` list names what does not exist.

## What you need

1. **A base URL.** No hosted public endpoint ships in this repository;
   an operator runs `crates/gateway` and hands you the address
   (`--url`, `OPENAGENTS_BASE_URL`, or `base_url` in the config file).
2. **A credential.** An `oak_<id>.<secret>` bearer key, issued by the
   operator. [auth.md](auth.md) covers where it may live and what a
   rejected call returns. There is no signup flow.
3. **A door name.** `GET /v1/models` lists the doors your credential
   may name in a request's `model` field; anonymous callers see
   `shared` doors only.

## Ways to call

| Surface | What it is | Status |
| --- | --- | --- |
| `oak` CLI | `oak ask` — one state, or a bounded `--input lines\|ndjson` batch with ordered NDJSON output; `oak models`; `oak classify`; `oak version`. Build with `cargo build -p oak --release`. | Supported |
| Raw HTTP | The routes in [openapi.yaml](openapi.yaml): JSON in, typed answers or typed refusals out. | Supported |
| `oak-mcp` | An MCP stdio server: `list_models` and `classify`, plus the bundled documentation tools `list_docs`, `read_doc`, `search_docs`, and `get_examples`. | Supported |
| `crates/jev` | The Rust SDK `oak` builds on, for the native `POST /v1/systemone` contract — including TypeSafe's hosted door with a `ts-` key. | Supported |

`oak-mcp` reads newline-delimited JSON-RPC on standard input and
output; credentials, the endpoint, and the workspace come from operator
configuration, never tool arguments. See
[classification callers](../decision-models/guides/classification-callers.md)
for its lifecycle and bounds.

## Supported versions

| Contract | Version |
| --- | --- |
| `oak` and `oak-mcp` binaries | The workspace crate version — `oak version` reports it (`0.1.0` at this writing) |
| Classification envelope | `openagents.classify.v1` |
| Classification policy | `openagents.classify-policy.v1` |
| Review policy (inside classify) | `openagents.classify-review.v1` |
| Classification discovery card | `openagents.classify-discovery.v1` |
| MCP protocol | `2025-11-25` and `2025-06-18` |
| Gateway deployment document | `openagents.gateway.v1` — operator-side |

## Reading answers

- `answered`, `refused`, `unavailable`, and `unattempted` — plus
  `mixed` on classify — are first-class outcomes, not edge cases. A
  `refused` result carries the door's typed code; an `unavailable`
  result means the work may not have run at all. Neither is a wrong
  answer, and neither is a zero.
- Probabilities are calibrated judgments. A `noul` of 0.5 means yes and
  no are about equally likely — it is not "medium". The threshold that
  turns a probability into an action belongs to your code, measured on
  your own labelled data.
- `confidence` describes how concentrated a distribution is. It is not
  permission to act and not an end-to-end correctness probability.

## Retries and idempotency

- Send `Idempotency-Key` on work you may retry and increment
  `X-Attempt` per attempt. The service settles quota by that pair: a
  retry shares one reservation, and a settled pair reused against
  changed content answers `409 idempotency_conflict`.
- `429` — `rate_limited`, `busy`, `overloaded` — and `5xx` are
  retryable; honor `Retry-After`. `quota_exhausted` is a daily budget:
  retrying cannot fix it. `401`, `402`, `403`, `409`, and the
  envelope-shape codes are not retryable.
- A POST transport failure with no `Idempotency-Key` is uncertain
  work: `oak` reports it rather than silently re-dispatching.

## Honest limits

- **Cost is the operator's policy.** Quota units — requests,
  questions, input bytes, outstanding reservations — come from the
  tenant's quota, not a published price list. When the deployment runs
  monetary admission, each call additionally holds worst-case spend
  against your workspace and `GET /v1/balance` reads the position.
- **The model list is the door list.** `GET /v1/models` returns the
  doors your credential may name and the identity each is bound to — a
  serving claim verified at dispatch, not remote attestation.
- **No streaming, no chat.** One request is complete and independent;
  the service generates no prose.
- **No image or multimodal input.** `state` is JSON or text.
- **Bounds refuse; they do not truncate.** Body bytes, question count,
  option count, per-door rate and concurrency, and daily quota each
  refuse with a typed code before work is billed.
- **Receipts record digests, not content.** `x-receipt` resolves to a
  sealed receipt line in the service's `receipts.jsonl`; request bodies
  are not retained.

## Where to read more

- [auth.md](auth.md) — credentials, workspace membership, and the refusal shape.
- [api-catalog.json](api-catalog.json) — routes, refusal codes, and unimplemented surfaces.
- [openapi.yaml](openapi.yaml) — the machine-readable contract.
- [Caller guide](../decision-models/guides/caller.md) — the end-to-end caller contract.
- [Gateway service document](../decision-models/service/gateway.md) — the implemented admission path.
- [Caller examples](../decision-models/examples/README.md) — runnable `curl` and `oak` invocations.

---
Version 1.0.0 · generated-by: hand-maintained · 2026-09-21

VALIDATED: internal links resolve to repo paths; commands and claims match `crates/oak` and `crates/gateway` as of 2026-09-21. Exact check commands are in the commit message.
