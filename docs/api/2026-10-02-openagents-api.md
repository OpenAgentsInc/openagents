# The OpenAgents API: OpenAgents itself, behind an API

2026-10-02. Design, revised the same day with the owner's decisions. Nothing
here is a public API yet. It says what the OpenAgents API is, what a
developer sees, how our front turns each call into our own protocols, how
calls are paid with x402, how the HTTP resources follow our NIPs, and the
phased path. Written after [Episode 288](../transcripts/288.md) (open,
permissionless protocols instead of a store that asks permission and shares
no revenue) and [Episode 289](../transcripts/289.md):

> You go to OpenAgents.com, you load the OpenAgents mobile app, you use a
> partner app consuming the OpenAgents API, or their self-hosted version of
> this API, or a subset of it they care about. [...] one OpenAgents
> ecosystem that has all of the best plugins with reputation, identity, and
> payments all figured out.

## Decisions

| # | Decision (owner, 2026-10-02) |
| --- | --- |
| D1 | **HTTP is the public face, and it is plain.** A developer uses it with curl and an API key: JSON in, JSON out, server-sent events for streaming, standard status codes and errors, cursor pagination, idempotency keys. No Nostr concept appears anywhere in it. "Special bullshit in our thing only." |
| D2 | **Nostr stays inside our pieces**, as the `openagents` CLI hides it today. Our API front translates each HTTP call into our NIP traffic. |
| D3 | **Payment is x402.** A call that is neither covered by a key's plan nor free gets a standard `402 Payment Required` with x402 terms; the client pays over Lightning and retries. Key holders on a free or prepaid plan never see a 402. |
| D4 | **Coder through the API runs only on computers a user explicitly grants to that partner's key.** Never on the owner's computers, never on our machines, by default. |
| D5 | **Threads are kept by us from the first slice.** `POST /v1/messages` takes an optional `thread`; we store turns as the apps do. |
| D6 | **The address is `https://api.openagents.com`.** |
| D7 | **Plugin authors are paid a share of each x402 payment for calls that used their plugin**, split automatically over Lightning. |

## 1. What the API is

OpenAgents is a composable general agent (289). Behind an API means a caller
gets the whole agent, not a model: the knowledge base, the router, the
plugins, Coder, the eval system, the wallet, and background rules.

**One call is the product.** `POST /v1/messages` takes a message and an
optional thread. The router, a Jev (System One) judgment over a reviewed
question set ([chat router design](https://github.com/OpenAgentsInc/openagents/blob/main/docs/coder/design/2026-09-28-chat-router.md),
[`crates/coder/src/router/`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/coder/src/router)),
decides what happens: a prepared answer, a knowledge answer, a proposed
`openagents` command, a plugin, an offer to build a missing plugin, a Coder
run on a granted computer, a wallet action, or the model's own answer. Code
acts on that decision. The answer streams, and the decision is part of it.

**Resources under it:** threads, computers, runs, plugins, evals, knowledge,
wallet, background rules, usage, and decisions.

**Who calls it:** partner apps (the whole agent or a subset), websites (our
[Ask box](https://github.com/OpenAgentsInc/openagents/blob/main/crates/openagents-web/src/ask.rs)
is the first), other agents (through HTTP, the OpenAI-compatible front, or
MCP), self-hosters running their own copy or a subset, and our own apps.

It is not a model API, and it is not the decision gateway
([`crates/gateway`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/gateway)),
which answers typed Jev questions. That gateway becomes one resource here
(`/v1/decisions`).

## 2. Plain HTTP outside, our protocol inside

Today a person types `openagents chat` and never sees a relay, a key, an
event kind, or encryption: the CLI and the chat client
([`crates/openagents-chat`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/openagents-chat))
do all of it. The API works the same way. The developer sees HTTP. Our
**API front** does the protocol.

```text
developer --HTTPS, API key, JSON/SSE, x402--> API front --signed, NIP-44 encrypted jobs--> relay --> chat worker
                                                 |                                                   eval runner
                                                 '--HOST grant over relay / iroh / tailnet--> a user's computer (host)
```

**Which component translates.**

| Where | Component | What it does |
| --- | --- | --- |
| Hosted, `api.openagents.com` | A new `api` module in [`crates/openagents-web`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/openagents-web), beside `ask.rs`, which already does this for the Ask box | Checks the API key, collects x402 payment, keeps threads, signs and encrypts each call as a NIP job, sends it to `relay.openagents.com`, reads feedback and results, and renders them as JSON and server-sent events. |
| On a user's computer | [`crates/coder-host`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/coder-host) | Serves the same HTTP surface on `127.0.0.1` (and optionally on the tailnet), backed by its control socket and its own threads, runs, wallet, and background rules. Same paths, same JSON. |
| Both | `openagents-chat` | The one event schema both fronts render, so hosted and local answers look identical. |

**Where keys live.**

- **API keys** (`oak_<id>.<secret>`) are issued and stored hashed in the
  gateway's tenancy registry, which already supports scopes, pause, rotate,
  and revoke ([api catalog](https://github.com/OpenAgentsInc/openagents/blob/main/docs/agents/api-catalog.json)).
  The developer holds the secret.
- **Signing keys** are never seen by developers. The front derives one Nostr
  key per (API key, `user`) pair from a server secret, exactly as the Ask box
  derives one per visitor cookie. The secret lives in the front's
  environment file on its host (as the chat worker's model keys do) and
  nowhere else. A partner serving many people passes a plain `user` string
  (like OpenAI's `user` field); each person gets their own derived key, so
  their threads, grants, and usage stay apart.
- **The Lightning receiver key** that issues x402 invoices is the front's
  wallet node key, on the front's host, with exclusive invoice authority as
  the x402 Lightning scheme requires.
- **On a user's computer**, the host uses its own keys and grants. Nothing
  leaves the machine except the jobs it already sends.

## 3. Conventions

Everything here is ordinary HTTP API practice.

| Topic | Rule |
| --- | --- |
| Base URL | `https://api.openagents.com/v1`. On a computer, `http://127.0.0.1:<port>/v1`. |
| Auth | `Authorization: Bearer oak_<id>.<secret>`. No key is allowed on endpoints that are payable with x402 (section 5). |
| Bodies | `Content-Type: application/json`, UTF-8. Times are RFC 3339. Ids are opaque strings with a type prefix (`th_`, `msg_`, `run_`, `pl_`, `cf_`). |
| Streaming | `Accept: text/event-stream` (or `"stream": true`) returns server-sent events; otherwise one JSON body when the work is done. |
| Errors | Standard status codes (`400`, `401`, `402`, `403`, `404`, `409`, `422`, `429`, `500`, `503`) and one body shape: `{"error": {"type": "not_found", "message": "No thread th_123.", "request_id": "req_…"}}`. |
| Pagination | `?limit=` (default 20, max 100) and `?after=<cursor>`; responses carry `{"data": [...], "next": "<cursor>" or null}`. |
| Idempotency | `Idempotency-Key: <uuid>` on any `POST`. A repeat within 24 hours returns the first response, never a second run. |
| Request ids | Every response carries `Request-Id`. |
| Money | Sats as integers in JSON (`"amount_sats": 1000`). x402 headers use millisatoshi strings, as the x402 spec requires. |
| Versioning | `/v1` in the path; additive changes only within a version. |

## 4. The endpoints, with curl

Set once:

```sh
export OA=https://api.openagents.com/v1
export KEY=oak_…    # from your OpenAgents account
```

### Send a message and stream the answer

```sh
curl -N $OA/messages \
  -H "Authorization: Bearer $KEY" \
  -H "Content-Type: application/json" \
  -H "Accept: text/event-stream" \
  -d '{
    "message": "What is new in the Gym?",
    "thread": "th_8f2c…",
    "user": "customer-4711"
  }'
```

`thread` is optional; without it a new thread starts and its id comes back
in the first event. Optional fields: `attachments` (files or images,
uploaded first with `POST /v1/files`), `computer` (a granted computer id,
section 4.3), `allow` (narrow the routes, such as `["answer", "knowledge"]`;
it never widens what the key may do), and `instructions`.

```text
event: message.accepted
data: {"thread":"th_8f2c…","message":"msg_41…","created_at":"2026-10-02T15:01:02Z"}

event: route
data: {"route":"knowledge.product","tier":"model","confidence":0.94,"router":"chat-router-v4@3b1e…"}

event: text.delta
data: {"delta":"Three new plugins landed in the Gym this week: "}

event: text.delta
data: {"delta":"Project map, …"}

event: message.completed
data: {"message":"msg_41…","text":"Three new plugins …","follow_ups":["Show the leaderboard"],"offers":[]}

event: usage
data: {"route":"knowledge.product","model":"stealth/space-bunny-alpha","cost_usd":0.0014,"cost_status":"priced","first_token_ms":1050,"total_ms":3900}
```

Event types: `message.accepted`, `route`, `text.delta`, `command` (a
proposed `openagents` command), `plugin` (a plugin used, or an offer to build
one), `offer` (anything that changes state or spends; carries a confirmation
id), `run.started`, `run.event`, `run.completed`, `message.completed`,
`usage`, and `error`. Without `Accept: text/event-stream`, the response is
the `message.completed` object with `route`, `offers`, and `usage` inside it.

### Read threads

```sh
curl $OA/threads?limit=20 -H "Authorization: Bearer $KEY"
curl $OA/threads/th_8f2c… -H "Authorization: Bearer $KEY"
curl "$OA/threads/th_8f2c…/messages?limit=50" -H "Authorization: Bearer $KEY"
curl -X POST $OA/threads/th_8f2c…/stop -H "Authorization: Bearer $KEY"
```

Also `PATCH /v1/threads/{id}` (title, pinned, archived) and
`GET /v1/threads/{id}/trajectory` (the thread as an ATIF trajectory).

### Confirm an offer

State changes and payments come back as offers. Nothing happens until the
same caller confirms:

```json
{"type":"offer","offer":{"id":"cf_19…","action":"run.start","label":"Run Coder on Ada's MacBook","expires_at":"2026-10-02T15:11:02Z"}}
```

```sh
curl -X POST $OA/confirmations/cf_19… -H "Authorization: Bearer $KEY"
```

An offer that costs money carries `"price_sats"`; for a pay-per-call caller
the confirmation is the x402-paid call (section 5).

### 4.3 Computers and Coder runs

Coder runs only on a computer the person granted to this partner's key (D4).
In the OpenAgents app the person asks for a connect code for that app (HOST
already has connect codes; the "connect an app" screen is Phase 3 work) and
gives it to the partner app:

```sh
curl $OA/computers -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -d '{"user":"customer-4711","connect_code":"K7Q-…"}'
# 201 {"id":"cmp_3a…","name":"Ada's MacBook","engines":[{"engine":"codex","state":"ready"}],"rights":["observe","operate"]}

curl "$OA/computers?user=customer-4711" -H "Authorization: Bearer $KEY"
```

Start, follow, steer, and stop a run:

```sh
curl $OA/runs -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -H "Idempotency-Key: 5d0e…" \
  -d '{"user":"customer-4711","computer":"cmp_3a…","project":"webapp",
       "prompt":"Fix the failing login test","engine":"codex"}'
# 201 {"id":"run_77…","state":"starting","thread":"th_…"}

curl -N $OA/runs/run_77…/events -H "Authorization: Bearer $KEY" -H "Accept: text/event-stream"
curl $OA/runs/run_77…/steer -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -d '{"message":"Keep the old API working"}'
curl -X POST $OA/runs/run_77…/stop -H "Authorization: Bearer $KEY"
curl $OA/runs/run_77… -H "Authorization: Bearer $KEY"
# {"id":"run_77…","state":"finished","summary":"…","files":[…],"commands":[…],
#  "diff_url":"/v1/runs/run_77…/diff","cost_usd":0.94,"cost_status":"priced"}
curl $OA/runs/run_77…/diff -H "Authorization: Bearer $KEY"     # text/x-diff
```

A run can also start from a message: the reply's `offer` with
`action: "run.start"`, confirmed with `POST /v1/confirmations/{id}`.

### Plugins and evals

```sh
curl "$OA/plugins?q=linear&limit=20" -H "Authorization: Bearer $KEY"
curl $OA/plugins/pl_repo-map -H "Authorization: Bearer $KEY"
# {"id":"pl_repo-map","name":"Project map","author":{…},"version":"1.2.0",
#  "results":{"subject_passed":7,"baseline_passed":4,"total":8},"price_sats":0}

curl $OA/plugins/pl_repo-map/invoke -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -d '{"user":"customer-4711","computer":"cmp_3a…","input":{"project":"webapp"}}'

curl $OA/evals -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -d '{"plugin":"pl_draft_19…","runs":3}'          # runs on our eval runner
curl $OA/evals/ev_51… -H "Authorization: Bearer $KEY"
curl -X POST $OA/evals/ev_51…/publish -H "Authorization: Bearer $KEY"
```

Making a plugin starts in conversation ("turn my meeting notes into Linear
tickets"): the reply offers to build it, drafts its tests, and the draft
becomes `pl_draft_…`, evaluated with `POST /v1/evals` and published with
`POST /v1/plugins/{id}/publish`.

### Knowledge

```sh
curl $OA/knowledge/search -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -d '{"query":"how does the router choose a route","limit":5}'
# {"data":[{"id":"kb_…","title":"…","snippet":"…","source":"https://…"}]}
curl $OA/knowledge/kb_… -H "Authorization: Bearer $KEY"
```

### Wallet

The wallet is the person's own, on their granted computer.

```sh
curl "$OA/wallet?user=customer-4711" -H "Authorization: Bearer $KEY"
# {"balance_sats":52310,"pending_sats":0}
curl "$OA/wallet/payments?user=customer-4711&limit=20" -H "Authorization: Bearer $KEY"
curl $OA/wallet/payments -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -d '{"user":"customer-4711","to":"lnbc…","amount_sats":1000}'
# 202 {"offer":{"id":"cf_…","action":"wallet.pay","label":"Send 1,000 sats"}}
```

Reads need the `wallet:read` scope. A payment always returns an offer; it is
sent only when confirmed, and the person's own spending rules on their
computer still apply.

### Background rules, usage, decisions

```sh
curl "$OA/background?user=customer-4711" -H "Authorization: Bearer $KEY"
curl -X POST $OA/background/bg_disk-cleanup/disable -H "Authorization: Bearer $KEY"
curl "$OA/usage?from=2026-10-01&group_by=route" -H "Authorization: Bearer $KEY"
curl $OA/decisions -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -d '{"model":"jev","state":{…},"questions":[…]}'
```

### OpenAI-compatible front (a convenience)

```sh
curl $OA/chat/completions -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -d '{"model":"openagents","messages":[{"role":"user","content":"What is new in the Gym?"}],"stream":true}'
```

It returns the reply text in the standard shape, with the route and usage in
an `openagents` extension field. It cannot carry offers or runs; those need
`/v1/messages`. `/v1/responses` follows the same rule. An MCP server
(`openagents mcp serve`, which exists) gains an `ask` tool for other agents.

## 5. Payment: x402

We use [x402](https://github.com/x402-foundation/x402) v2 over HTTP, unchanged,
with its `exact` scheme on Lightning
([`scheme_exact_lnbtc.md`](https://github.com/x402-foundation/x402/blob/main/specs/schemes/exact/scheme_exact_lnbtc.md),
merged upstream as #2861). Our [NIP-X402](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-X402.md)
already requires the HTTP role to be exactly that upstream profile, and
[`crates/x402`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/x402)
already implements it: the challenge, the request binding, the replay store,
and an embedded facilitator. Nothing Nostr is involved on this path.

**Who sees a 402.**

| Caller | Result |
| --- | --- |
| Our own apps and the owner's keys | Never. No price, no limit, no usage shown (#10120, #10121). |
| A key on a free or prepaid plan | Never. Calls draw on the plan. |
| A key with no plan, or no key at all | `402 Payment Required` with x402 terms on any priced endpoint. Free endpoints (reading the plugin registry, knowledge reads) answer without payment. |

**The flow.** The x402 Lightning scheme uses the `upfront` flow: payment
settles before the work runs. The steps:

1. Call without payment:

   ```sh
   curl -i $OA/messages -H "Content-Type: application/json" \
     -d '{"message":"What is new in the Gym?"}' -o body.json -D headers.txt
   ```

   ```http
   HTTP/1.1 402 Payment Required
   PAYMENT-REQUIRED: eyJ4NDAyVmVyc2lvbiI6Mi…
   Cache-Control: no-store
   Content-Type: application/json

   {"error":{"type":"payment_required","message":"This call costs 21 sats. Pay the invoice in the PAYMENT-REQUIRED header and retry with PAYMENT-SIGNATURE."},
    "price_sats":21}
   ```

   The header decodes to standard x402 terms:

   ```json
   {"x402Version":2,
    "resource":{"url":"https://api.openagents.com/v1/messages","mimeType":"text/event-stream"},
    "accepts":[{"scheme":"exact","network":"lnbtc:000000000019d6689c085ae165831e93",
      "amount":"21000","asset":"BTC","payTo":"02…","maxTimeoutSeconds":300,
      "extra":{"assetTransferMethod":"bolt11","paymentFlow":"upfront",
        "requestBindingProfile":"http:1","requestBindingParams":{"headers":["accept","content-type"]},
        "requestHash":"0d66…","invoice":"lnbc210n1…"}}]}
   ```

2. Pay the invoice with any Lightning wallet that returns the preimage.

3. Retry **the same request, byte for byte**, with the proof:

   ```sh
   ACCEPTED=$(grep -i '^payment-required:' headers.txt | cut -d' ' -f2 | tr -d '\r' \
     | base64 -d | jq -c '.accepts[0]')
   SIG=$(jq -nc --argjson a "$ACCEPTED" --arg p "$PREIMAGE" \
     '{x402Version:2,resource:{url:"https://api.openagents.com/v1/messages"},accepted:$a,payload:{preimage:$p}}' \
     | base64)
   curl -N $OA/messages -H "Content-Type: application/json" \
     -H "PAYMENT-SIGNATURE: $SIG" \
     -d '{"message":"What is new in the Gym?"}'
   ```

   The response is the normal answer (here an event stream), with a
   `PAYMENT-RESPONSE` header carrying the settlement result.

Or in one command, with the wallet built into our CLI:

```sh
echo '{"message":"What is new in the Gym?"}' | openagents x402 fetch https://api.openagents.com/v1/messages \
    --method POST --body - --max-msat 21000 --max-fee-msat 100 --json
```

The request hash binds the method, URL, body bytes, and the configured
headers, so a paid proof cannot buy a different call. A key holder who pays
per call has `authorization` in the bound headers.

**Pricing follows the upfront flow.** The price must be known before the
router runs, so each endpoint has one published price (a message, a knowledge
search, a plugin invocation). Anything costly is not hidden inside a message:
it comes back as an offer with its own `price_sats`, and confirming it is the
paid call. A Coder run's price is quoted on its offer.

**Plugin authors (D7).** When a paid call used a plugin, the front splits a
share of that payment to the plugin author's Lightning address after the call
settles, and records the split beside the usage record.

**Threads without a key.** A pay-per-call caller with no key still gets a
`thread` id. The id is long and random and is the only thing that opens the
thread, like a share link.

### x402: gaps between our NIP-X402, our code, and upstream

| Gap | Close it by |
| --- | --- |
| The NIP pins upstream commit `4fcf836`. Upstream `main` is 16 commits later; none of them touch the v2 core, the HTTP transport, or the Lightning scheme. | Bump the pin in NIP-X402 and `crates/x402` (doc-only). |
| The repository moved from `coinbase/x402` to `x402-foundation/x402`; the old `coinbase` `main` still lacks the Lightning scheme. | Link only `x402-foundation` (the NIP already does). |
| The upstream SDKs ship no Lightning mechanism (the TypeScript mechanisms are aptos, avm, cardano, casper, concordium, evm, hedera, keeta, near, stellar, svm, tvm, and xrpl). A generic x402 client handles the 402 and the headers but cannot pay Lightning. | Contribute an `lnbtc` mechanism to the upstream TypeScript, Python, and Go SDKs, ported from `crates/x402` and `nostr::x402`, with a payer adapter (NWC or LDK) that returns the preimage. Until then, `openagents x402 fetch` and the curl steps above are the clients. |
| Lightning supports only the `upfront` flow; upstream has `upto` and `escrow` flows on other networks, not Lightning. A message whose cost depends on the route cannot be priced exactly. | Fixed per-endpoint prices and priced offers now. Later, propose an `escrow`-flow Lightning variant upstream using hold invoices (settle a ceiling, charge the actual). |
| v1 clients use `X-PAYMENT` and `X-PAYMENT-RESPONSE`; the Lightning scheme is v2-only (`PAYMENT-REQUIRED`, `PAYMENT-SIGNATURE`, `PAYMENT-RESPONSE`). | Serve v2 only and say so in the 402 body. |
| Upstream has an A2A transport; the Lightning request binding defines only `http:1` and `mcp:1`. | If we offer A2A, propose an `a2a:1` binding profile upstream first. |
| A paid call that fails after settlement has no refund (spec and NIP). | Our policy: a failure before any answer is retried free under the same `Idempotency-Key`; no automatic refund. |
| The x402 discovery extension (the "Bazaar") can list paid endpoints. | List `api.openagents.com` endpoints there once the Lightning mechanism exists upstream. |

Inside our front, payment ends at the HTTP edge: the NIP jobs the front then
sends are unpaid internal traffic. NIP-X402's own `nostr:openagents:1`
profile is for self-hosters and agents buying natively (section 11).

## 6. How the HTTP resources follow our NIPs

The resources are shaped after the protocol, one to one, so the front is a
translation and a self-hoster can implement either side. The developer never
sees the right-hand columns.

| HTTP endpoint | Our NIP and kinds it becomes | Notes |
| --- | --- | --- |
| `POST /v1/messages` | [CJ](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-CJ.md) conversation: request `25900`, feedback `27000` (`status`, `judgment`, `partial`, `offer`, `card`), result `26900` | `judgment` becomes `route`, `partial` becomes `text.delta`, `offer` becomes `offer`, `card` becomes `plugin` or `run.*`. The stored thread supplies the transcript. The signer is the derived key for (API key, `user`). |
| `GET /v1/threads`, `GET /v1/threads/{id}`, `…/messages`, `POST …/stop` | Hosted: the front's thread store (gap G1). On a computer: [HOST](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-HOST.md) `thread.list`, `thread.read`, `thread.stop` (private `3188` artifacts, or CJ execution `25920`/`26920`/`27020`) | Same JSON either way. |
| `GET /v1/threads/{id}/trajectory` | [ATIF](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-ATIF.md) `3198`/`3199` | Rendered as plain ATIF JSON. |
| `POST /v1/confirmations/{id}` | Depends on the offer: `run_coder` to HOST `thread.run`; `start_eval` to an EVAL hosted run; `cli` to HOST `task.command`; `wallet.pay` to the proposed HOST `wallet.pay` (G4); approvals per [POL](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-POL.md) action review | Offers need stable ids and prices (G7). |
| `POST /v1/computers`, `GET /v1/computers`, `DELETE /v1/computers/{id}` | HOST connect code and `enroll.redeem`; the grant held; [REACH](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-REACH.md) presence for name and engines | The derived key is the HOST device. Deleting drops our grant locally; the person revokes in their app with `device.revoke`. |
| `POST /v1/runs` | HOST `task.create` (`title`, `prompt`, `workspace`, `engine`) | Needs the `operate` right on that computer. |
| `GET /v1/runs/{id}`, `GET /v1/runs/{id}/events`, `GET /v1/runs/{id}/diff` | [RUN](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-RUN.md) records `3187` and heads `30186`; observation per [SESS](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-SESS.md) and [CTRL](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-CTRL.md); ATIF for the trajectory | Server-sent events are the run's event lines. |
| `POST /v1/runs/{id}/steer`, `POST /v1/runs/{id}/stop` | HOST `task.steer`, `task.cancel` | |
| `GET /v1/plugins`, `GET /v1/plugins/{id}` | [EXT](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-EXT.md) listings `30184`, releases `3184`, revocations `3185`; [EVAL](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-EVAL.md) publications for results | The front queries public relays and caches. |
| `POST /v1/plugins/{id}/invoke` | [CAP](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-CAP.md) operation through CJ execution `25920`/`26920`/`27020` ([PRG](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-PRG.md) workflows) | Runs where the plugin's effects allow: on the granted computer, or hosted for read-only ones. |
| `POST /v1/evals`, `GET /v1/evals/{id}`, `POST /v1/evals/{id}/publish` | EVAL hosted runs: CJ execution `25920` to the eval runner, `ext-eval` actions `run` and `publish` | |
| `POST /v1/plugins/{id}/publish` | EXT release `3184` and listing `30184`, signed by the author's derived key | The author's Lightning address for D7 rides in the release `meta` (G9). |
| `POST /v1/knowledge/search` | No NIP operation today (G3); [KB](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-KB.md) entries are `3190`/`30190` | |
| `GET /v1/knowledge/{id}` | KB head `30190`, version `3190`, withdrawals `3191`, evidence `3189` | |
| `GET /v1/wallet`, `GET /v1/wallet/payments`, `POST /v1/wallet/payments` | HOST `spend.list` covers x402 spends only; no wallet operations (G4) | |
| `GET /v1/background`, `POST …/enable`, `…/disable` | [AUTO](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-AUTO.md) plan admission and controls through CJ execution to the host | |
| `GET /v1/usage` | POL `openagents.route-usage.v1` artifacts; the CJ result's `usage` (tokens only, G5) | |
| `POST /v1/decisions` | [DEC](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-DEC.md) `25910`/`26910`/`27010` | DEC already defines its HTTP-gateway equivalence; this is the model the rest follows. |
| `GET /v1/profile` (later) | [XP](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-XP.md) awards `3193`, trainer cards `30194` | |
| `POST /v1/files` | HOST `artifact.put` on a computer; hosted, the front's store (G6) | |
| x402 on any endpoint | Upstream x402 `http:1` (NIP-X402 HTTP role), at the edge only | |

Upstream NIPs underneath: [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md)
events and signatures, [NIP-44](https://github.com/nostr-protocol/nips/blob/master/44.md)
encryption, [NIP-42](https://github.com/nostr-protocol/nips/blob/master/42.md)
relay authentication, and [NIP-40](https://github.com/nostr-protocol/nips/blob/master/40.md)
expiration. CJ follows the job pattern of [NIP-90](https://github.com/nostr-protocol/nips/blob/master/90.md)
(request, feedback, result) with its own encrypted kinds. [NIP-98](https://github.com/nostr-protocol/nips/blob/master/98.md)
HTTP auth is not on the developer path; it is the option for implementers
who sign HTTP calls with their own Nostr key (section 11).

### Gaps in our NIPs, and the change to propose

Each gap is closed in the NIP, not with a side channel in the front.

| Gap | What the HTTP API needs | Proposed NIP change |
| --- | --- | --- |
| G1 | Hosted threads (D5). CJ conversation jobs are stateless: the caller sends the transcript. | The chat worker (or the front) advertises the HOST thread operations (`thread.list`, `thread.read`, `thread.send`, `thread.stop`) as CAP roles, so hosted and computer threads are one contract. |
| G2 | Stopping a reply. CJ conversation jobs have no cancel ("effects requiring cancellation use execution jobs"). | Add a conversation `cancel` control mirroring the execution family's, or route stops through G1's `thread.stop`. |
| G3 | Knowledge search. KB defines entries, not a query. | A CAP operation role `kb.search` over CJ execution, returning cited KB references. |
| G4 | Wallet reads and payments on a granted computer. HOST has `spend.list` and `spend.settle` for x402 spends only. | HOST operations `wallet.read` (new right `wallet_read`) and `wallet.pay` (new right `spend`), with every payment still a POL approval. |
| G5 | Cost per call. The CJ result carries token counts only. | An optional `cost` (`{micro_usd, status: priced/partial/unknown}`) and timings in the CJ result, or a POL `route-usage.v1` reference in it. |
| G6 | Attachments. CJ conversation content is strings only. | CJ request `attachments` as bounded ArtifactRefs, as HOST `task.create` already takes `images`. |
| G7 | Confirmations and quotes. CJ offers have no stable id or price. | Offers gain `id` and optional `price_msat`, so a confirmation and an x402 quote bind to one offer. |
| G8 | A person must see which app holds a grant. HOST device listings have no label. | An enrollment `label` (the app's name) kept in the grant and shown in `devices`. |
| G9 | Paying plugin authors (D7). EXT releases name no payout address. | An optional `payout` (Lightning address or node key) in the EXT release, signed by the author. |

None needs a new event kind.

## 7. Where it runs

| Place | What it serves |
| --- | --- |
| **Hosted**, `api.openagents.com` | Messages, threads, knowledge, plugin reads, read-only plugins, evals on our eval runner, decisions, usage. Never a Coder run or a person's wallet: those go to a granted computer. |
| **A person's computer** (the host) | Coder runs, local commands, plugins with local effects, the wallet, background rules, files. Reached by the hosted front through the person's grant, or directly on `127.0.0.1` by local apps and scripts. |
| **Self-hosted subsets** | Any part: the front and the chat worker with the operator's own model keys, a knowledge-only service, a plugin registry mirror. Same HTTP surface, same protocol, so keys and plugins work across them. |

## 8. Keys, scopes, approvals, audit

- **Scopes on an API key:** `chat`, `threads`, `knowledge`, `plugins:read`,
  `plugins:invoke`, `plugins:publish`, `computers`, `runs`, `wallet:read`,
  `wallet:pay`, `background`, `usage`. A new key gets `chat`, `threads`,
  `knowledge`, `plugins:read`.
- **Rights on a computer** are the HOST grant the person gave, never more
  than the scope allows and never more than the person granted.
- **Approvals:** state changes and payments are offers confirmed by the same
  caller; payments also obey the person's own rules on their computer. No
  scope approves a POL action by itself.
- **Audit:** a usage line per call (ids and fixed words, no message text),
  run records, x402 settlement records, and plugin payout splits.
- **No usage limits in our apps.** Partner abuse is handled by key pause and
  revoke, per-key concurrency bounds, the engineering bounds every worker
  already has, and payment.

## 9. The cost selling point

A caller gets System One for free: a Jev judgment (about 200 ms and a
fraction of a cent) sends each message to the cheapest thing that answers it
well. A prepared answer costs no model call; a knowledge answer is one
retrieval and a fast model; a Coder run uses the recipe in the
[cost audit](https://github.com/OpenAgentsInc/openagents/blob/main/docs/cost/2026-10-02-system-one-cost-efficiency-audit.md),
61% cheaper and 37% faster than the same model run raw across eight
development tasks, and 45% cheaper with more passes on 26 Terminal-Bench 4.0
tasks. Every response includes `usage` (route, model, `cost_usd`,
`cost_status`, `first_token_ms`, `total_ms`) so the claim is checkable per
call. In the owner's apps, cost stays recorded and unshown
([f9a8cce433](https://github.com/OpenAgentsInc/openagents/commit/f9a8cce433)).

## 10. Phased path

**Phase 0, exists.** The Ask box (`POST /ask`), `openagents chat --json`, CJ
conversation jobs, HOST thread operations, `openagents mcp serve`, gateway
keys and receipts, `crates/x402` with `openagents x402 fetch`, Coder run cost
records ([#10161](https://github.com/OpenAgentsInc/openagents/issues/10161)).

**Phase 1, the first slice: hosted messages with threads and x402.**
- `POST /v1/messages` (JSON and server-sent events), `/v1/threads` reads and
  stop, `/v1/knowledge/search`, `/v1/plugins` reads, `/v1/usage`, on
  `api.openagents.com`.
- `oak_` keys with plans; x402 Lightning for everyone else.
- A new `Surface::Api` router policy, starting from the web surface's
  (answers and knowledge, no Coder).
- The OpenAI-compatible `/v1/chat/completions`.
- NIP amendments G1, G2, G3, G5 drafted with it.
- Crates: `openagents-web` (the `api` module), `openagents-chat` (event
  schema, `Surface::Api`), `coder` (`router::policy`, usage key id),
  `gateway` (key lookup), `x402` (the HTTP edge). Size: medium.

**Phase 2, plugins and evals.** Invoke, create through conversation, evals on
the eval runner, publish, and author payouts (D7, G9). An MCP `ask` tool.
Size: medium.

**Phase 3, computers.** Connect codes for partner apps (G8), runs, steer,
stop, diffs, wallet (G4), background rules, files (G6), confirmations (G7).
Crates: `coder-host`, `coder` task, `wallet`, `openagents-web`. Size: large.

**Phase 4, local and self-hosted.** The host serves the same HTTP surface on
`127.0.0.1`; a packaged front and worker for self-hosters; a conformance suite
over the HTTP schema and the NIP mapping. Size: medium.

**In parallel:** the upstream `lnbtc` SDK mechanism (section 5).

## 11. For self-hosters and implementers: the raw protocol

Most developers should stop at section 10. This section is for people who
run their own copy, write another implementation, or build an agent that
speaks Nostr natively.

- Every HTTP call in section 6 has a NIP equivalent. A client can skip the
  front and publish CJ jobs to `relay.openagents.com` itself, signed with its
  own key and NIP-44 encrypted to the worker's public key (the one compiled
  into our apps; see the [chat worker doc](https://github.com/OpenAgentsInc/openagents/blob/main/docs/deployment/chat-worker.md)).
- An HTTP client that wants its own Nostr identity instead of an API key can
  sign requests with NIP-98; the front then uses that key as the signer
  instead of a derived one.
- Native payment uses NIP-X402's `nostr:openagents:1` profile, an OpenAgents
  extension, not an upstream x402 profile.
- A self-hoster runs the same `coder-worker`, front, and relay with their own
  keys and model doors. Their users' plugins, knowledge entries, and XP are
  the same signed events, so they stay in the one ecosystem.
- The specifications: [`nips/openagents/`](https://github.com/OpenAgentsInc/openagents/tree/main/nips/openagents),
  starting with CJ, HOST, RUN, EXT, EVAL, KB, and X402.

## 12. Still open

1. Prices: the per-endpoint price list, and what free plan (if any) a new key
   gets. The Ask box answers anonymous visitors free today; should an
   anonymous `POST /v1/messages` be free too, or always x402?
2. The plugin author's share of a payment (D7): a fixed percentage, or a fee
   the author sets?
3. Accept upstream's standard stablecoin schemes (USDC through a public
   facilitator) beside Lightning, or Lightning only?
4. How much of the `route` event to show: the route id only, or also
   probabilities and the question set's digest?
