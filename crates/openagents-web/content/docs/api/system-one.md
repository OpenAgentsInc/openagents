# Typed decisions with System One

`POST https://api.openagents.com/v1/systemone` answers named questions about
one state. The question types are `noul` (probability of yes), `choice`, and
`score`. It returns JSON answers, not a chat transcript or an SSE stream.

This differs from the OpenAI-shaped `/v1/decisions` endpoint. Keep each
operation's request and response shapes separate. The public
[OpenAPI reference](https://api.openagents.com/v1/openapi.json) describes both.

## Send a call

Prepare `call.json` with `model`, `state`, and a `questions` map. Use a decision
door your deployment permits. The public `/v1/models` inference catalog is not
the same contract as a standalone decision gateway's door catalog. The
[decision examples](https://github.com/OpenAgentsInc/openagents/tree/main/docs/decision-models/examples)
show the typed question format.

```sh
curl --fail-with-body https://api.openagents.com/v1/systemone \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H 'Content-Type: application/json' \
  --data-binary @call.json
```

A gateway-issued API key must have access to the selected door. A supported
account session also needs its current workspace membership; send
`X-Workspace-Id` when that deployment requires it. Sign-in to an app does not
automatically grant decision access. Requests without credentials work only
when the deployment's shared-door and funding policies permit them.

## Outcomes and retries

The response includes one typed answer per question. Noul reports `noul`;
Choice reports `choice`, `confidence`, and `probabilities`; Score reports
`score`, `confidence`, and `legend`, with probabilities when provided.
Probabilities express model judgments, not permission to act.

Errors use `error.code` and `error.message`. Known codes include
`unauthenticated`, `door_not_bound`, `idempotency_conflict`, `rate_limited`,
`busy`, `quota_exhausted`, and `identity_mismatch`. Backends can return other
typed refusal codes. Keep them rather than replacing them with a generic error.

For a retry, retain `Idempotency-Key` and increment `X-Attempt`. Reusing a settled
pair, or changing content under the same pair, returns `409`. Follow
`Retry-After` when present. Daily quota exhaustion needs the next available
budget; retrying immediately does not create one. Keep `x-request-id`,
`x-attempt`, `x-outcome`, and `x-receipt` when returned. The gateway reserves
quota before forwarding and settles the attempt with a receipt.

## Public API and standalone gateways

The [decision gateway contract](https://github.com/OpenAgentsInc/openagents/blob/main/docs/decision-models/api/openapi.yaml)
also covers classification and jobs on a separately deployed decision gateway.
Those paths are not public API-front operations. The combined public reference
includes only System One and its referenced types from that contract. A schema
or a discovery link does not grant access to another service or operation.
