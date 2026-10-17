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
| `/v1/decisions`, `/systemone` | Gateway/decision door through front | Account service key, subscribed workspace and door authorization; request admission and settlement apply | Both are included with distinct request shapes. System One uses a [generated public slice](openapi.systemone.json) from the [owning decision contract](../decision-models/api/openapi.yaml); classification and jobs remain standalone-gateway operations. |
| `/v1/device/{code,token,sign-out}`, `/account` | Web device handlers and account discovery | No credential to request a code; single-use device code to poll; website GitHub session approves; issued app session or supported account credential for account reads | Qualified app schema and [auth evidence](auth-client-evidence.json). Browser approval stays outside the app API. |
| `/v1/keys` | Web sign-in-key management over account service | Live account session/sign-in key; cookies and `oak_` credentials cannot manage keys | Qualified list/create/revoke schemas for sign-in and owner-workspace API keys. See [key management](#account-key-management); workspace key administration is separate. |
| `/v1/account/access`, `/workspaces*`, `/invitations*` | Gateway public allowlist | Account service plus workspace role, scoped key and invitation checks | Gateway audience schemas include these operations. Some are summary schemas; field-level qualification remains incremental. |
| `/v1/agents/{challenge,register,me,claim,claimed,credit}`, `/agents/{account}/revoke` | Gateway public agent registration | Challenge/proof, agent credential, or account owner as declared by handler | Gateway schema. These are not the web activity list at `/v1/agents`. |
| `/v1/labs*` | Shared `lab-api` handlers mounted by the web front | Fresh account credential, current project role, and per-kind availability; experimental kinds use the Computers account gate | Generated [Labs contract](openapi.labs.json), [limits and retries](labs.md); CAD and other kind migrations follow their Labs issues. |
| `/v1/history*` | Web account-owned saved history | Account-bound app credential; owner or current shared-project membership for each read | Qualified [complete history](chat-history.md), including file/capture reads, unknown saved content and source-bound pagination. |
| `/v1/threads*`, `/computers/check-in`, `/computers/{name}/sync` | Web chat/phone APIs, computer sync, and sharing | App account, owner or current shared-project access; replies check role and use the sender’s allowance | Qualified [thread contracts](threads.md) cover lists, messages, move/delete, attachments, replies, sharing, sync and destructive reply delivery. The [cloud chat client](chats.md) adds public JSON events and request-specific stop; browser HTML routes remain compatible. |
| `/v1/agents`, `/agents/actions` | Web activity and supervision | App account and account-owned computer work | Activity/action schemas remain a separate qualification scope. |
| `/v1/projects*`, `/files*` | Web projects, membership, files, sources, and uploads | App session/sign-in key plus current project role and file protection; these web handlers reject `oak_` credentials | Project lifecycle, repositories, membership and invitations have source-backed schemas. Files, documents, and resumable uploads also have source-backed schemas (#11866). Connected sources have a separate gated schema (#11868). |
| `/v1/connections*`, project Drive sources | Web connected-service handlers and gateway account integration | Account credential plus connected-service grant; Google availability gate | Eight account connection/Drive/source operations have qualified schemas (#11868). Google admission and each person’s grant remain required. Gmail, Calendar, and admin access-list schemas remain separate gaps. |
| `/v1/computers*` | Web computer lifecycle over Computers service; existing device/sync handlers share the family | App account; lifecycle owner and idempotency checks; device activity has separate rights | Lifecycle, capabilities, Boat command sessions, and bounded binary workspace files are in the app schema. Commands use authenticated byte-offset polling. The [Computers subset](openapi.computers.json) and [generated operation index](computers-operations.json) cover the contract; the ergonomic client remains #11805; existing device sync remains a separate contract. |
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
for project/thread/file/sign-in-key contracts, split by authority into
[#11864](https://github.com/OpenAgentsInc/openagents/issues/11864) (keys),
[#11865](https://github.com/OpenAgentsInc/openagents/issues/11865) (projects and membership),
[#11866](https://github.com/OpenAgentsInc/openagents/issues/11866) (project files and uploads),
[#11867](https://github.com/OpenAgentsInc/openagents/issues/11867) (threads), and
[#11868](https://github.com/OpenAgentsInc/openagents/issues/11868) (connected sources), and
[#11858](https://github.com/OpenAgentsInc/openagents/issues/11858) for the
public System One slice and discovery links, now implemented. Separately served voice,
payments, legacy services, and configured work surfaces retain the coverage gaps
listed above; they need their owners' contracts before client generation.

## Account key management

`GET /v1/keys`, `POST /v1/keys`, and `DELETE /v1/keys/{id}` use a live
account-service app session or sign-in key in `Authorization: Bearer …`.
Cookies alone and `oak_` API keys are refused. A sign-in key can create
another sign-in key and revoke itself. Each request resolves the account;
another account's key ID returns `404 unknown_key`.
The website's `/v1/keys` also works. The older `/api/v1/keys` prefix does
not mount these web handlers and continues to return `404`; use the API
origin in the published reference.

The default kind is `signin`; `sign-in` is an accepted input alias. Lists
contain metadata, never tokens. Sign-in dates are Unix seconds; nullable
expiry means until revoked. Creation takes `name` and optional
`expires_in_days` (1–365 or null), trims the name, and rejects an empty name,
control characters, or more than 80 characters. An account can have 50 working
sign-in keys. Name, expiry, and capacity refusals from this service become
`400 invalid_key` at the web front. Lists have no pagination.

`kind=api` lists or revokes keys in the first workspace where the account is
an owner (normally its personal workspace).
Creation uses `{"name":"example","kind":"api"}` and returns the token once.
These API keys have only `responses`, `models:read`, and `usage:read` scopes;
this route does not accept a workspace or scopes from the caller. API names are
trimmed and truncated to 80 characters. A non-null expiry returns
`400 invalid_expiry`; no owned workspace returns `409 no_workspace`.
API list dates and names can be null; dates are strings rather than the
sign-in list's Unix seconds.

All results use `Cache-Control: no-store, private`. Missing or invalid
credentials return `401 signed_out`; unavailable account storage returns
`503 unavailable`. Invalid JSON or unknown creation fields return
`400 invalid_request`. Key creation does not honor an idempotency header:
retrying an uncertain creation can make another key, and the first token
cannot be recovered from a list. Inspect the list before deciding what to
revoke or create. Revoking an already revoked sign-in key returns
`404 unknown_key`. These operations neither run a model nor make a payment.

The [key contract evidence](keys-evidence.json) records the implementation,
response-schema tests, deployed image, staging and production probes, and
cleanup. Recheck a deployment without credentials or key changes with
`python3 scripts/api/check_keys_public.py`; set both `--api` and `--website`
to `https://staging.openagents.com` for staging.

## Project and membership contracts

The reference describes 15 implemented project operations: lifecycle,
repositories, members, invitations, leaving, and repository access checks.
These web handlers accept live `sess_` sessions or `oa_sk_` sign-in keys,
not `oak_` API keys or cookies alone. A project ID or `name:NAME` selects
from current memberships; duplicate names return `409 ambiguous_name`.
Project roles are separate from workspace roles. Removed members lose access
on their next request; `X-Workspace-Id` cannot restore it.

Lists are oldest first, with a default limit of 100 clamped to 1–200.
`after` must still be present in the same account's current list; a missing
cursor returns `400 invalid_cursor`. Lists are not snapshots. Member lists
contain at most 100 people with `next: null`; only owners see open invitation
metadata. Invitation codes appear only on creation. Their default lifetime
is seven days; explicit nonzero values clamp to ten minutes through thirty
days. At most 50 open invitations are kept, replacing the oldest when needed.

Creation always makes a new project; names and repositories can repeat
across projects. No idempotency header deduplicates project or invitation
creation. A project can be created before protection setup fails, and
protection can change before a later edit fails. Deletion can commit before
file cleanup completes. Inspect current state before retrying an uncertain
mutation. Reads do not start a chat, execute a command, or make a payment.
Thread execution/sync (#11867) and connected sources (#11868) keep separate contracts.

The [project contract evidence](projects-evidence.json) records source,
schemas, isolated authorization and failure checks, deployed images, and
cleanup. `python3 scripts/api/check_projects_public.py` checks the deployed
reference and signed-out routing without creating a project; set both
origins to staging to check that environment.

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
python3 scripts/api/public_reference.py --schema docs/sdk/openapi.app.json --slice
python3 scripts/api/systemone_openapi.py --check
python3 scripts/api/public_reference.py --schema docs/sdk/openapi.systemone.json --slice
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

The System One artifact is generated with
`python3 scripts/api/systemone_openapi.py` (requires PyYAML). It follows only
references reachable from the owning `/v1/systemone` operation and prefixes
component names with `SystemOne`. The web front adds it separately from the
app fixture; the auth/history SDK fixture keeps its existing scope. Drift checks
compare operation bodies and component definitions, not only path names.

For a deployed System One check without account data or model work, run:

```sh
python3 scripts/api/check_systemone_public.py --malformed-call
```

For staging, set both `--api https://staging.openagents.com` and
`--website https://staging.openagents.com`. The check reads the reference,
cards, guide, and docs MCP. The optional POST body is deliberately invalid JSON;
it contains no credential, model, state, or questions. Use `--output FILE` to
save scrubbed results and `--browser-baseline FILE` to compare a previously
saved public `vault/release.json` file list.

The [System One qualification record](systemone-evidence.json) identifies its
source, immutable image, staging/production checks, legacy compatibility,
synthetic admission tests, and cleanup. No model work ran in the live probes.

## Project files and uploads

The reference includes 13 file, attachment, document, and resumable-upload
operations. These require a live account session or sign-in key and current
project rights. Attached files check those rights even without `?project=`;
shared storage reads still need that query to select the project's storage.
Vault attachments expose encrypted-object metadata only. Source creation
uses the separate connected-source routes.

Direct uploads accept original bytes, ordinarily up to 10 MiB, with a hard
16 MiB request limit. Resumable uploads use 8 MiB parts and configured file
and account quotas. Their expiry is a Unix timestamp. Repeat a start with
the same request ID and identical metadata to inspect progress; repeat an
identical part safely. Inspect progress after an uncertain completion before
retrying. Completion checks the complete file hash and publishes its project
attachment. Ordinary document creation has no such retry deduplication.
Removing a vault attachment leaves its encrypted object in the vault.

The [file contract evidence](files-evidence.json) records authorization tests,
the frozen 1.0.0 client's staging file workflow, cleanup, and deployments.
`python3 scripts/api/check_files_public.py` checks the reference and signed-out
routes without creating data; set both origins to staging to check it there.


## Connected project sources

The reference describes eight existing connection and Drive/source operations
([qualification evidence](connected-sources-evidence.json)). They accept current
`sess_` sessions or `oa_sk_` sign-in keys, with a separate `connectionBearer`
security scheme. Cookies and `oak_` API keys are refused. Connections must be
enabled for the account; other accounts receive `404 not_found`. Google must
also allow the person to connect while app verification is pending.

Source routes accept the account's own projects. A shared-project member,
including an editor, receives `403 project_role`; a nonmember receives `404`.
Project membership does not supply another person's Google credentials.
Listing stored sources does not check Drive access and has no pagination.
It can succeed after disconnect or grant revocation. Attaching checks the
caller's live Drive access, returns `201` for a new source, and returns `200`
with the original metadata for a duplicate. Limits are 20 sources per project
and 50 per account. A duplicate retry still checks Drive and can fail after
an earlier successful write. Disconnect keeps source metadata; detaching
removes that metadata without deleting the Drive file and needs no live grant.

Drive folder browsing passes Google's opaque `next` token back through `page`.
Keep the same folder between pages. Search ignores `page` and returns
`next: null`; an empty or over-512-byte page value is ignored. These lists
are not snapshots. Google revocation returns `409 google_reconnect`; absent
grants return `409 google_not_connected`, forbidden files return `403 drive_forbidden`, and downstream failures can return `502 google_unavailable`.
Tool failures use `422 tool_failed`, including missing scopes. Photo/video
reads can call a model and return usage and cost; the qualification uses only
synthetic text and Sheets data, with no inference. All eight operations return
JSON results; framework request refusals can use plain text. This surface offers
no event stream. Gmail, Calendar, browser OAuth,
and admin access-list operations retain their separate contracts.

Run `cargo test -p openagents-web connections::tests --lib`, regenerate the
app fixture with the `app_openapi` example, and run the public-reference checks
to verify these shapes without Google credentials or live account resources.
After deployment, `python3 scripts/api/check_connected_sources_public.py` checks
the public reference and signed-out routing. Pass `--api
https://staging.openagents.com` to check staging.
