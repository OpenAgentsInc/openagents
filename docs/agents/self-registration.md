# Agents that register themselves

Issue [#11630](https://github.com/OpenAgentsInc/openagents/issues/11630).
The owner decided on 2026-10-14 that agents may create their own accounts
and do basic things without burning down usage unless they pay. The agent
facing description is [`/auth.md`](https://openagents.com/auth.md)
(`crates/openagents-web/content/agents/auth.md`).

## The flow

| Step | Route (gateway, also under `openagents.com/api/v1`) |
| --- | --- |
| Get a proof-of-work challenge (10 minutes) | `GET /v1/agents/challenge` |
| Register: `{name, challenge, solution}` where SHA-256(`challenge:solution`) has 22 leading zero bits | `POST /v1/agents/register` → `201` with `api_key` (shown once), the account (`kind: agent`), a `claim` code, `free_tier`, `pay` |
| See the free tier left today, the credit, how to pay | `GET /v1/agents/me` (agent key) |
| A person links the agent to their account | `POST /v1/agents/claim` `{claim}` (person's session), or Settings → Agents (`/settings/agent-accounts`) |
| The person's agents | `GET /v1/agents/claimed` |
| Turn an agent off (its key stops at once) | `POST /v1/agents/{account}/revoke` (the claiming person or a site admin) |
| Every agent account (site admins; also listed on `/settings/agent-accounts`) | `GET /v1/admin/agents` |
| Credit after a Lightning payment (the pay host only, signed `OpenAgents-Paid` header) | `POST /v1/agents/credit?account=acct_…` |

Discovery: `/auth.md`, the `agent_auth` block of
`/.well-known/oauth-authorization-server` (`identity_types_supported:
["anonymous"]`, `anonymous.credential_types_supported: ["api_key"]`,
`register_uri`, `claim_uri`), `llms.txt`, the API catalog, the JSON site
index, and the served OpenAPI (`Agents` tag).

Code: `crates/gateway/src/agent_accounts.rs` (routes, proof of work,
limits, the `402`, credit), `crates/gateway/src/inference_public.rs`
(admission and settlement), `crates/tenancy/src/accounts/agents.rs` (the
agent marker on the account, claims, revocation),
`crates/openagents-web/src/agent_accounts.rs` (the Settings page). Tests:
`crates/gateway/tests/agent_signup.rs`.

## What an agent account is

- An ordinary account with one personal workspace on the sign-up tenant,
  marked `agent` (never a person), holding one `oak_` key scoped to
  `responses`, `models:read`, `usage:read`. No email, no GitHub identity.
- It can't become a website session (`POST /v1/sessions` answers
  `403 agent_account`, and its key is scoped), so chats, computers, Solver,
  Drive, and the signed-in free allowance stay out of reach.
- It has no workspace balance; it has its own bought credit.

## Abuse limits

| Limit | Value |
| --- | --- |
| Proof of work | 22 leading zero bits of SHA-256 (about 4 million hashes, a few seconds), one use per challenge, 10-minute expiry, signed with a key kept beside the registry |
| Per address | 5 registrations a day (UTC) |
| Per network (IPv4 /24, IPv6 /48) | 20 a day |
| Everyone | 500 a day (persisted) |

The address is the `X-Forwarded-For` entry `forwarded_hops` places from the
right (`AGENT_FORWARDED_HOPS`, default 1); anything left of it is the
caller's own text. Per-address and per-network counts are kept in memory
(a gateway restart clears them); the daily total survives restarts.

## The free tier (per agent, per UTC day)

| What | Value |
| --- | --- |
| Models | `google/gemini-2.5-flash-lite` only ($0.10 in / $0.40 out per million tokens) |
| Model requests | 20, each at most 8,000 input characters and `max_output_tokens` ≤ 1,024 (an agent's request with none gets 1,024), no images or files |
| Dollar cap | $0.005 of use a day; each request holds its most possible cost first |
| Decision calls (`/v1/systemone`, `/v1/decisions`, `/v1/classify`, `/v1/jobs`) | 20 |
| Other API calls | 500 |
| Circuit breakers (all free agents together) | $2.00 of model use and 2,000 decision calls a day |

**Cost ceiling.** A free request's most possible cost is about $0.0016
(12k input tokens counting framing, 1,024 output tokens); the per-agent
$0.005 cap is enforced on that held worst case, so one free agent can
never cost more than **$0.005 a day** in model use, and a typical day of
20 short requests costs about $0.002. All free agents together are capped
at **$2.00 a day**; past that every free request answers `402
agent_free_tier_full` until 00:00 UTC. With 500 sign-ups a day the cap,
not the sign-up count, bounds the spend.

Agents with at least $0.01 of credit aren't held to the decision and
API-call caps.

## The `402`

Admission refuses before anything is sent to a model, so a request the
agent can't afford fails at once (test:
`the_free_tier_ends_in_a_402_that_says_how_to_pay_then_credit_pays`). The
body is the inference error envelope with `type: insufficient_balance`, a
`code` (`agent_free_requests_used`, `agent_free_budget_used`,
`agent_free_tier_full`, `agent_payment_required`, `agent_balance_too_low`),
`free_tier` (terms, used, `resets_at`), `balance_usd`,
`request_max_cost_usd`, and `pay` (`available`, `rail: lightning`,
`methods[]` with `url`, `price_sats`, `credit_usd`). Over the decision or
API-call cap: `429 agent_daily_limit` with `Retry-After`.

## Paying (Lightning only)

The pay host (`docs/deployment/pay-host.md`) sells a top-up as a hosted
resource at `https://api.openagents.com/x/<resource>`: its `402`, its
invoice, its replay store. Once paid, it forwards the request to
`https://openagents.com/v1/agents/credit?account=…` with an
`OpenAgents-Paid` header signed by the pay host key. The gateway checks the
signature (`openagents_x402::hosted::verify_paid`), the resource name, and
that the payment hash wasn't credited before, appends the credit to
`agent-credit.jsonl` (fsynced), then adds it to the agent's balance in the
key book; at start the book is topped up from the journal if a crash came
between the two. Paid requests hold their worst case from the balance and
settle at the reported usage, any model.

Turning it on: register the resource on the pay front (`openagents x402
publish --upstream https://openagents.com/v1/agents/credit --price-sats N
--payout ADDRESS --resource openagents-agent-credit --method POST`) **before**
setting `AGENT_CREDIT_JSON` (`{"paid_key": "<GET api.openagents.com/v1/paid-key>",
"buy_url": "https://api.openagents.com/x/openagents-agent-credit",
"upstream_url": "https://openagents.com/v1/agents/credit", "price_sats": N,
"credit_usd_micros": M}`) on the gateway: a resource name belongs to the
key that registers it first, and the gateway trusts the name. Until then
`pay.available` is `false` and the `402` says buying credit isn't open yet.
The payout address for that resource is an owner decision
(`NEEDS_OWNER.md`).
