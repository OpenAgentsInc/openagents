# The gateway

`crates/gateway` is the keyed HTTP front of the Decision API's serving
half — the one admission path every decision call travels. A backend
that answers a caller directly is a deployment misconfiguration, not a
second path.

## What a call travels

Every `POST /v1/systemone` runs the same sequence:

1. **Authenticate.** `Authorization: Bearer oak_<id>.<secret>` resolves
   to a tenant through `tenancy::keys`. A request with no credential is
   anonymous; an anonymous call reaches the manifest's `shared` bindings
   and nothing else.
2. **Authorize.** The request's `model` field names a door; the
   registry's `authorize` returns the admission snapshot the call is
   served under. The snapshot is a copy — a registry update mid-flight
   cannot relabel the call, and the receipt names the digest and
   sequence that admitted it.
3. **Bound.** The door's declared rate window and concurrency, then the
   process's forward bound, refuse before the reservation exists — a
   congestion refusal never holds quota.
4. **Reserve.** `tenancy::quota` writes the `reserved` event before any
   byte is dispatched. A crash after this point leaves a held
   reservation that recovery orphans as `unknown` — never an
   unaccounted spend.
5. **Verify.** The backend's `GET /v1/models` card is checked against
   the bound identity. A moved model, moved adapter, swapped artifact,
   or drifted execution setting refuses before the request is
   forwarded — the caller is never billed an answer the wrong artifact
   produced.
6. **Forward, settle, receipt.** The backend's status maps to a typed
   outcome — answered, refused, unavailable — the reservation settles
   once, and a sealed `ExecutionReceipt` appends to `receipts.jsonl`
   beside the registry.

Backend identity reads and inference stay on the configured endpoint. The
client follows no HTTP redirects. Both response paths enforce
`max_response_bytes` while reading, including chunked responses with no declared
length. A redirected or oversized identity document refuses before inference;
a redirected inference response is unavailable, with potentially attempted work
retained for settlement.

## Configuration

`gateway --config gateway.json` reads one document:

```json
{
  "v": "openagents.gateway.v1",
  "listen": "0.0.0.0:443",
  "registry": "/etc/openagents/registry",
  "max_body_bytes": 1048576,
  "max_response_bytes": 4194304,
  "forward_timeout_ms": 120000,
  "reservation_ttl_secs": 300,
  "max_in_flight": 64,
  "max_questions": 256,
  "max_options": 4096,
  "doors": {
    "shared-kev": {"endpoint": "http://127.0.0.1:9080"},
    "acme-kev": {"endpoint": "http://10.0.1.7:9080"}
  }
}
```

`registry` is the directory `tenancy` manages: `registry.json`,
`keys.json`, `quota-ledger.jsonl`, and the gateway's `receipts.jsonl`
all live there. A door named in the registry but missing from `doors`
is `door_unavailable` — the gateway refuses rather than guessing an
address, because which host serves a door is the operator's business
and which identity that host publishes is the binding's.

`reservation_ttl_secs` must cover `forward_timeout_ms` — a reservation
that expired while its forward still ran would orphan live work, and
the config check refuses the pairing.

## Routes

- `POST /v1/systemone` — the decision call. The body is the TypeSafe
  request envelope: `state`, a `model` naming the door, and `questions`.
- `GET /v1/models` — the doors the caller's tenant may name, with the
  identity each is bound to. This is the registry's claim, not a
  statement about backend health — a listed door may still refuse at
  identity check, and the card is the serving process's word about its
  own artifact, never attestation.
- `GET /healthz` — the process is up. Says nothing about backends;
  identity verification runs per request.

Work that outlives a request runs as a durable job instead:

- `POST /v1/jobs` — accept a classify request, persist it, and run it
  outside the submitting connection.
- `GET /v1/jobs/{id}` — the job's status document: state, counts,
  receipt, and cause.
- `POST /v1/jobs/{id}/cancel` — cancel queued and undispatched work;
  idempotent.
- `GET /v1/jobs/{id}/results` — the finished items, paginated by an
  expiring cursor.
- `DELETE /v1/jobs/{id}` — remove a terminal job's record.
- `POST /v1/jobs/{id}/notify/rotate` — rotate the webhook signing
  secret a notified job signs with.

[durable-jobs](durable-jobs.md) is the contract: the same admission
path, a manifest and item ledger beside the registry, honest
`unknown` outcomes for ambiguous work, and opt-in signed webhooks.

The gateway also serves a public discovery surface — unauthenticated
`GET` routes that describe the deployment: the document set
(`llms.txt`, `agents.md`, `auth.md`, `skills.md`, `api-catalog.json`,
`openapi.yaml`), the well-known agent card and agent-skills index, a
mirrored MCP server card, the versioned `/v1/docs` corpus API
(list/read/search/examples), plugin packages under `/plugins/`, and
`sitemap.xml`/`robots.txt`. Canonical URLs come from the optional
`public_origin` config or the request's `Host`; forwarded headers are
never trusted. Nothing on the surface decides and nothing on it
authenticates — the inference routes still require the bearer
credential.

## Responses

Every attempt carries `x-request-id`, `x-attempt`, `x-outcome`, and
`x-receipt` — the receipt's digest, resolving to a sealed line in
`receipts.jsonl`. A forwarded answer arrives verbatim with the backend's
status. The gateway's own refusals are typed JSON:

```json
{"error": {"code": "quota_exhausted", "message": "…",
           "request": "req-…", "attempt": 1}}
```

| Status | Codes | Meaning |
| --- | --- | --- |
| 400 | `malformed`, `invalid_request` | The header or envelope does not parse. |
| 401 | `unauthenticated` | The credential is missing its `Bearer` shape, unknown, revoked, or wrong. |
| 403 | `door_not_bound` | The tenant holds no binding for the door, and the door is not shared. |
| 409 | `idempotency_conflict` | The `(request, attempt)` pair is taken — resolved, or held for different content. |
| 422 | `invalid_request`, `too_many_questions`, `too_many_options` | The envelope names no `model` door, or carries more questions or options than `max_questions`/`max_options` admit — refused before the door is consulted. |
| 429 | `rate_limited`, `busy`, `overloaded`, `quota_exhausted` | Capacity, or a spent budget — `Retry-After` accompanies the refusal. |
| 503 | `door_unavailable`, `identity_mismatch`, `unavailable` | No backend is configured, the backend's card disagrees with the binding, or the forward failed. |

## Retries and idempotency

A caller that wants a retryable request sends `Idempotency-Key` and
`X-Attempt`. The pair `(key, attempt)` is the reservation's identity:
the same pair reserved twice while in flight returns the reservation
that exists — a retry is not a second spend. The same pair with a
different request body is `idempotency_conflict`: a caller that reuses
a key for changed content is making two requests and calling them one.
A pair that already resolved is refused the same way — a completed
request's key is spent, and a genuinely new attempt carries the next
attempt number.

A 429 or 503 is retryable; the `Retry-After` header accompanies the
congestion codes. A 401, 403, or 409 is not — the refusal is about the
request itself, not the moment it arrived.

## The privacy lanes

A `dedicated` door binds a tenant to a backend its calls alone reach;
a `shared` door serves every keyed tenant and anonymous callers the
operator admits. The lane is the binding's word — the gateway enforces
which calls reach which endpoint, and a shared lane makes no dedicated
privacy claim. What a backend does with the requests it sees is the
operator's agreement with that host, not something a lane name can
promise.

## Retention and redaction

The gateway keeps no request bodies. `receipts.jsonl` holds the sealed
receipt per attempt: the request and attempt identity, the tenant's key
reference, the registry revision, the requested and served identities,
the outcome and cause, timing, the request's canonical digest, the
response body's digest, and the usage reference — digests and
references, never the caller's text. The quota ledger's `reserved` and
`settled` events carry the same shape: identities, digests, unit counts.
A caller's questions and state exist in memory for the request's
lifetime and nowhere else.

## The trusted boundary

The gateway is the public surface; backends are not. The deployment
policy is that a door's endpoint binds a private interface — a
loopback port, a pod-local address, a private subnet — and accepts
forwarded calls only. The check `Admission::verify` runs proves the
serving process claims the identity the registry bound; it is not
remote attestation, and it cannot be — the card is the process's own
word. Plain `http://` endpoints are expected between the gateway and a
backend on the same trusted network; TLS terminates at the gateway's
public listener or the balancer in front of it.

## Operating it

One gateway process owns a registry directory — the ledger's exclusive
lock refuses a second writer, so run one replica per registry or put a
single writer behind a failover pair. Provisioning is `tenant-keys`
(issue, rotate, revoke) against the same directory, and `tenant-usage`
reads the ledger's position. A registry update lands on the next
request — the gateway rereads the manifest per call — while a call in
flight keeps the admission it was served under.

## Caller disconnect cleanup

Native and classification handlers retain ownership of bounded cleanup after a
caller disconnects. Queued classification work stops; completed work remains
accounted for. A forwarded parent call records `caller_disconnected`; a call
stopped before dispatch records `cancelled`. A canceled identity
lookup releases a provably undispatched reservation and monetary hold. A canceled
inference dispatch has unknown completion and retains its monetary hold until
reconciliation. HTTP cancellation does not establish remote computation stopped.

Classification doors also declare `max_forward_bytes`, a bound on each complete
serialized native request. Discovery advertises this byte limit. Primary
classification validates all expanded envelopes before reserving usage; secondary
review and fallback calls validate their own envelopes against classification
bounds when their doors declare them, before reservation.
Oversized envelopes receive `context_limit` without truncation. Token capacity
remains unknown unless a backend-specific mechanism establishes it.
