# Authenticate to the decision API

Every call to the gateway authenticates with a bearer credential. This
page covers what the credential looks like, where it may live, what a
rejected call returns, and how keys come into existence. It describes
what `crates/gateway` and the `oak` caller implement today — nothing
more.

## The credential

A key has the shape `oak_<id>.<secret>`. Send it in the `Authorization`
header and nowhere else:

```http
Authorization: Bearer oak_acme.9f…c4
```

An operator issues keys with the `tenant-keys` binary against the
deployment's registry; the service stores only a digest of each key.
A deployment that configures the `accounts` document in `gateway.json`
also offers self-serve issuance — `POST /v1/accounts` creates an
account, its personal workspace, its first key, and its first session
in one call, and the `/v1/workspaces/{workspace}/keys` family manages
named and scoped keys after that. A deployment without `accounts`
registers none of those routes — there is no signup flow, and the
operator issues keys.

## Where the key may live

`oak` and `oak-mcp` resolve the credential from exactly two places:

1. The `OPENAGENTS_API_KEY` environment variable.
2. The `api_key` field of a config file — by default
   `~/.config/openagents/oak.json`, overridable with `OPENAGENTS_CONFIG`
   or `--config`. The file must be mode `0600`; `oak` refuses a config
   file that group or others can read.

```json
{"api_key": "oak_acme.9f…c4", "base_url": "https://gateway.example.com",
 "model": "acme-kev", "workspace": "ws_…"}
```

The key never goes on the command line — there is no flag for it — never
in a URL or query string, never in a log line, never in a source file,
and never in an MCP tool argument; `oak-mcp` accepts no credential in
tool input.

`oak-mcp-http` adds one more channel: a caller may put its own
`Authorization: Bearer oak_…` on an MCP request, and the server forwards
it to the decision API for that call only — it is not stored, not tied
to the session, and not sent anywhere else. A request without the
header falls back to the operator credential above.

## Workspace membership

A deployment that sets `require_workspace_membership` also requires
exactly one `X-Workspace-Id` header per call. The workspace comes from
`--workspace`, `OPENAGENTS_WORKSPACE`, or the config file's `workspace`
field. A call without it is refused `400 workspace_required`; a
credential with no active membership in the named workspace and tenant
is refused `403 workspace_forbidden`. Monetary admission requires this
mode: a charge binds an authenticated workspace, never an anonymous or
bearer-only call.

## Session tokens

A session token has the shape `sess_<hex>` and goes in the same
`Authorization: Bearer` slot as a key. A deployment mints one only
through `POST /v1/sessions` (sign-in under an `oak_` key, or — when it
funds the anonymous lane — with no credential) and `POST /v1/accounts`
(self-serve sign-up). Neither token nor key ever appears in a URL.

A user session names its workspace with `X-Workspace-Id` on every
decision call and authorizes the account's fresh membership — a removed
member's session refuses on the next call. `DELETE /v1/session` logs
out; `GET /v1/session` describes standing and deadline.

The same token authorizes the member-scoped usage reads —
`GET /v1/workspaces/{id}/usage`, `/usage/activity`, `/usage/timeseries`,
`/usage/receipts/{digest}`, and `/usage/export` — and the dashboard
under `/dashboard`, which carries it in an `HttpOnly` cookie named
`oa_session` rather than an `Authorization` header.

## Anonymous calls

A request with no `Authorization` header is anonymous. Anonymous callers
reach only the deployment's `shared` doors — a tenant's dedicated doors
are invisible to them, not merely unreachable. When the deployment
requires workspace membership, an anonymous call is refused
`401 unauthenticated` outright.

When a deployment funds the anonymous lane (`accounts.anonymous` in
`gateway.json`), `POST /v1/sessions` with no credential mints a bounded
anonymous session: one unit of the operator-funded `onb_public` budget
per decision call, a per-session cap, a TTL, and shared doors only.
`require_workspace_membership` still refuses it — the lane exists only
where membership is optional.

## What a rejected call gets

Refusals are typed JSON, not bare status codes:

```json
{"error": {"code": "unauthenticated",
           "message": "the credential was refused: unknown key",
           "request": "req-…", "attempt": 1}}
```

The authentication-adjacent codes:

| Status | Code | Cause |
| --- | --- | --- |
| `400` | `malformed` | The `Authorization` header is not text. |
| `400` | `workspace_required` | Membership mode is on and no single `X-Workspace-Id` arrived. |
| `401` | `unauthenticated` | No `Bearer` shape, an unknown, revoked, or wrong key, or a membership-mode call with no credential. |
| `403` | `door_not_bound` | The credential holds no binding for the door the request named, and the door is not shared. |
| `403` | `workspace_forbidden` | The key has no active membership in the named workspace and tenant. |
| `401` | `session_closed` | The session expired, was logged out, or was revoked. |
| `403` | `out_of_scope` | The key's declared scopes do not name the door or the action attempted. |

Calls that pass through the owned admission path — `POST /v1/systemone`
and `POST /v1/classify` — also carry `x-request-id` and `x-attempt`
response headers; quote the request id when you report a call. The
durable-jobs family (`/v1/jobs`) authenticates the same way. The full
refusal-code table lives in the
[gateway service document](../decision-models/service/gateway.md) and the
[catalog](api-catalog.json).

Under a `billing` document, a decision call additionally names a
subscribed workspace whose plan covers the door — checked before
registry authorization and any quota or money reservation. An
unsubscribed or expired workspace answers `402 no_subscription` /
`subscription_expired`; a plan that does not list the door answers
`403 plan_excludes_model`. The billing management routes themselves —
`/v1/workspaces/{id}/billing/*` — are owner-only: the credential must
resolve to an account owning the workspace. `GET /v1/plans` and the
checkout-session read need no credential; `POST /v1/billing/webhook`
authenticates by HMAC signature, not by bearer key. See
[plans, checkout, and entitlements](../decision-models/service/billing.md).

## What needs no credential

The discovery documents and the documentation corpus are public by
design: `GET /v1/docs` and its `search`, `examples`, and `{id}` routes,
`GET /healthz`, and every discovery document under `/` (this page,
`api-catalog.json`, `openapi.yaml`, the agent card, the skills index)
answer without `Authorization`. A credential on a discovery call is
accepted and ignored. Public access to documents never grants access to
inference — `POST /v1/systemone`, `/v1/classify`, and `/v1/jobs` always
authenticate. Under `billing`, the plan catalog (`GET /v1/plans`) and
the checkout-session read (`GET /v1/billing/sessions/{id}`) are public
too — the catalog is the published price list, and the session read is
the browser's display-only return target.

## Rotation and revocation

By default key lifecycle — issue, rotate, revoke — is an operator act on
the registry directory with `tenant-keys`. A rotated key authenticates
the same tenant under a new secret; a revoked key fails
`unauthenticated`. Under monetary admission, balance belongs to the
workspace, so rotating a key changes nothing the account holds.

Under the `accounts` document the workspace's members manage their own
keys over HTTP: `POST /v1/workspaces/{workspace}/keys` issues a named,
optionally scoped key; `copy`, `pause`, `resume`, `rotate`, and `DELETE`
complete the lifecycle; and a member recovery token
(`POST /v1/workspaces/{workspace}/recovery` then `/v1/recovery/redeem`)
rotates a lost credential and ends the sessions it minted. Every
returned secret exists once — in the response that minted it; the store
holds digests and lineage only.

## Compatible doors

The native `POST /v1/systemone` contract is also served by compatible
backends directly — a local `kev-serve` or `laya-serve`, or TypeSafe's
hosted `api.typesafe.ai`, which takes its own `ts-` credential
(`TYPESAFE_API_KEY` in `crates/jev`). The gateway's own routes —
`/v1/classify`, `/v1/jobs`, `/v1/balance`, and the keyed `GET /v1/models`
view — exist only on the gateway.

## Related documents

- [skills.md](skills.md) — how to make a call once you hold a key.
- [api-catalog.json](api-catalog.json) — the routes a key can reach.
- [Caller guide](../decision-models/guides/caller.md) — the end-to-end caller contract.
- [Monetary accounting](../decision-models/service/monetary-accounting.md) — workspace membership and the conditional balance route.
- [Plans, checkout, and entitlements](../decision-models/service/billing.md) — the conditional billing surface.

---
Version 1.2.0 · generated-by: hand-maintained · 2026-09-22

VALIDATED: JSON examples parse; internal links resolve to repo paths. Exact commands are in the commit message.
