# The decision worker

`decision-worker` is the NIP-CJ relay front of the Decision API's
serving half. It serves kind-`25910` decision job requests from a Nostr
relay and forwards each admitted call to the same `POST /v1/systemone`
admission path the `gateway` binary serves over HTTP. The wire contract
is
[`docs/decision-models/api/relay-decision-contract.md`](../api/relay-decision-contract.md);
this document is the operator's guide to the process.

The relay is transport. Tenant authorization, artifact binding, quota
reservation, and settlement run once, in the gateway code that already
owns them — the worker adds durable state only where the contract puts
durable ownership on it: a `jobs.jsonl` ledger for pair state and a
live table cancellation resolves against.

## What a job travels

1. **Connect and subscribe.** The worker answers the relay's NIP-42
   challenge with its own key and subscribes
   `{"kinds": [25910], "#p": [<worker pubkey>]}`.
2. **Admit.** Each event passes `nostr::decision::admit` in the
   contract's order: kind, event structure and signature, the `p` tag,
   `created_at` against the request window, then NIP-44 decryption and
   the `openagents.systemone.v1` envelope. A check that fails maps to a
   typed refusal the caller can bind; an undecryptable or unaddressed
   event is dropped.
3. **Resolve the principal.** The verified event signer is the
   principal — never a claim inside the payload. A signer listed in
   `principals` forwards under its provisioned `oak_` credential; an
   unlisted signer forwards with no bearer, the anonymous shared-door
   call, unless `anonymous` is configured off, in which case the job is
   refused `not_admitted`.
4. **Forward.** The envelope posts to `<upstream>/v1/systemone` with
   `Idempotency-Key: <request>` and `X-Attempt: <attempt>`, so a relay
   retry is not a second spend on the HTTP lane either. The caller's
   `deadline` bounds the call; absent a deadline,
   `upstream_timeout_secs` does.
5. **Settle and answer.** The upstream's status maps to an outcome —
   `answered`, `refused`, or `unavailable` — recorded in the ledger,
   then published as a kind-`26910` result with a sealed
   `ExecutionReceipt` (`transport: "relay"`). Progress publishes as
   kind-`27010` statuses (`queued`, `processing`); a terminal refusal
   is a status `error`.

## Configuration

`decision-worker <config.json>` reads one document:

```json
{
  "relay": "wss://relay.openagents.com",
  "worker_secret": "0000000000000000000000000000000000000000000000000000000000000001",
  "upstream": "https://gateway.internal",
  "anonymous": true,
  "jobs": 8,
  "upstream_timeout_secs": 120,
  "jobs_dir": "/var/lib/decision-worker",
  "request_window": {"max_age_seconds": 600, "max_future_seconds": 300},
  "principals": {
    "<caller pubkey hex>": {
      "key": "oak_acme.secret",
      "tenant": "key-ref:acme",
      "workspace": "ws-42"
    }
  }
}
```

- `relay` — the relay URL the worker connects to. The process
  reconnects every two seconds after a dropped session.
- `worker_secret` — the worker's secret key, 64 lowercase hex. Absent,
  the `DECISION_WORKER_SECRET` environment variable supplies it. The
  corresponding public key is the value callers `p`-tag.
- `upstream` — the base URL of a gateway serving
  `POST /v1/systemone`. All admission semantics live there.
- `principals` — signer pubkey (hex) to the credential its jobs forward
  under. `key` is the `oak_<id>.<secret>` bearer; `tenant` is the
  reference receipts and the ledger name (`key-ref:<id>` derived from
  the key when unset); `workspace` is the `X-Workspace-Id` a
  membership-gated upstream requires.
- `anonymous` — whether an unmapped signer forwards with no bearer,
  reaching only the manifest's shared bindings. Default `true`,
  matching the HTTP lane.
- `jobs` — how many jobs run at once. A request past the bound is
  refused `busy` with `retry_after_ms` before anything is held.
- `upstream_timeout_secs` — the upstream call's ceiling when the caller
  set no `deadline`.
- `jobs_dir` — the ledger's directory; `jobs.jsonl` lives inside.
- `request_window` — the `created_at` freshness window, defaulting to
  ten minutes back and five minutes ahead. A request outside it is
  refused `stale`.

Credentials appear only in this file and in `Authorization` headers.
Nothing writes them to payloads, receipts, logs, or the ledger — the
ledger records `key-ref:<id>` tenant references, never secrets.

## Durable state

`jobs.jsonl` is append-only, one JSON line per admission or settlement,
keyed by `(principal, tenant, request, attempt)` — the same quadruple
the upstream ledger settles by:

- A redelivery of a settled pair republishes the recorded result,
  resealed to the new delivery's request event id. Execution stays
  once.
- The same pair carrying a different request digest is refused
  `idempotency_conflict`.
- An `admitted` line with no `settled` line at boot is a crash mark:
  the pair's redelivery redrives the upstream call under the same
  idempotency identity, which joins the existing reservation rather
  than spending twice.
- Events owed while the relay connection is down queue in memory and
  publish on the next session — a settlement owed is a settlement owed.

## Cancellation

A `type: "cancel"` payload resolves only inside the signer's own
`(principal, tenant, request)` scope — a guessed request id names
nothing in another principal's scope, and a cancel signed by another
key leaves the job untouched. A cancelled job settles `unattempted`
when it never dispatched, `unavailable` when it had, with cause
`cancelled` either way. A cancel that arrives after settlement is
ignored: the recorded result stands.

## Refusals

The worker's own refusals go to the contract's table: `stale`,
`not_admitted`, `unsupported_version`, `idempotency_conflict`, `busy`,
and `malformed` are terminal kind-`27010` status `error` events bound
to the delivery that carried the request. Everything past admission is
the upstream's own answer — `door_not_bound`, `quota_exhausted`,
`rate_limited`, `unavailable` — carried as a refused or unavailable
result with the upstream's code kept as cause and its `Retry-After`
kept as `retry_after_ms`.

## Receipts

Every result carries a sealed `ExecutionReceipt` with
`transport: "relay"`. `attempt_id` is the request event the result
answers, so a republished settled result binds to its own transport
delivery — not to the first event that carried the pair.
`request_digest` covers `{model, state, questions, request, attempt,
deadline}`; `tenant` is the resolved tenant reference; `usage` is the
upstream's `x-receipt` pointer, so a relay answer and an HTTP answer
cite the same settlement.
