# Public route inventory and schema coverage

The public reference is [OpenAPI 3.1](https://api.openagents.com/v1/openapi.json).
Its server is `https://api.openagents.com`; every path includes `/v1`.
The SDK base URL remains `https://api.openagents.com/v1`. Do not add `/v1`
twice when generating a client. This reference covers qualified operations,
not every implemented route. Missing schemas do not mean missing handlers.

This inventory distinguishes the current source from the target design in
[API design](../api/design.md). It does not make internal routes public.
The [source index](route-inventory.json) records literal and constant web mounts,
the gateway's audience declarations, and the front's forwarding patterns.
Dynamic web mounts remain checked by `route_owners_tests`; the index does not
infer methods or invent contracts from path names. Each family below names
its owner and the qualification still needed.

## Routing and compatibility

[The production address map](../../deploy/production/api-url-map.yaml) sends
most `/v1/*` traffic to the web front. Exceptions are voice `/v1/sessions*`,
paid plugin invocation and resources, and older conversations/Sarah routes.
A route present on the gateway is not necessarily available on this address.
The web front forwards only [its explicit rules](../../crates/openagents-web/src/api_alias.rs).
Internal gateway account-session routes and public voice sessions are different
objects even where their path names overlap.

The two older addresses, `https://openagents.com/v1` and
`https://openagents.com/api/v1`, keep their existing handlers and deprecation
headers. Deprecation does not mean redirect or removal. The root website
`/openapi.json` serves the combined reference too. Gateway-local schemas and
the committed first-party schema retain their separate purposes.
No frozen Coder program, model ID, refusal code, or operation changes here;
see the [1.0 contract](../coder/1.0-freeze.md).

## Implemented families

Auth below names the issuing authority, not just a recognizable token prefix.
The account service must resolve a credential and the owning handler must check
account/workspace membership or resource access on every request. `oak_` keys
have operation-specific scopes; never assume that an app session and a decision
key have interchangeable authority.

| Family | Current owner and exposure | Auth and authorization | Public schema and remaining coverage |
| --- | --- | --- | --- |
| `/v1/models`, `/responses*`, `/chat/completions`, `/rates`, `/usage*`, `/key` | Gateway through front allowlist | Gateway account/API key; catalog reads are public where declared; stored responses and usage belong to the caller | Gateway inference schema; [inference contract](../inference/gateway.md). The front removes operations it does not forward. |
| `/v1/messages` | Existing compatibility/older upstream path; not a new inference schema operation | Owning service auth remains in force | Keep the 1.0 Anthropic-compatible address working; this inventory does not claim a qualified combined schema. |
| `/v1/decisions`, `/systemone` | Gateway/decision door through front | Account service key, subscribed workspace and door authorization; request admission and settlement apply | Decisions included; System One has its [own verified contract](../decision-models/service/gateway.md) and discovery schema. Combining that slice remains a gap. |
| `/v1/device/{code,token,sign-out}`, `/account` | Web device handlers and account discovery | No credential to request a code; single-use device code to poll; website GitHub session approves; issued app session or supported account credential for account reads | Qualified app schema and [auth evidence](auth-client-evidence.json). Browser approval stays outside the app API. |
| `/v1/keys` | Web sign-in-key management over account service | Account session/sign-in key; only its own account's keys | Implemented, not the workspace `oak_` key route. Public request/response coverage remains incomplete. |
| `/v1/account/access`, `/workspaces*`, `/invitations*` | Gateway public allowlist | Account service plus workspace role, scoped key and invitation checks | Gateway audience schemas include these operations. Some are summary schemas; field-level qualification remains incremental. |
| `/v1/agents/{challenge,register,me,claim,claimed,credit}`, `/agents/{account}/revoke` | Gateway public agent registration | Challenge/proof, agent credential, or account owner as declared by handler | Gateway schema. These are not the web activity list at `/v1/agents`. |
| `/v1/history*` | Web account-owned saved history | Account-bound app credential; owner or current shared-project membership for each read | Qualified [complete history](chat-history.md), including file/capture reads, unknown saved content and source-bound pagination. |
| `/v1/threads*`, `/agents`, `/agents/actions` | Web chat/phone APIs, sync routes, and sharing | App account, chat ownership/shared access; mutating or execution operations need their own authority | Implemented app-used routes; older first-party schema is a navigation aid, not full current public coverage. Reply, sync, activity, and shared-chat contracts need qualification. |
| `/v1/projects*`, `/files*` | Web projects, membership, files, sources, and uploads | App account plus project membership and file protection | Implemented; [projects](../api/projects.md) and owning tests describe current fields, cursors, file limits, and conflicts. Public schema coverage remains a gap. |
| `/v1/connections*`, project Drive sources | Web connected-service handlers and gateway account integration | Account credential plus connected-service grant; Google availability gate | Gated, not universally available. Preserve unavailable/permission outcomes. Schema coverage remains incomplete. |
| `/v1/computers*` | Web computer lifecycle over Computers service; existing device/sync handlers share the family | App account; lifecycle owner and idempotency checks; device activity has separate rights | Lifecycle and capabilities are in app schema. Commands/files and full capability coverage are #11802–#11805; existing device sync remains a separate contract. |
| `/v1/mac-jobs*`, `/own-runs*`, `/work*`, `/work-hosts*` | Web job/run handlers | Account ownership plus host/job authorization and deployment gates | Implemented app-used surfaces; no guessed client operations. Contracts and schema coverage need owning-consumer qualification. |
| `/v1/schedules*` | Web account schedule and sync handlers | App account and account-owned schedule store | Implemented app-used routes; payload, bounded sync/tombstones, and execution authority need separate schema qualification. |
| `/v1/w/*`, `/v1/actors/*` | Web actor runtime mounts | Runtime/workspace authorization; deployment-specific host grants | App/runtime surface, not an inferred general public SDK contract. Dynamic mounts need owning-runtime tests and a qualified audience before composition. |
| `/v1/sales`, backend `/v1/capabilities` | Separate loopback sales adapter and internal Computers-service call, respectively | Per-binding service bearer or trusted account-service call; not general app credential equivalence | These source names do not identify mounted public-front routes. Keep them out of generated public operations. |
| `/v1/traces*` | Web trace store | App/account authority and explicit share state; public trace reads differ from private ones | Implemented first-party routes; complete public payload/paging/file contracts remain a gap. |
| Workspace usage, balances, budgets, audit and team/SSO policy | Gateway with per-operation front rules | Workspace membership, role and key scope; SSO has its own proof requirements | Some gateway audience entries are qualified only as summary operations. Front exposure is narrower than gateway mounting. |
| Plans, checkout, billing portal, charges, earnings, referrals, reports, older skills/submissions | Gateway billing/account modules | Account/workspace roles or provider signature; internal and first-party audience declarations apply | Mounted where configured; a public gateway audience label alone does not expose the route through this front. Do not generate operations for blocked routes. |
| Paid `/v1/plugins/{id}/invoke`, `/v1/resources`, `/v1/paid-key`, `/x/*` | Separately routed pay front | Resource offer, payment proof, replay protection, and receipt checks | [Payment contracts](../payments/agent-payments.md); not yet composed into the combined reference. Paid effects must be qualified without making a payment as a documentation check. |
| `/v1/sessions*` on the public API host | Separately routed voice service | Voice session authority; not account-session creation or browser approval | Separate owner/protocol; no account-service operation may be generated for this path based on a gateway-local schema. |
| `/v1/conversations*`, `/v1/sarah/*` | Older API service via explicit address-map exceptions | That service's own auth and resource checks | Legacy service coverage remains unqualified in this public Rust schema. Keep routing compatibility; do not copy private implementations. |
| `/v1/openapi.json`, website docs, `llms.txt`, MCP docs and agent cards | Web discovery, gateway schema, and discovery crate | Public read-only discovery | The combined reference is canonical for the front. The decision-door schema and first-party schema retain distinct scopes; links do not grant route access. |

## Internal, gated, and proposed routes

Gateway `/v1/accounts`, `/session`, `/sessions/github`,
`/sessions/device/{lookup,decide,paired}`, `/account/github/token`,
`/admin/*`, classification and durable decision jobs are not public front
operations. Operator sign-up has an explicit staging-only switch; the normal
public schema must not advertise that exception. Provider webhooks also retain
their signature/authority boundary. Use [gateway audience declarations](../../crates/gateway/src/audience.rs)
and the front allowlist together, not either alone.

Managed-agent APIs in [the proposal](../api/managed-agents.md), new computer
provider admission, host SDK publication, and optional payments in the SDK are
not implemented merely because a design names their paths. Keep proposed,
configured/gated, implemented, and externally qualified states separate.

## Contract details and gaps

[Auth](README.md#first-desktop-delivery-sign-in-and-read-existing-chats),
[history](chat-history.md), and [Computers](computers.md) specify their actual
errors, limits, expiry, cursor behavior, and effects. Inference owns its SSE,
stored-response, request-size, metering, and payment contracts. Resource reads
must not allocate, wake a computer, execute a tool, or charge for an unrelated
operation. An operation's contract must state any intentional paid effect.

For uncovered families, existing handlers and owning tests are authoritative;
this inventory does not claim a stable JSON shape, pagination protocol,
idempotency guarantee, size bound, timeout, or streaming format. Qualify those
before exporting typed methods. Record at least request/response/event schemas,
auth issuer and scope, cross-account refusal, stable error codes, cursor/size and
concurrency limits, long-operation cancellation, retry identity, money effects,
and the Coder 1.0 aliases. Computers commands/files keep their independent issue
sequence; unrelated coverage is not a prerequisite for the first SDK client.

Incremental follow-ups are [#11857](https://github.com/OpenAgentsInc/openagents/issues/11857)
for project/thread/file/sign-in-key contracts and
[#11858](https://github.com/OpenAgentsInc/openagents/issues/11858) for the
verified public System One slice and discovery links. Separately served voice,
payments, legacy services, and configured work surfaces retain the coverage gaps
listed above; they need their owners' contracts before client generation.

## Composition and manual checks

The gateway keeps its existing schema generator. The web front normalizes its
relative paths to `/v1/*`, filters gateway operations with the production
forwarding rules, and merges the app-owned slice. The front's account GET
replaces the gateway account GET; other methods remain distinct. Conflicting
component definitions or missing/duplicate operation IDs produce `502`, rather
than a misleading reference. Components use owner prefixes for new slices.

The app fixture remains deterministic:

```sh
cargo run -p openagents-web --example app_openapi > docs/sdk/openapi.app.json
python3 scripts/api/public_reference.py --write-inventory
python3 scripts/api/public_reference.py --schema docs/sdk/openapi.app.json
python3 -m unittest discover -s scripts/api -p 'test_public_reference.py'
cargo test -p openagents-web app_openapi
cargo test -p openagents-web combined_reference_has_one
python3 scripts/api/public_reference.py --api https://api.openagents.com
```

Review regenerated inventory changes; source line changes can move entries.
Use `--api https://staging.openagents.com` for staging. These schema checks need
no credentials or account data. The [independent SDK example](../../examples/sdk-history/README.md)
and existing auth/history smokes exercise qualified handlers with synthetic
fixtures and clean them up. No GitHub workflow or package publication is needed.

The [deployment evidence](public-reference-evidence.json) records the qualified
source/image, staging and production revisions, 74 checked operations, legacy
aliases, unchanged model catalogs/browser assets, checks, and cleanup.
