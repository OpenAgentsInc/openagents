# Self-Hosted mdkd Sidecar Option

Date: 2026-06-07
Issue: #452 / OPENAGENTS-H-015

## Summary

OpenAgents product surface can use a self-hosted `mdkd` sidecar only as a Node/native-capable
service outside the Cloudflare Worker. The Worker remains the typed commerce
and policy authority. The sidecar may create or inspect MDK checkout/payment
state only after OpenAgents product surface has issued a checkout intent, challenge, policy decision,
and idempotency key.

Production update:
On 2026-06-07, OpenAgents product surface deployed the first live MDK checkout sidecar on
Cloudflare Containers. That deployed sidecar is a lighter wrapper around
`@moneydevkit/core` route handlers, not the full `mdkd` daemon described in
this option paper. The result still validates the main runtime decision in this
document: keep native/Node MDK runtime outside the Worker, keep the Worker as
the typed authority, and use a Cloudflare binding to reach the sidecar.

Implemented OpenAgents product surface contract:

```text
workers/api/src/mdk-sidecar-option.ts
workers/api/src/mdk-sidecar-option.test.ts
workers/api/src/config.ts
services/mdk-sidecar
workers/api/src/index.ts
workers/api/wrangler.jsonc
```

New config discriminator:

```text
MDK_CHECKOUT_ROUTE_KIND=fake_provider | hosted_platform | self_hosted_mdkd_sidecar
```

The existing route fields remain the call boundary:

```text
MDK_CHECKOUT_ROUTE_URL
MDK_CHECKOUT_ROUTE_SECRET
MDK_CHECKOUT_PATH_BASE
MDK_CHECKOUT_PROVIDER_REF
MDK_CHECKOUT_CONFIG_REF
MDK_CHECKOUT_CREDENTIAL_BINDING_REF
MDK_CHECKOUT_WEBHOOK_SOURCE
MDK_CHECKOUT_WEBHOOK_SECRET or MDK_WEBHOOK_SECRET
MDK_CHECKOUT_WEBHOOK_BINDING_REF
```

This issue does not deploy `mdkd`, start a native node, or grant payment
dispatch authority.

## Modes

| Mode | Meaning | Production claim |
| --- | --- | --- |
| `fake_provider` | Contract tests and non-live fixtures only. | No live checkout or payment. |
| `hosted_platform` | OpenAgents product surface Worker calls an MDK-compatible hosted platform route. | Live only when route and webhook config are verified. |
| `self_hosted_mdkd_sidecar` | OpenAgents product surface Worker calls an operator-owned `mdkd` sidecar or wrapper route. | Live only when sidecar health, route, auth tiers, storage, and webhook verification pass. |

## Worker Compatibility Decision

Do not import native MoneyDevKit runtime into the Cloudflare Worker.

The local `mdkd` source is Rust/native and depends on `ldk-node`, local wallet
state, SQLite invoice metadata, VSS-backed state, Basic Auth, webhook signing,
and long-running node behavior. That belongs in a sidecar, SHC node, Cloud Run
service, or other Node/native-capable runtime.

The Worker should call the sidecar through one of these boundaries:

- Cloudflare service binding if the sidecar is another Worker-compatible
  wrapper;
- Cloudflare VPC Service binding or Tunnel when the sidecar is private
  infrastructure;
- a tightly scoped HTTPS route with a route secret when the sidecar must be
  externally reachable; or
- SHC-local operator routing for development and private smokes.

Cloudflare's current Worker guidance prefers service bindings for
Worker-to-Worker calls and VPC bindings/Tunnel for private services. Public
HTTP is the fallback, not the ideal path.

The live 2026-06-07 checkout smoke uses Cloudflare Containers:

```text
Worker /api/mdk
-> MDK_SIDECAR Durable Object binding
-> MdkSidecarContainer
-> services/mdk-sidecar/src/server.mjs
-> @moneydevkit/core route handler
```

This avoids GCP for the checkout sidecar while still respecting the Worker
compatibility boundary. The Container is allowed to carry the Node/native MDK
runtime; generated Sites, public browser code, D1 projections, and Forum posts
are not.

## mdkd Source Shape

Local source audited:

```text
/Users/christopherdavid/work/projects/moneydevkit/repos/mdkd
revision 9ffea5f
```

Relevant `mdkd` behavior:

- accepts secrets through file descriptors, with environment fallback;
- requires platform access token, wallet recovery material, read-only Basic
  Auth password, full-control Basic Auth password, and webhook HMAC key;
- serves `/scalar` OpenAPI docs;
- stores local invoice/payment metadata in SQLite under the configured storage
  directory and network;
- uses VSS-backed state for wallet/node backup behavior;
- exposes read-only status and invoice lookup APIs;
- exposes full-control APIs for invoice creation and payment sends;
- emits HMAC-signed webhook events for payment received and invoice expiry;
- can run signet or mainnet; and
- is not a Cloudflare Worker runtime.

## Required Auth Tiers

The sidecar option must keep these authorities separate:

| Tier | Purpose | Where it belongs |
| --- | --- | --- |
| Read-only status auth | Health, balance-readiness bucket, invoice/payment status lookup. | Sidecar private route or binding. |
| Checkout/control auth | Create checkout/invoice or confirm checkout through the sidecar. | Sidecar private route or binding. |
| Payout control auth | Any send/pay endpoint. | Nexus/Treasury gated path only, never customer/Site checkout. |
| Webhook verification | Verify exactly one configured event source. | OpenAgents product surface webhook verifier plus sidecar HMAC/dashboard source config. |
| Emergency pause | Disable sidecar checkout/control/payout calls immediately. | Operator-owned control and config projection. |

The typed projection in `mdk-sidecar-option.ts` requires all five auth tiers
for `self_hosted_mdkd_sidecar` readiness. Hosted platform mode requires
checkout/control auth, webhook verification, and emergency pause. Fake mode
does not claim live readiness.

## Secret Boundary

Do not commit, print, export into public docs, store in D1 public projection,
or paste into issue comments:

- MDK platform token;
- wallet recovery material;
- read-only Basic Auth password;
- full-control Basic Auth password;
- webhook HMAC key;
- raw wallet home path;
- SQLite file path;
- VSS payloads;
- raw invoices;
- payment hashes;
- preimages;
- payment destination strings; or
- sidecar route secrets.

`mdkd` prefers file-descriptor secret passing. If OpenAgents product surface uses Cloudflare Worker
secrets for route-level calls, the Worker receives only the route secret and
webhook verification secret. It should not receive wallet recovery material or
full-control sidecar passwords unless a later issue explicitly creates and
audits that authority boundary.

## Storage And Backup

The sidecar must own:

- network-scoped wallet/node state;
- local SQLite invoice and outgoing-payment metadata;
- VSS-backed state, when enabled;
- backup/restore runbook;
- log retention and redaction;
- rotation procedure for sidecar auth tiers; and
- clear operator status for wallet readiness.

OpenAgents product surface stores only redacted refs:

- service ref;
- route binding ref;
- version ref;
- wallet-readiness bucket ref;
- storage refs;
- observability refs;
- health refs;
- reconciliation lag refs;
- webhook binding refs; and
- checkout/provider refs produced by the existing Site commerce ledger.

## Health And Observability

Sidecar readiness should project:

- health status: `healthy`, `degraded`, `unknown`, or `unreachable`;
- route configured state;
- current `mdkd` version ref;
- wallet readiness bucket;
- status lookup availability;
- checkout creation availability;
- webhook verifier source;
- reconciliation lag bucket;
- emergency pause state;
- storage/VSS backup readiness;
- failure classes; and
- operator action refs.

Do not expose:

- exact wallet balance;
- raw payment route;
- local file path;
- provider token;
- Basic Auth value;
- raw webhook body;
- raw invoice;
- payment hash; or
- preimage.

## OpenAgents product surface Call Boundary

The OpenAgents product surface Worker-side hosted MDK client already posts MDK-compatible route
payloads through `OpenAgentsHostedMdkRouteClientRuntime`:

```text
create_checkout -> { handler: "create_checkout", params: ... }
get_checkout    -> { handler: "get_checkout", checkoutId: ... }
```

The sidecar or platform route must accept that contract and return the same
redactable checkout/status shape used by `hosted-mdk-client.ts`.

The sidecar route must not accept raw customer prompts, raw Site source,
private repository material, or arbitrary payout destination strings. OpenAgents product surface
must perform product/catalog lookup, spend-cap checks, customer-data redaction,
idempotency, and payment-destination classification before calling the
sidecar.

## Webhook Source Selection

Do not use a generic "MDK webhook" model. Pick exactly one configured source:

- `dashboard_standard_webhooks`;
- `daemon_invoice_hmac`; or
- `sdk_node_control`.

OpenAgents product surface already models this through `MDK_CHECKOUT_WEBHOOK_SOURCE` and
site-commerce reconciliation. The sidecar runbook must record which source is
enabled, which secret or signature scheme verifies it, and which replay key is
stored.

## Emergency Pause

If emergency pause is active, OpenAgents product surface must treat sidecar readiness as
`blocked_emergency_pause`, even if health and route config look good.

Pause must block:

- checkout creation;
- checkout confirmation;
- sidecar payout calls;
- retry loops; and
- any operator automation that would mutate wallet or payment state.

Read-only status may remain available if the read-only tier is explicitly
allowed during pause.

## Live And Not-Live Boundaries

Live as of 2026-06-07:

- Cloudflare Worker route `POST /api/mdk`;
- Cloudflare Durable Object binding `MDK_SIDECAR`;
- Cloudflare Container class `MdkSidecarContainer`;
- MDK checkout core signed ping through `https://openagents.com/api/mdk`;
- production amount checkout creation for `100` bitcoin sats; and
- payment of that checkout from a local MDK agent wallet, with provider status
  observed as `PAYMENT_RECEIVED`.

Still not live:

- full `mdkd` daemon deployment;
- sidecar wallet storage/VSS backup management;
- sidecar read-only/full-control Basic Auth tiering;
- daemon invoice HMAC webhook source;
- product-mode dashboard checkout from a stable MDK product ID;
- public demo page wired to the live sidecar path; and
- operator restart/rotation controls for long-lived container instances.

Before claiming production readiness, OpenAgents still needs:

- an operator-owned sidecar lifecycle runbook;
- file-descriptor or equivalent secret injection for any future full `mdkd`
  daemon path;
- exact webhook-source verification;
- a Site-commerce route smoke against OpenAgents product surface's hosted MDK client contract, not
  only a direct `/api/mdk` sidecar smoke;
- storage/VSS backup proof;
- emergency pause smoke;
- redacted observability proof; and
- a decision on whether any future daemon route uses private binding/VPC/Tunnel
  or public HTTPS.

## Verification Added

Focused tests:

```bash
bun run --cwd workers/api test -- src/mdk-sidecar-option.test.ts src/config.test.ts
```

The tests prove:

- a self-hosted sidecar can be marked ready only with route, health, auth,
  storage, version, wallet-readiness, and pause refs;
- hosted platform and fake provider modes are distinct;
- missing route/auth/storage, pause, and unreachable health block readiness;
- projections preserve Worker compatibility and keep payout dispatch disabled;
- raw secret, invoice, wallet path, and payment material are rejected; and
- `MDK_CHECKOUT_ROUTE_KIND` decodes through the Worker config boundary.
