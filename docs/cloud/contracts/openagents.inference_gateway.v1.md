# `openagents.inference_gateway.v1`

Status: design contract for Model-2 managed inference resale

This contract describes the Model-2 inference gateway path: a customer buys
OpenAgents credits, runs a managed Cloud workroom, and sends model inference
requests to an OpenAgents-operated gateway. OpenAgents routes those requests
through OpenAgents' own commercial provider API keys and charges prepaid
credits on a cost-plus-10% basis.

This is the inverse of `openagents.byo_credential_broker.v1`: the customer
does not bring or receive a provider API key. OpenAgents holds the commercial
provider account, brokers access through its control plane, meters each
request, and retains only refs, policy decisions, and redacted usage facts.

## Purpose

The Model-2 inference gateway lets managed Cloud workrooms consume model
capacity without placing provider credentials inside the customer session VM.
It defines:

- where OpenAgents-owned commercial API keys live;
- how a customer session VM reaches model inference;
- the per-request metering record used for prepaid credit-ledger drawdown;
- the pricing basis for direct providers and gateway providers such as
  OpenRouter;
- the refs-only artifact, log, receipt, and projection boundary;
- the policy boundary between API-inference resale and subscription resale.

## Invariant Binding

This contract is a concrete application of `INVARIANTS.md` capability,
secret-handling, wallet, settlement, and workroom-lifecycle rules:

- Workrooms consume model capability through a gateway, not raw provider
  secrets on disk.
- OpenAgents' own commercial provider API keys live only in the control-plane
  secret broker. They must never be placed on the workload host, session VM,
  `oa-workroomd` filesystem, runner environment, artifact bundle, log stream,
  receipt, trace, callback, fixture, tracked file, or closeout manifest.
- The customer session VM talks only to an egress-locked
  OpenAgents-operated inference gateway. It must not call the provider
  directly for Model-2 inference.
- Every request is scoped, policy-checked, metered, auditable, and tied to a
  workroom, run, org, project, customer credit account, and gateway policy.
- Secret access by the gateway produces redacted evidence that can be audited
  without leaking provider API keys or provider account secrets.
- Durable projections retain refs only: no raw keys, no provider bearer
  tokens, no prompts, no completions, and no provider request or response
  bodies.
- Credit drawdown is prepaid and meter-derived. Workrooms do not receive
  wallet authority, ledger write credentials, payment credentials, or raw
  accounting credentials.

The `danger_full_access` invariant still applies to any externally isolated
VM/container profile using this gateway: the workload may have network access
only through the declared egress-locked gateway path and receives no broad
host, cloud, wallet, settlement, or provider credentials.

## Actors

| Actor | Role |
| --- | --- |
| Customer / Pylon client | Buys OpenAgents credits, starts the workroom, and receives refs-only usage and balance projections. Does not receive OpenAgents provider keys. |
| openagents.com credit ledger | Product authority for prepaid credit balance, reservation, drawdown, and customer-visible usage projections. |
| Cloud control plane | Allocates the workroom, issues gateway access refs, resolves provider routing policy, and authorizes secret broker use. |
| Control-plane secret broker | Sole custody location for OpenAgents-owned commercial provider API keys and gateway-provider credentials. |
| Managed node | Starts the isolated session VM and enforces that Model-2 egress targets the OpenAgents gateway only. |
| Isolated session VM | Sends model requests to the OpenAgents inference gateway using a session-scoped gateway token or mTLS identity, never a provider key. |
| `oa-workroomd` | Enforces local egress policy, exposes only the declared model gateway endpoint, and emits redacted gateway access receipts. |
| OpenAgents inference gateway | Authenticates workrooms, retrieves provider credentials from the secret broker, routes requests, meters usage, and emits refs-only receipts. |
| Model provider or upstream gateway | Commercial API endpoint such as OpenAI, Anthropic, Google, or OpenRouter. Receives OpenAgents' provider credential from the gateway only. |

## Gateway Session Fields

`openagents.inference_gateway_session.v1` records policy and routing metadata
for a managed Model-2 inference session. It is refs-only and must never contain
raw provider credentials, encrypted provider credentials, provider key digests,
prompts, completions, or provider request/response bodies.

| Field | Purpose |
| --- | --- |
| `workroom_id` | Private workroom receiving the inference capability. |
| `run_ref` | Bounded run, task, or session scope. |
| `organization_ref` / `project_ref` / `user_ref` | Non-secret customer scope selected by the product authority. |
| `credit_account_ref` | Non-secret prepaid credit ledger account ref. |
| `gateway_session_ref` | Random non-secret session ref for this gateway attachment. Not derived from credentials, prompts, or completions. |
| `gateway_endpoint_ref` | Ref for the OpenAgents-operated inference gateway endpoint reachable from the session VM. |
| `gateway_auth_ref` | Session-scoped gateway auth ref or mTLS identity ref. Not a provider credential. |
| `egress_policy_ref` | Policy ref requiring model egress through the OpenAgents gateway only. |
| `provider_policy_ref` | Routing policy ref for permitted provider families and models. |
| `pricing_policy_ref` | Policy ref declaring `cost_plus_10pct_metered_api_gateway_cost`. |
| `ledger_reservation_ref` | Optional prepaid credit reservation ref for admission or request batching. |
| `receipt_sink_ref` | Ref where redacted gateway and metering receipts are delivered. |
| `issued_at_ms` / `expires_at_ms` | Session TTL. Must not outlive the workroom run or VM lease. |

## Credential Custody

OpenAgents-owned commercial provider credentials are control-plane secrets.
They may be referenced by policy but are never materialized on the workload
host.

Allowed custody path:

1. An operator or provider-account automation installs an OpenAgents-owned
   provider credential into the control-plane secret broker.
2. The secret broker returns a non-secret provider account ref, such as
   `provider-account://...`, for routing policy.
3. The inference gateway asks the secret broker for a short-lived credential
   materialization only inside the gateway execution boundary.
4. The gateway attaches the credential to the upstream provider request.
5. The gateway clears the materialized credential from request-local memory
   after the provider call completes or fails.

Forbidden custody paths:

- storing OpenAgents provider keys on the managed node;
- storing OpenAgents provider keys inside the session VM;
- injecting OpenAgents provider keys into workload process environment
  variables;
- writing OpenAgents provider keys to `.env`, shell rc, SDK config, auth cache,
  artifact, trace, callback, receipt, fixture, screenshot, or normal log;
- returning OpenAgents provider keys, key prefixes, key suffixes, key hashes,
  authorization headers, or secret-store payloads to Pylon, the customer VM, or
  any retained projection.

Provider secret refs are permitted only inside control-plane and gateway
policy records with restricted visibility. They must not appear in
customer-visible receipts or public-facing projections.

## Egress Contract

For Model-2 inference the customer session VM is egress-locked to the
OpenAgents-operated gateway.

- The VM may call only the declared `gateway_endpoint_ref` for model
  inference.
- Direct egress from the VM to model providers or upstream gateways is denied.
- DNS, HTTP proxy, NAT, firewall, and local gateway policy must agree on the
  same allow-list.
- `oa-workroomd` may expose a local convenience endpoint, such as
  `/openagents/model`, but that endpoint must forward only to the declared
  OpenAgents inference gateway.
- The VM authenticates to the gateway with `gateway_auth_ref`, such as a
  short-lived session token or mTLS identity minted for the workroom. This
  credential authorizes gateway access only and is not accepted by the model
  provider.
- Failed direct-provider attempts emit redacted `egress.denied` or
  `model.gateway.denied` events with refs and reasons only.

The intended capability name is:

```text
model.openagents_inference_gateway
```

It may be listed under the `model` link-local gateway allow-list. Gateway
access continues to emit redacted gateway-access events with gateway,
capability, decision, reason, and refs only.

## Request Flow

1. The control plane verifies that the customer has an eligible prepaid credit
   account and that the workroom policy permits Model-2 inference.
2. The credit ledger may reserve a bounded amount of prepaid credit for the
   session or for a request batch. A reservation is not a charge.
3. The control plane returns `gateway_session_ref`, `gateway_endpoint_ref`,
   `gateway_auth_ref`, egress policy refs, provider policy refs, and TTLs to
   the node and session VM.
4. The session VM sends inference requests only to the OpenAgents gateway.
5. The gateway authenticates the session, checks workroom policy, model
   policy, credit availability, and rate limits.
6. The gateway selects a provider route, retrieves the corresponding
   OpenAgents-owned credential from the control-plane secret broker inside the
   gateway boundary, and sends the upstream request.
7. The gateway captures provider, model, status, input token count, output
   token count, and upstream cost evidence when available.
8. The gateway emits a redacted metering receipt and submits the credit-ledger
   drawdown input.
9. The gateway returns the provider response to the session VM but does not
   persist raw prompts, completions, tool payloads, or provider bodies.

If provider token counts are unavailable, the request must produce an explicit
`count_source = unavailable` metering record with `unavailable_reason`. Silent
missing token usage is not acceptable proof for prepaid drawdown.

## Per-Request Metering

Every gateway request emits an
`openagents.inference_gateway_metering.v1` record.

| Field | Purpose |
| --- | --- |
| `metering_ref` | Stable opaque ref for this metered request, e.g. `igw://meter/sha256:...`. |
| `gateway_session_ref` | Gateway session that authorized the request. |
| `workroom_id` / `run_ref` | Workroom and bounded run scope. |
| `organization_ref` / `project_ref` / `user_ref` | Non-secret customer scope refs. |
| `credit_account_ref` | Prepaid ledger account ref. |
| `ledger_reservation_ref` | Reservation consumed, if any. |
| `ledger_drawdown_ref` | Credit-ledger drawdown ref emitted after pricing. |
| `provider_kind` | Provider family such as `openai`, `anthropic`, `google`, `openrouter`, or `custom_gateway`. |
| `provider_route_ref` | Non-secret route ref selected by policy. Must not be a secret-store payload. |
| `model` | Provider model identifier requested or resolved by the gateway. |
| `mode` | `chat`, `responses`, `embeddings`, `rerank`, `image`, `audio`, or other bounded inference mode. |
| `request_status` | `accepted`, `completed`, `provider_error`, `policy_denied`, `credit_denied`, `rate_limited`, or `failed`. |
| `input_tokens` | Provider-reported or gateway-counted input tokens, nullable only when unavailable. |
| `cached_input_tokens` | Cached input token count when exposed by provider, otherwise `null`. |
| `output_tokens` | Provider-reported or gateway-counted output tokens, nullable only when unavailable. |
| `reasoning_tokens` | Provider-reported reasoning token count when exposed, otherwise `null`. |
| `total_tokens` | Sum used for billing evidence when available. |
| `count_source` | `provider_reported`, `gateway_counted`, `parsed_from_stream`, `estimated`, or `unavailable`. |
| `upstream_cost_microusd` | Metered provider or upstream gateway API cost before markup, nullable only when unavailable at receipt time. |
| `gateway_cost_microusd` | OpenAgents gateway-specific metered cost component, if separately measured; `0` when no separate component applies. |
| `cost_input_microusd` | Nullable billing input: metered API/gateway cost x 1.10. |
| `cost_input_basis` | `cost_plus_10pct_metered_api_gateway_cost` when populated; `unavailable` otherwise. |
| `pricing_policy_ref` | Pricing policy applied to the request. |
| `emitted_at_ms` | Metering receipt emission time. |
| `receipt_digest` | `sha256:` digest over the redacted metering record. |

The metering record must not include raw prompts, completions, tool call
arguments, provider request bodies, provider response bodies, provider
authorization headers, provider API keys, gateway request authorization token
values, or customer private data.

## Pricing Principle

Model-2 inference billing inputs follow a **cost-plus-10%** principle. The
`cost_input_microusd` for a completed request is:

```text
cost_input_microusd = floor((upstream_cost_microusd + gateway_cost_microusd) x 1.10)
```

where:

- `upstream_cost_microusd` is OpenAgents' metered provider API cost for the
  request;
- for an upstream gateway such as OpenRouter, `upstream_cost_microusd` is
  OpenAgents' metered gateway bill for that request;
- `gateway_cost_microusd` is any separately measured OpenAgents gateway
  execution, network, or surcharge cost component included by policy;
- the 10% markup applies to the metered API/gateway cost whether OpenAgents
  calls the model provider directly or routes through an upstream gateway.

`cost_input_microusd` is the billing input passed to the prepaid credit ledger.
It is not a subscription charge and not a postpaid invoice. The ledger draws
down prepaid OpenAgents credits using the pricing policy and its own rounding
rules.

When provider or upstream gateway cost is unavailable at request closeout,
`cost_input_microusd` must be `null` and `cost_input_basis` must be
`unavailable`. A reconciliation pass may later emit a corrected drawdown input
when trustworthy cost evidence arrives. The gateway must not substitute
subscription-plan value, flat-rate guesses, manually entered figures, or
provider-list-price estimates unless a future policy explicitly allows that
mode and tests cover it.

## Credit-Ledger Drawdown

The prepaid ledger receives only metered drawdown inputs and refs.

Allowed drawdown input fields:

```text
credit_account_ref
ledger_reservation_ref
gateway_session_ref
metering_ref
provider_kind
model
input_tokens
output_tokens
total_tokens
cost_input_microusd
cost_input_basis
pricing_policy_ref
receipt_digest
```

Forbidden drawdown input fields:

```text
raw prompt
completion
tool arguments
provider request body
provider response body
provider API key
provider Authorization header
gateway auth token value
provider account secret payload
customer private data
```

A request may be denied before provider dispatch when the credit account has
insufficient available prepaid balance. Credit denials emit refs-only
`credit_denied` receipts and must not call the upstream provider.

## Receipt Fields

`openagents.inference_gateway_receipt.v1` records redacted evidence for the
gateway lifecycle.

| Field | Purpose |
| --- | --- |
| `receipt_kind` | `session_prepared`, `access_checked`, `request_metered`, `ledger_drawdown_submitted`, `egress_denied`, `released`, or `refused`. |
| `workroom_id` / `run_ref` | Workroom and bounded run scope. |
| `gateway_session_ref` | Non-secret session ref. |
| `gateway_endpoint_ref` | Gateway endpoint ref, not raw network secrets. |
| `credit_account_ref` | Prepaid ledger account ref. |
| `provider_kind` | Provider family, not account secret. |
| `provider_route_ref` | Non-secret route ref selected by policy. |
| `model` | Model identifier involved in the decision or request. |
| `metering_ref` | Per-request metering ref when applicable. |
| `ledger_drawdown_ref` | Credit-ledger drawdown ref when applicable. |
| `gateway_policy_ref` | Gateway policy ref used for local and control-plane decisions. |
| `decision` | `accepted`, `denied`, `released`, `submitted`, or `failed`. |
| `reason` | Redacted bounded reason string. |
| `evidence_digest` | Digest over non-secret receipt evidence only. Not a digest of prompts, completions, credentials, or provider bodies. |
| `emitted_at_ms` | Receipt emission time. |
| `receipt_digest` | Local `sha256:` digest over the redacted receipt material. |

Receipts must not contain:

- OpenAgents-owned provider API keys;
- encrypted provider credential payloads;
- provider key hashes, prefixes, suffixes, or fingerprints;
- provider authorization headers or bearer tokens;
- gateway auth token values;
- raw prompts, completions, tool calls, tool outputs, or provider bodies;
- process environment values;
- provider account secrets or secret-store payloads;
- customer wallet, payment, or raw ledger credentials.

## Retained Projections

Any durable projection retained by Cloud, openagents.com, Forge, Nexus,
Autopilot, Vortex, Probe, or receipt sinks is refs-only.

Allowed retained projection fields:

```text
workroom_id
run_ref
gateway_session_ref
credit_account_ref
ledger_reservation_ref
ledger_drawdown_ref
provider_kind
provider_route_ref
model
input_tokens
cached_input_tokens
output_tokens
reasoning_tokens
total_tokens
count_source
cost_input_microusd
cost_input_basis
pricing_policy_ref
gateway_policy_ref
receipt_digest refs
release_status
```

Forbidden retained projection fields:

```text
raw provider API key
encrypted provider credential
provider key digest or fingerprint
provider key prefix or suffix
Authorization header value
gateway auth token value
process environment value
raw prompt
completion
tool arguments
tool outputs
provider request body
provider response body
provider SDK config containing a key
secret-store payload
customer payment credential
wallet seed or private key
```

Raw prompts and completions may pass through the gateway only as transient
request/response data needed to serve the session VM. They must not be stored
in any artifact, log, receipt, trace, callback, fixture, tracked file,
closeout manifest, resource usage receipt, screenshot, durable event payload,
or customer-visible usage projection.

## Policy Classification

Model-2 inference gateway usage is **API-inference resale**. The customer buys
OpenAgents credits and OpenAgents sells metered access to API inference through
OpenAgents-operated infrastructure, with OpenAgents' own commercial provider
keys held in the control-plane secret broker.

This path is **not subscription resale**:

- it must not resell ChatGPT, Claude, Gemini, or other end-user subscription
  seats;
- it must not share, broker, tunnel, or multiplex subscription cookies,
  browser sessions, device tokens, OAuth grants, or user subscription auth;
- it must not price usage from a subscription plan unless a future policy
  explicitly creates a compliant API-inference mechanism with test coverage;
- subscription-backed workrooms remain governed by their own auth-grant and
  unavailable-usage contracts, not by this Model-2 credit drawdown path.

The openagents.com Provider Capacity Marketplace Gate owns the product-policy
hook for this classification. Model-2 API-inference resale is allowed only
under that gate's `future policy + tests` hook: any expansion to new provider
classes, upstream gateways, pricing bases, or subscription-adjacent capacity
must update the future policy and add tests before admission.

## Runner Behavior

`oa-workroomd` writes local gateway and egress evidence to:

```text
gateway-access.jsonl
inference-gateway-receipts.jsonl
egress-denials.jsonl
resource-usage-receipts.jsonl
```

The OpenAgents inference gateway writes redacted metering and lifecycle
receipts to the configured `receipt_sink_ref`. At session closeout,
`oa-workroomd` may append refs to the related
`openagents.resource_usage_receipt.v1` `model_usage` block, but raw prompts,
completions, provider bodies, and provider credentials remain forbidden.

Gateway routing emits the following `openagents.runner_event.v1` events:

- `model.gateway.session.prepared` when a gateway session is attached;
- `model.gateway.access.checked` for local allow/deny decisions;
- `model.gateway.request.metered` when a per-request metering record is
  emitted;
- `model.gateway.ledger.drawdown.submitted` when prepaid credit drawdown input
  is sent;
- `model.gateway.egress.denied` when direct-provider egress is blocked;
- `model.gateway.session.released` at closeout or revocation.

Artifact and closeout receipts remain separate. This contract covers only
Model-2 inference gateway custody, routing, metering, pricing, and refs-only
projection facts.

## Validation Rules

- `gateway_session_ref`, `metering_ref`, `ledger_reservation_ref`,
  `ledger_drawdown_ref`, and `receipt_digest` must be bounded non-secret refs.
- `expires_at_ms` must be positive and no later than the run or VM lease TTL.
- `gateway_endpoint_ref` must identify an OpenAgents-operated gateway endpoint.
- Egress policy must deny direct VM egress to model providers and upstream
  gateways for Model-2 inference.
- Provider credentials must resolve only inside the control-plane secret
  broker and gateway execution boundary, never on the managed node or workload
  host.
- `provider_kind` and `model` must be permitted by `provider_policy_ref`.
- `credit_account_ref` must be present for billable requests.
- `request_status = credit_denied` must have no upstream provider dispatch.
- `input_tokens`, `output_tokens`, and `total_tokens` must be non-negative
  when present.
- `count_source = unavailable` requires `unavailable_reason` in the associated
  runner event or metering context.
- `cost_input_microusd`, when not `null`, must equal
  `floor((upstream_cost_microusd + gateway_cost_microusd) x 1.10)`.
- `cost_input_basis` must be
  `cost_plus_10pct_metered_api_gateway_cost` when `cost_input_microusd` is
  populated and `unavailable` otherwise.
- Ledger drawdown inputs must cite a metering receipt digest and must not carry
  prompts, completions, credentials, provider bodies, or raw payment data.
- Every receipt, event payload, callback, artifact candidate, and retained
  projection must pass forbidden-secret and forbidden-content marker filters
  before persistence.
- API-inference resale classifications must pass the openagents.com Provider
  Capacity Marketplace Gate policy and tests before admission.

## Non-Goals

- No customer BYO key transfer; that is
  `openagents.byo_credential_broker.v1`.
- No OpenAgents provider key on the workload host, managed node, session VM, or
  runner environment.
- No direct provider egress from the customer session VM.
- No retained raw prompts, completions, provider request bodies, or provider
  response bodies.
- No subscription resale, subscription seat sharing, browser-session resale, or
  subscription-auth multiplexing.
- No wallet authority or raw settlement credential sharing with the workroom.
- No replacement for `openagents.resource_usage_receipt.v1`; gateway metering
  may feed that receipt family by ref, but remains a separate contract.
