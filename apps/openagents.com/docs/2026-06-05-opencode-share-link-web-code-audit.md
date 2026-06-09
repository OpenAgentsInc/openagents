# OpenCode Share Link Web Code Audit

Date: 2026-06-05

Reference checkout:
`/Users/christopherdavid/work/projects/repos/opencode`

OpenAgents product surface target:
`/Users/christopherdavid/work/openagents`

Prompt:
Audit the OpenCode web code that serves share links such as
`https://opncd.ai/share/OxNwAx6l`.

## Executive Summary

The OpenCode share-link system is split across two code planes:

- Public web host and share API: `packages/enterprise`
- Local OpenCode runtime, session HTTP API, TUI, CLI, GitHub Action paths:
  `packages/opencode`, plus a few generated SDK and docs surfaces

The public route for the user-provided example is:

```txt
GET https://opncd.ai/share/OxNwAx6l
```

The route owner in the local reference checkout is:

```txt
../projects/repos/opencode/packages/enterprise/src/routes/share/[shareID].tsx
```

The route is a SolidStart server-rendered page. It reads the share record and
all synced share data from the enterprise storage layer, reshapes the flat data
array into a structure compatible with the shared OpenCode UI components, and
renders a full-session read-only transcript and optional file-review/diff pane.

The write side is not owned by that page. The local OpenCode runtime uses
`ShareNext`:

```txt
../projects/repos/opencode/packages/opencode/src/share/share-next.ts
```

`ShareNext` creates the public share on the hosted API, stores the returned
share id, public URL, and secret in local SQLite, then listens to session,
message, part, and diff events. Those events are coalesced and flushed to the
hosted API. The hosted API persists snapshots in object storage.

The key security property is simple: share links are bearer URLs. Read access
does not require an account, password, or secret. Write/delete/sync access does
require the secret that was returned at creation time and stored locally. The
public share page marks itself `noindex, nofollow`, but that is search-engine
advice, not access control.

Important finding: the current active public route is `/share/:shareID`.
Several older docs and GitHub Action code paths still construct `/s/:shareID`.
A filtered live check on 2026-06-05 showed:

- `/share/OxNwAx6l` returns the share route and includes `getShareData`,
  `_shareID_`, `shareID`, `noindex`, and the current public share URL.
- `/s/OxNwAx6l` returns an HTML shell containing `Not Found`.

That mismatch matters for compatibility, imports, GitHub PR footer links, and
any OpenAgents product surface feature that tries to learn from or interoperate with OpenCode share
URLs.

OpenAgents product surface's proposed version should use the canonical human route `/share/{uuid}`.
It should support public, team-only, and named-user audiences, and the top of
the page should make that audience explicit with labels such as `Shared
publicly`, `Shared with members of OpenAgents Core Team`, or `Shared with you`.
Unlike OpenCode's split hosted share package, OpenAgents product surface does not need a separate
Worker for this feature. It should be part of the usual application and API
stack, implemented through Effect services, Effect Schema boundaries, normal D1
or R2 repositories, and the existing workroom/chat UI primitives.

## What I Read

OpenCode reference files:

- `packages/enterprise/src/routes/share/[shareID].tsx`
- `packages/enterprise/src/routes/api/[...path].ts`
- `packages/enterprise/src/core/share.ts`
- `packages/enterprise/src/core/storage.ts`
- `packages/enterprise/src/app.tsx`
- `packages/enterprise/src/routes/share.tsx`
- `packages/enterprise/src/routes/[...404].tsx`
- `packages/enterprise/vite.config.ts`
- `packages/enterprise/package.json`
- `packages/enterprise/test/core/share.test.ts`
- `packages/enterprise/test/core/storage.test.ts`
- `packages/opencode/src/share/share-next.ts`
- `packages/opencode/src/share/session.ts`
- `packages/opencode/src/server/routes/instance/httpapi/groups/session.ts`
- `packages/opencode/src/server/routes/instance/httpapi/handlers/session.ts`
- `packages/opencode/src/server/routes/instance/httpapi/server.ts`
- `packages/opencode/src/session/session.ts`
- `packages/core/src/session/sql.ts`
- `packages/core/src/share/sql.ts`
- `packages/core/src/session/projector.ts`
- `packages/core/src/v1/config/config.ts`
- `packages/opencode/src/config/config.ts`
- `packages/opencode/src/project/bootstrap.ts`
- `packages/opencode/src/cli/cmd/tui/routes/session/index.tsx`
- `packages/opencode/src/cli/cmd/tui/routes/session/sidebar.tsx`
- `packages/opencode/src/cli/cmd/run.ts`
- `packages/opencode/src/cli/cmd/import.ts`
- `packages/opencode/src/cli/cmd/pr.ts`
- `packages/opencode/src/cli/cmd/github.handler.ts`
- `packages/opencode/test/share/share-next.test.ts`
- `packages/opencode/test/cli/import.test.ts`
- `packages/opencode/test/server/httpapi-exercise/index.ts`
- `packages/opencode/test/server/httpapi-exercise/environment.ts`
- `github/index.ts`
- `github/action.yml`
- `infra/enterprise.ts`
- `infra/stage.ts`
- `sst.config.ts`
- `packages/function/src/api.ts`
- `packages/web/src/content/docs/share.mdx`
- `packages/web/src/content/docs/config.mdx`
- `packages/web/src/content/docs/cli.mdx`

Live checks:

- `curl -I https://opncd.ai/share/OxNwAx6l`
- `curl -I https://opncd.ai/s/OxNwAx6l`
- filtered body checks for `getShareData`, `shareID`, `_shareID_`,
  `noindex`, and `Not Found`
- filtered size checks only; the public transcript payload was not copied into
  this document

## Source Layout

The OpenCode repo is a monorepo. The share-link UI is not in the docs website
package. It is in the package named `@opencode-ai/enterprise`.

Relevant top-level organization:

```txt
packages/enterprise     Public share host and API for opncd.ai
packages/opencode       Local runtime, TUI, CLI, local HTTP API
packages/core           SQLite schemas and shared low-level core modules
packages/sdk            Generated API clients and OpenAPI output
packages/web            Public documentation website
github                  Composite GitHub Action entrypoint
infra                   SST/Cloudflare deployment definitions
```

The user-facing web route lives here:

```txt
packages/enterprise/src/routes/share/[shareID].tsx
```

That package has a tiny wrapper route:

```txt
packages/enterprise/src/routes/share.tsx
```

The wrapper only returns `props.children`. The real route behavior is in the
dynamic child `[shareID].tsx`.

The public API for creating, syncing, reading, and deleting shares lives here:

```txt
packages/enterprise/src/routes/api/[...path].ts
```

The in-process business logic lives here:

```txt
packages/enterprise/src/core/share.ts
packages/enterprise/src/core/storage.ts
```

The client-side publisher used by local OpenCode lives here:

```txt
packages/opencode/src/share/share-next.ts
packages/opencode/src/share/session.ts
```

## Public URL Model

Production short domain selection is in `infra/stage.ts`:

```txt
production -> opncd.ai
dev        -> dev.opncd.ai
other      -> <stage>.dev.opncd.ai
```

The public enterprise SolidStart app is deployed in `infra/enterprise.ts` with:

```txt
domain: shortDomain
path: packages/enterprise
buildCommand: bun run build:cloudflare
```

The current API create route returns URLs in this form:

```txt
https://<host>/share/<share-id>
```

That is assembled in `packages/enterprise/src/routes/api/[...path].ts` from
the request's forwarded protocol and host headers:

```txt
url: <protocol>://<host>/share/<share.id>
```

The local OpenCode client defaults to this hosted base URL when no active org
account is selected:

```txt
https://opncd.ai
```

That default is in `packages/opencode/src/share/share-next.ts`. The request
selector has two modes:

- Legacy/public mode: `baseUrl` is `config.enterprise.url` or
  `https://opncd.ai`, API resource name is `share`, no auth headers.
- Org/account mode: `baseUrl` is the active account URL, API resource name is
  `shares`, and headers include `authorization: Bearer <token>` plus
  `x-org-id`.

Tests in `packages/opencode/test/share/share-next.test.ts` assert these two
path families:

```txt
legacy:
POST   /api/share
POST   /api/share/:shareID/sync
GET    /api/share/:shareID/data
DELETE /api/share/:shareID

org/account:
POST   /api/shares
POST   /api/shares/:shareID/sync
GET    /api/shares/:shareID/data
DELETE /api/shares/:shareID
```

The public `packages/enterprise` app only implements the legacy family:

```txt
/api/share
/api/share/:shareID/sync
/api/share/:shareID/data
/api/share/:shareID
```

I did not find a local `packages/console` implementation for `/api/shares`.
The test suite treats it as a different authenticated account-control surface.

## URL Format Drift

There is live code and documentation drift around `/share/:id` versus `/s/:id`.

Current route implementation:

```txt
packages/enterprise/src/routes/share/[shareID].tsx
```

Current share create response:

```txt
packages/enterprise/src/routes/api/[...path].ts
url: <origin>/share/<id>
```

Current import parser:

```txt
packages/opencode/src/cli/cmd/import.ts
^https?://[^/]+/share/([a-zA-Z0-9_-]+)$
```

Current import tests explicitly reject `/s/:id` as legacy:

```txt
packages/opencode/test/cli/import.test.ts
parseShareUrl("https://opncd.ai/s/Jsj3hNIW") -> null
```

Stale or legacy references still construct `/s/:id`:

- `packages/web/src/content/docs/share.mdx`
- many localized copies of `share.mdx`
- `packages/web/src/content/docs/cli.mdx`
- many localized copies of `cli.mdx`
- `packages/web/src/content/docs/index.mdx`
- `github/index.ts`
- `packages/opencode/src/cli/cmd/github.handler.ts`
- `packages/opencode/src/cli/cmd/pr.ts`
- old Worker route code in `packages/function/src/api.ts`

The GitHub Action paths are especially important. They compute share IDs from
`session.id.slice(-8)` and build:

```txt
https://opencode.ai/s/<shareId>
```

The current public enterprise route expects:

```txt
https://opncd.ai/share/<shareId>
```

or whatever returned URL came from `POST /api/share`.

For OpenAgents product surface, the takeaway is not "copy OpenCode's URL scheme." The takeaway is
that a public-share system needs a single canonical URL constructor owned by
the share API response. Secondary paths should use the returned URL, not
rederive a route or slug.

## Public Share Page Route

The public page route imports these key pieces:

- `Share` from `~/core/share`
- `SessionTurn`, `SessionReview`, `MessageNav`, `FileSSR` from the shared UI
  package
- `DataProvider`, `FileComponentProvider`, `WorkerPoolProvider`
- `ProviderIcon`, `IconButton`, `Logo`, `Mark`
- `Binary.search`
- `DateTime` from `luxon`
- `Base64` from `js-base64`
- SolidStart route helpers: `query`, `createAsync`, `useParams`
- Solid primitives: `createMemo`, `createSignal`, `Show`, `For`, `Switch`,
  `Match`, `ErrorBoundary`

The server query is named:

```txt
getShareData
```

It is declared with SolidStart's `query(async (shareID) => { "use server" ... })`.

The route does this:

1. Reads the share metadata by ID via `Share.get(shareID)`.
2. Throws `SessionDataMissingError` if no share exists.
3. Reads flat share data via `Share.data(shareID)`.
4. Builds a normalized data object for `DataProvider`.
5. Initializes `session_status` as idle for the shared session.
6. Groups messages by `sessionID`.
7. Groups parts by `messageID`.
8. Stores file diffs under `session_diff[sessionID]`.
9. Stores model metadata under `model[sessionID]`.
10. Confirms the session list contains the canonical shared session using
    `Binary.search`.
11. Returns the normalized object.

The route sets HTTP caching for the rendered page:

```txt
Cache-Control: public, max-age=30, s-maxage=300, stale-while-revalidate=86400
```

The public JSON data route uses the same cache-control header.

The page adds:

```txt
<Meta name="robots" content="noindex, nofollow" />
```

That reduces indexing, but the read data remains public to anyone with the URL.

## SSR Payload Behavior

The route is server-rendered and serializes the query result into the HTML for
hydration. A live filtered check of `https://opncd.ai/share/OxNwAx6l` showed
the page HTML includes `getShareData`, `_shareID_`, `shareID`, `noindex`, and
the public share URL. The full HTML response was roughly 1.76 MB for that
session.

This means a public share is available in two practical read forms:

```txt
GET /share/:id
GET /api/share/:id/data
```

The first returns HTML with the serialized SolidStart data payload. The second
returns the flat `Share.Data[]` JSON directly.

For OpenAgents product surface, this is the most important implementation detail. If OpenAgents product surface provides
public workroom/session/share views, it should decide explicitly whether public
HTML may embed raw transcript/diff payloads, or whether the page should fetch
redacted/projection-scoped data through a narrower API.

## Public Share UI

The route renders a full-screen dark operational interface:

- Fixed header with OpenCode mark on the left.
- GitHub and Discord icon buttons on the right.
- Main body is read-only and selectable.
- Session title block shows version, provider icon, model display name, and
  created timestamp.
- On wide screens with no diffs, the session view gets the full width.
- On wide screens with diffs, the session view and file-review pane split the
  screen.
- On narrower screens with diffs, a tabs control switches between `Session`
  and `<N> Files Changed`.
- `MessageNav` appears when there is more than one user message.
- The active user message drives the provider/model metadata shown in the
  title area.
- `SessionTurn` renders conversation turns.
- `SessionReview` renders changed files and diffs.
- The file renderer is `FileSSR`, mounted through `FileComponentProvider`.
- A client-only worker pool is loaded from `@opencode-ai/ui/pierre/worker`.

The route does not present an input box. It is a display/projection surface,
not a replay or collaboration surface.

The active message state is local UI state:

```txt
{ messageId: string | undefined }
```

If no message is selected, the first user message becomes the active message.
The displayed provider is `activeMessage().model.providerID`, and the displayed
model ID is `activeMessage().model.modelID`.

The diffs are read from:

```txt
data().session_diff[data().sessionID] ?? []
```

The diff display mode is local state:

```txt
"unified" | "split"
```

The page uses shared OpenCode UI components heavily. A port into OpenAgents product surface should
not copy those components wholesale. Treat their data boundary as the reference
and build OpenAgents product surface-native rendering around OpenAgents product surface's own workroom, approval, receipt,
and artifact projection contracts.

## Metadata and Social Cards

When a session title exists, the route sets:

```txt
<Title>{title} | OpenCode</Title>
<Meta name="description" content="opencode - The AI coding agent built for the terminal." />
<Meta property="og:image" content={ogImage()} />
<Meta name="twitter:image" content={ogImage()} />
```

The `ogImage` URL is built from:

- base64-encoded and URL-encoded title substring, capped at 700 characters
- assistant model IDs observed in messages
- session version
- share ID

Target service:

```txt
https://social-cards.sst.dev/opencode-share/<encodedTitle>.png
```

Query parameters:

```txt
model=<modelParam>
version=v<session.version>
id=<shareID>
```

Model display rules:

- one assistant model: exact model ID
- two assistant models: `<first> & <second>`
- more than two: `<first> & <N> others`
- none: `unknown`

This social-card path repeats in stale GitHub Action footer code, but with the
old `/s/:id` link target.

For OpenAgents product surface, the relevant idea is not the social card service itself. The useful
pattern is that share metadata is generated from the public projection state,
not from a separate marketing CMS.

## Error Handling and Not Found

The share route defines a custom `SessionDataMissingError`.

Missing share or missing canonical session data becomes:

```txt
<NotFound />
```

Other render failures are logged to the browser console and displayed with a
stack/message in a dark error screen:

```txt
Unable to render this share.
Check the console for more details.
<pre>{details}</pre>
```

The NotFound component is the default Solid template-like page and still links
to SolidJS and `/about`. That is not polished for a production public share
surface.

The live `/s/OxNwAx6l` check returned a short HTML response containing
`Not Found`, not a share route payload.

OpenAgents product surface should not reuse this fallback behavior. A public artifact/share route
should have a branded and product-specific not-found state, and should avoid
printing stack traces to public users.

## Public API Routes

The public enterprise API is implemented as a single catch-all SolidStart
server route wrapping a Hono app:

```txt
packages/enterprise/src/routes/api/[...path].ts
```

The Hono app:

- sets base path `/api`
- enables global CORS with `cors()`
- exposes an OpenAPI document at `/api/doc`
- implements create, sync, data, and delete endpoints

Routes:

```txt
POST   /api/share
POST   /api/share/:shareID/sync
GET    /api/share/:shareID/data
DELETE /api/share/:shareID
```

`POST /api/share`

Input:

```json
{ "sessionID": "..." }
```

Output:

```json
{ "id": "...", "secret": "...", "url": "https://host/share/id" }
```

The `secret` is only returned at creation time. It is required for future
sync/delete operations.

`POST /api/share/:shareID/sync`

Input:

```json
{ "secret": "...", "data": [Share.Data] }
```

Effect:

- validates the route param
- validates the JSON payload
- calls `Share.sync({ share: { id, secret }, data })`
- returns `{}`

`GET /api/share/:shareID/data`

Effect:

- validates the route param
- sets the same public cache-control policy as the page
- returns `Share.data(shareID)`

No secret is required.

`DELETE /api/share/:shareID`

Input:

```json
{ "secret": "..." }
```

Effect:

- validates the route param
- validates the JSON body
- calls `Share.remove({ id, secret })`
- returns `{}`

The API route exports `GET`, `POST`, `PUT`, and `DELETE` handlers. `PUT` is
exported even though the Hono route tree shown here does not define a PUT
share endpoint.

## Share Data Schema

The public hosted data model is a Zod discriminated union on `type`:

```txt
session
message
part
session_diff
model
```

Types:

```txt
{ type: "session",      data: SDK.Session }
{ type: "message",      data: SDK.Message }
{ type: "part",         data: SDK.Part }
{ type: "session_diff", data: SnapshotFileDiff[] }
{ type: "model",        data: SDK.Model[] }
```

The schema imports SDK v2 data types from:

```txt
@opencode-ai/sdk/v2
```

But note the use of `z.custom<T>()`. That means the hosted API validates the
outer discriminated shape, but it does not deeply validate SDK session,
message, part, diff, or model payloads with a structural schema in this file.

For OpenAgents product surface, that is a serious boundary difference. A public projection endpoint
should prefer Effect Schema or another runtime schema with deep validation at
every external payload boundary.

## Hosted Share Storage

Storage is abstracted by:

```txt
packages/enterprise/src/core/storage.ts
```

The adapter interface is:

```txt
read(path): Promise<string | undefined>
write(path, value): Promise<void>
remove(path): Promise<void>
list(options): Promise<string[]>
```

Two object-store adapters exist:

- S3
- Cloudflare R2

Adapter selection comes from:

```txt
OPENCODE_STORAGE_ADAPTER
```

S3 environment:

```txt
OPENCODE_STORAGE_BUCKET
OPENCODE_STORAGE_REGION
OPENCODE_STORAGE_ACCESS_KEY_ID
OPENCODE_STORAGE_SECRET_ACCESS_KEY
```

R2 environment:

```txt
OPENCODE_STORAGE_ACCOUNT_ID
OPENCODE_STORAGE_BUCKET
OPENCODE_STORAGE_ACCESS_KEY_ID
OPENCODE_STORAGE_SECRET_ACCESS_KEY
```

The deployed Cloudflare/SST app sets:

```txt
OPENCODE_STORAGE_ADAPTER=r2
OPENCODE_STORAGE_ACCOUNT_ID=<Cloudflare account id>
OPENCODE_STORAGE_ACCESS_KEY_ID=<secret>
OPENCODE_STORAGE_SECRET_ACCESS_KEY=<secret>
OPENCODE_STORAGE_BUCKET=<created bucket>
```

Storage keys are resolved by joining path segments and appending `.json`:

```txt
["share", id] -> share/<id>.json
```

Important key families:

```txt
share/<id>.json
share_snapshot/<id>.json
share_compaction/<id>.json
share_event/<id>/<event>.json
share_data/<id>/...
```

Current sync writes to `share_snapshot/<id>.json`. Legacy migration paths can
read and compact `share_event` and `share_compaction`. There is also `syncOld`
that writes `share_data`, but current `Share.sync` writes snapshots.

The storage `list` implementation parses object-store XML with a regex:

```txt
/<Key>([^<]+)<\/Key>/g
```

This is acceptable for the reference, but OpenAgents product surface should avoid ad hoc XML parsing
if it has a typed API available.

## Share Creation

Hosted share creation is in `Share.create`.

Input:

```txt
{ sessionID: string }
```

ID rule:

```txt
id = sessionID.slice(-8)
```

Test/session prefix rule:

```txt
if NODE_ENV === "test" or sessionID starts with "test_":
  id = "test_" + sessionID.slice(-8)
```

Secret rule:

```txt
secret = crypto.randomUUID()
```

The create method checks `Share.get(info.id)` first. If a share already exists,
it throws `AlreadyExists`. Otherwise it writes:

```txt
share/<id>.json
share_snapshot/<id>.json with []
```

Implications:

- Share IDs are deterministic from the last 8 characters of the session ID.
- If the session ID is public or leaked, the share ID is derivable.
- The session ID itself appears to have a random suffix, so this is not
  automatically guessable, but it is not independent entropy.
- A suffix collision becomes `AlreadyExists`, not a retry with fresh entropy.
- The public URL is stable for a given session suffix.

For OpenAgents product surface, prefer an independent public projection ID with enough entropy and
a durable mapping back to the private workroom/session. Do not derive public
share IDs from internal authority IDs unless there is a clear reason and a
collision/revocation model.

## Sync Merge Semantics

Both hosted `Share` and local `ShareNext` define a `key(item)` function with
the same semantics:

```txt
session      -> "session"
message      -> "message/<message.id>"
part         -> "part/<messageID>/<part.id>"
session_diff -> "session_diff"
model        -> "model"
```

The hosted `merge(...items)` function uses a `Map<string, Data>`.

Behavior:

- Later data overwrites earlier data with the same key.
- New message and part records accumulate.
- The latest `session` item replaces the prior session item.
- The latest `session_diff` replaces the prior diff set.
- The latest `model` item replaces the prior model list.
- The result is sorted lexicographically by merge key before returning values.

That sort creates stable output, but it is not conversation chronological
ordering by itself. The UI route later sorts user messages by
`time.created`.

The tests cover:

- creating a share
- syncing one batch
- syncing multiple batches
- retrieving synced data
- duplicate part updates returning latest data
- empty shares returning `[]`
- migrating legacy event data into snapshots
- invalid secret failure
- missing share failure
- multiple data types in one share

## Legacy Hosted Paths

`Share.legacy(shareID)` reads older storage shapes:

```txt
share_compaction/<shareID>.json
share_event/<shareID>/...
```

It lists events before the compaction cursor, reverses them, merges them with
compaction data, writes a new compaction record, writes a new snapshot, and
returns the merged data.

`Share.syncOld` writes to:

```txt
share_data/<id>/session
share_data/<id>/message/<messageID>
share_data/<id>/part/<messageID>/<partID>
share_data/<id>/session_diff
share_data/<id>/model
```

Current `Share.data` prefers `share_snapshot` and falls back to legacy
migration only when no snapshot exists.

For OpenAgents product surface, the lesson is to model legacy snapshot migration explicitly if the
public projection format changes. Do not let the public page quietly stitch
together multiple obsolete formats unless the migration logic has tests and
bounded retention semantics.

## Local OpenCode ShareNext

The local publishing service is:

```txt
packages/opencode/src/share/share-next.ts
```

It is an Effect service:

```txt
@opencode/ShareNext
```

Interface:

```txt
init(): Effect<void>
url(): Effect<string>
request(): Effect<Req>
create(sessionID): Effect<Share>
remove(sessionID): Effect<void>
```

`Req` contains:

```txt
headers: Record<string, string>
api: {
  create: string
  sync(id): string
  remove(id): string
  data(id): string
}
baseUrl: string
```

The service-level disable flag is:

```txt
OPENCODE_DISABLE_SHARE=true
OPENCODE_DISABLE_SHARE=1
```

If disabled:

- `init` returns without subscribing.
- `sync` returns.
- `create` returns empty strings.
- `remove` returns.

The runtime also has config-level sharing controls, covered below. The
environment disable flag is lower-level and used by tests such as the HTTP API
exercise suite.

## Local Request Selection

`ShareNext.request()` decides where to send share API calls.

If there is no active account or no active org:

```txt
baseUrl = config.enterprise.url ?? "https://opncd.ai"
api = legacy /api/share family
headers = {}
```

If there is an active account with an active org:

```txt
baseUrl = activeAccount.url
api = /api/shares family
headers.authorization = Bearer <account token>
headers.x-org-id = <active org id>
```

If the active account has an org but no token, sharing fails with:

```txt
No active account token available for sharing
```

Tests assert these behaviors, including the default `https://opncd.ai`
fallback.

For OpenAgents product surface, this is a useful split: public unauthenticated share service versus
authenticated organization-owned share service. But OpenAgents product surface should avoid making a
single config key called `enterprise.url` decide public projection authority
without an explicit Source Authority or operator-controlled routing contract.

## Local Durable Share Record

The local SQLite table is:

```txt
packages/core/src/share/sql.ts
```

Table:

```txt
session_share
```

Columns:

```txt
session_id primary key references session(id) on delete cascade
id         text not null
secret     text not null
url        text not null
timestamps
```

The public URL is also copied into the `session` table as:

```txt
share_url
```

`Session.fromRow` converts `share_url` to:

```txt
share: { url }
```

`Session.toRow` writes:

```txt
share_url: info.share?.url
```

The session projector also writes `share_url` when projecting session events.

This is an important separation:

- `session.share.url` is user-facing and safe enough for UI display.
- `session_share.secret` is local write authority and should not be projected
  into public UI or exported casually.

OpenAgents product surface should preserve that separation. Public projection URL, internal
projection ID, and mutation secret/capability should be different fields with
different storage and logging rules.

## Local Share Creation Flow

`ShareNext.create(sessionID)`:

1. Returns empty share values if `OPENCODE_DISABLE_SHARE` is set.
2. Calls `request()` to pick hosted base URL, API paths, and headers.
3. `POST`s `{ sessionID }` to `<baseUrl><api.create>`.
4. Decodes the JSON response with `ShareSchema`:
   `{ id: string, url: string, secret: string }`.
5. Upserts into local `session_share` keyed by `session_id`.
6. Updates in-memory cache for the session.
7. Forks a full sync of the current session.
8. Returns the share response.

`SessionShare.share(sessionID)` wraps this:

1. Reads config.
2. Throws if `config.share === "disabled"`.
3. Calls `shareNext.create(sessionID)`.
4. Calls `session.setShare({ sessionID, share: { url: result.url } })`.
5. Returns the result.

The local HTTP API handler for `POST /session/:sessionID/share`:

1. Requires the session exists.
2. Calls `shareSvc.share(sessionID)`.
3. Maps share/unshare errors to typed internal server errors, not blanket
   bad-request errors.
4. Returns the updated session.

## Full Sync and Incremental Sync

Full sync is local and runs after share creation.

`ShareNext.full(sessionID)` gathers:

- session info
- session diffs
- messages and their parts
- provider model records for unique user-message models

It then calls `sync(sessionID, data)` with:

```txt
session
message[]
part[]
session_diff
model
```

Incremental sync is event-driven.

`ShareNext.init()` materializes per-instance state. During initialization, it
subscribes to:

```txt
Session.Event.Updated
MessageV2.Event.Updated
MessageV2.Event.PartUpdated
Session.Event.Diff
Session.Event.Deleted
```

Each subscriber filters by event location:

```txt
event.location?.directory === instanceContext.directory
```

Updates:

- Session updated -> sync latest session.
- Message updated -> sync latest message.
- User message updated -> look up provider model and sync model list.
- Part updated -> sync latest part.
- Diff updated -> sync latest diff.
- Session deleted -> remove public share.

`sync(sessionID, data)`:

1. Returns if disabled.
2. Looks up cached local share record.
3. Returns if the session is not shared.
4. If a queue already exists for the session, merges new items into that queue
   by the same item key rules.
5. If no queue exists, creates one and forks a delayed `flush(sessionID)`.

Delay:

```txt
1000 ms
```

`flush(sessionID)`:

1. Removes the queued map from state.
2. Re-checks the cached share record.
3. Calls `request()`.
4. `POST`s to `<baseUrl><api.sync(share.id)>`.
5. Sends `{ secret: share.secret, data: Array.from(queued.values()) }`.
6. Logs a warning for HTTP status >= 400.

Tests verify rapid diff events coalesce into a single delayed sync with the
latest diff payload.

For OpenAgents product surface, this is a strong design idea: publish deltas from typed events,
coalesce by semantic identity, and do a full sync at share creation. But the
hosted write boundary should deeply validate input and record projection
authority explicitly.

## Session Creation and Auto Share

`SessionShare.create(input)` wraps `session.create(input)`.

It skips auto-share for child sessions:

```txt
if (result.parentID) return result
```

It then reads config and runtime flags:

```txt
flags.autoShare || config.share === "auto"
```

If true, it forks `share(result.id)` in the current scope and returns the
session immediately.

This means auto-share is asynchronous and best-effort. Session creation does
not wait for hosted share creation to succeed.

Manual share uses `SessionShare.share` and waits for the share operation.

## Config Controls

The config schema accepts:

```txt
share: "manual" | "auto" | "disabled"
autoshare?: boolean
enterprise?: { url?: string }
username?: string
```

`autoshare` is deprecated. `packages/opencode/src/config/config.ts` migrates:

```txt
if autoshare === true and !share:
  share = "auto"
```

Docs say:

- `manual`: allow manual sharing via commands
- `auto`: automatically share new conversations
- `disabled`: disable sharing entirely

Managed settings can override user and project sharing settings. Tests verify
managed `share: "disabled"` overrides user `share: "auto"`.

The v2 config spec keeps `share`, removes `autoshare`, keeps
`enterprise.url`, and keeps `username`.

For OpenAgents product surface, config should distinguish:

- whether public sharing is allowed
- whether auto-publication is allowed
- which public projection authority receives data
- whether an org/account context is required
- retention and deletion policy
- redaction policy

OpenCode mostly models the first three, with account mode only inside
`ShareNext.request()`.

## TUI and Web-Control Actions

The TUI command list in:

```txt
packages/opencode/src/cli/cmd/tui/routes/session/index.tsx
```

adds:

```txt
session.share
session.unshare
```

Manual share behavior:

- Title is `Copy share link` when the session already has a share URL.
- Title is `Share session` when not yet shared.
- Command is disabled when `sync.data.config.share === "disabled"`.
- Slash command name is `/share`.
- If a URL already exists, the command copies it to the clipboard.
- Before first share, it asks a consent dialog:
  `Share Session` / `Are you sure you want to share it?`
- Consent is persisted in key-value state as `share_consent`.
- It calls `sdk.client.session.share({ sessionID })`.
- On success it copies `res.data.share.url`.
- It displays success or error toasts.

Unshare behavior:

- Command appears as `session.unshare`.
- Slash command name is `/unshare`.
- Enabled only when `session().share?.url` exists.
- It calls `sdk.client.session.unshare({ sessionID })`.
- It shows success/error toasts.

The TUI sidebar receives and renders `share_url`:

```txt
session()!.share?.url
```

Plugin slots also receive `share_url`.

The local HTTP TUI route maps legacy command aliases:

```txt
session_share -> session.share
```

That mapping is in `packages/opencode/src/server/routes/instance/httpapi/handlers/tui.ts`.

## CLI Run Sharing

`packages/opencode/src/cli/cmd/run.ts` has a helper named `share`.

It reads config from the local SDK:

```txt
const cfg = await sdk.config.get()
```

It returns unless one of these is true:

```txt
cfg.data.share === "auto"
flags.autoShare
args.share
```

Then it calls:

```txt
sdk.session.share({ sessionID })
```

If the error message includes `disabled`, it prints that error. On success it
prints the public share URL.

This is a separate trigger path from automatic share-on-session-create inside
`SessionShare.create`.

## Import Command

The import command can import local JSON or a share URL.

URL parser:

```txt
^https?://[^/]+/share/([a-zA-Z0-9_-]+)$
```

It rejects:

```txt
https://opncd.ai/s/<id>
```

When importing a URL:

1. It extracts slug from `/share/<slug>`.
2. It computes `baseUrl` from the URL origin.
3. It gets the current `ShareNext.request()` to know current API paths and
   headers.
4. It attaches share auth headers only if the share URL origin equals the
   account base URL origin.
5. It tries `<baseUrl><req.api.data(slug)>`.
6. If that fails and the path was not legacy `/api/share/:id/data`, it falls
   back to `<baseUrl>/api/share/:id/data`.
7. It expects a flat `Share.Data[]`.
8. It transforms flat data back into:
   `{ info: session, messages: [{ info: message, parts: [...] }] }`.
9. It decodes session/message/part data with local schemas before inserting.

The parser tests show the canonical URL has moved to `/share`.

This is valuable for OpenAgents product surface: import/replay should not read the public HTML page.
It should use a versioned JSON projection endpoint with scoped schema decoding.

## GitHub Action and PR Workflow Drift

There are two GitHub-related code areas:

- `github/index.ts`: composite action package entrypoint
- `packages/opencode/src/cli/cmd/github.handler.ts`: newer local CLI handler

Both still construct stale `/s/:id` links.

`github/index.ts`:

- reads `SHARE` input
- skips sharing when `SHARE=false`
- skips sharing by default for private repos
- calls `client.session.share`
- returns `session.id.slice(-8)` as share ID
- logs `Share link: <shareUrl>/s/<shareId>`
- adds PR/issue footer links to `<shareUrl>/s/<shareId>`
- adds social-card image links to `<shareUrl>/s/<shareId>`

`github/action.yml` says share defaults to true for public repos.

`packages/opencode/src/cli/cmd/github.handler.ts`:

- sets `shareBaseUrl` to `https://opencode.ai` or `https://dev.opencode.ai`
- shares unless disabled or private repo without explicit share
- calls `sessionShare.share(session.id)`
- computes `shareId = session.id.slice(-8)`
- later checks existing comments for `<shareBaseUrl>/s/<shareId>`
- writes footer links to `<shareBaseUrl>/s/<shareId>`

`packages/opencode/src/cli/cmd/pr.ts` also looks for:

```txt
https://opncd.ai/s/<id>
```

and then calls `opencode import <sessionUrl>`. The current import parser rejects
`/s`, so that path appears broken unless another redirect/rewrite occurs before
the parser sees the URL. A live filtered check indicates `/s/:id` returns a
NotFound page, not a share payload.

For OpenAgents product surface, do not compute share links from internal session IDs in peripheral
automation. Always persist and reuse the canonical URL returned by the share
creation endpoint.

## Old Worker Share API

`packages/function/src/api.ts` contains an older Hono/Durable Object share API:

```txt
POST /share_create
POST /share_delete
POST /share_delete_admin
POST /share_sync
GET  /share_poll
GET  /share_data
```

That older API:

- uses a `SyncServer.shortName(sessionID)` equal to the last 8 characters
- returns `https://<WEB_DOMAIN>/s/<short>`
- syncs keyed content through a Durable Object
- supports WebSocket polling

This looks like the source of the older `/s/:id` convention. The current
SolidStart enterprise package uses `/share/:id`, object-store snapshots, and
direct JSON data routes instead.

For OpenAgents product surface, the caution is that URL and data-format migrations need active
compatibility policy. OpenCode has both current and legacy code in the repo,
and some call sites still follow the legacy route.

## Deployment

The public share host is deployed by SST in `infra/enterprise.ts`:

```txt
new sst.cloudflare.Bucket("EnterpriseStorage")
new sst.cloudflare.x.SolidStart("Teams", {
  domain: shortDomain,
  path: "packages/enterprise",
  buildCommand: "bun run build:cloudflare",
  environment: { ...R2 storage config... },
})
```

`packages/enterprise/vite.config.ts` switches Nitro target when:

```txt
OPENCODE_DEPLOYMENT_TARGET=cloudflare
```

Cloudflare preset:

```txt
preset: "cloudflare-module"
compatibilityDate: "2024-09-19"
nodeCompat: true
```

Package script:

```txt
bun run build:cloudflare
```

The enterprise package imports Tailwind styles through:

```txt
@import "@opencode-ai/ui/styles/tailwind";
```

For OpenAgents product surface, the relevant deployment pattern is:

- isolate public projection host from local runtime
- back it with object storage
- keep mutation credentials server-side
- make the share URL API return the canonical public URL

But the current OpenCode host also exposes public CORS data endpoints and SSR
hydration payloads; OpenAgents product surface should explicitly decide whether that is acceptable.

## Tests and Coverage

Hosted enterprise tests:

```txt
packages/enterprise/test/core/share.test.ts
packages/enterprise/test/core/storage.test.ts
```

They cover:

- create
- sync
- multi-batch sync
- retrieval
- duplicate item replacement
- empty shares
- legacy event migration into snapshots
- invalid secrets
- non-existent shares
- mixed data types
- object-store list range behavior

Local ShareNext tests:

```txt
packages/opencode/test/share/share-next.test.ts
```

They cover:

- legacy request path selection
- default public URL selection
- org account request path selection and headers
- create persistence and response decoding
- remove persistence and delete endpoint call
- create failure not persisting local share
- delayed coalesced diff sync

Import tests:

```txt
packages/opencode/test/cli/import.test.ts
```

They cover:

- parsing `/share/:id`
- rejecting `/s/:id`
- attaching auth headers only for same-origin account URLs
- transforming flat share data back into local storage shape

HTTP API exercise tests include `POST /session/:sessionID/share` and
`DELETE /session/:sessionID/share`, but the exercise environment sets
`OPENCODE_DISABLE_SHARE=true`, so it validates route shape/status more than
real hosted publication.

Coverage gaps I saw:

- No route-level test for the public SolidStart `/share/:id` page.
- No test that `/s/:id` redirects to `/share/:id` or intentionally returns
  branded NotFound.
- No end-to-end test from `/share` command through hosted data route and page
  render.
- No test that GitHub Action footer URLs use the canonical share URL returned
  by the API.
- No deep validation tests for malformed SDK payloads in hosted `Share.Data`.
- No explicit test for share ID collision behavior.
- No public redaction test for sensitive tool outputs, file paths, diffs, or
  metadata.
- No retention TTL test. Shared conversations persist until explicit unshare.

## Privacy and Security Properties

Public read:

- Anyone with `/share/:id` can read.
- Anyone with `/api/share/:id/data` can fetch the flat data array.
- Global CORS is enabled on the public API route.
- The page sets `noindex, nofollow`, but does not require auth.

Private write/delete:

- Sync and delete require the secret.
- Secret is generated with `crypto.randomUUID()`.
- Secret is stored locally in `session_share`.
- Secret is not copied into `session.share.url`.

Data exposed by the public read surface:

- Session ID
- Session slug
- Project ID
- Directory path
- Relative path
- Title
- Agent
- Model IDs and provider IDs
- Version
- Cost/tokens summary where present
- Created/updated timestamps
- Public share URL
- Messages
- Parts
- Diffs and patches
- Model metadata

The live user-provided example includes a large serialized transcript/diff
payload in the page HTML and direct JSON data at `/api/share/:id/data`.

For OpenAgents product surface, this should be treated as a public projection contract, not a
convenience UI. Public projection needs policy:

- What fields may leave the private workroom?
- Are local filesystem paths public?
- Are patches/diffs public?
- Are model IDs and token/cost details public?
- Is project ID public?
- Are tool outputs public?
- Are all message parts public, or only user-visible text?
- Does approval/receipt metadata get exposed?
- Who can revoke?
- What are retention guarantees?

## Operational Risks

1. URL drift between `/share/:id` and `/s/:id`

Current route and import paths use `/share`. Docs, GitHub Action code, and old
Worker code still use `/s`. This is likely to create broken public links and
failed imports.

2. Deterministic share ID from session ID suffix

The share ID is the last 8 characters of the session ID. That is convenient,
but it means the public ID is not independent from the private ID.

3. Hosted create does not retry on ID collision

If two session IDs collide on the final 8 characters, hosted create throws
`AlreadyExists`.

4. Deep payload validation is weak at the hosted boundary

`Share.Data` uses `z.custom<T>()` for SDK internals. The discriminant is
validated; nested payload correctness is mostly trusted.

5. Public page can embed very large payloads

The example page was roughly 1.76 MB. Larger sessions could create slow public
loads, expensive edge rendering, or CDN/cache stress.

6. Public error screen can expose stack details

Non-not-found render failures display details in a `<pre>` block. That may be
useful during development but is not ideal for a public hosted page.

7. Global CORS applies to the API

The share data endpoint is intended to be public, but global CORS makes
cross-origin scraping easier. That may be acceptable, but it should be an
explicit choice.

8. Public data includes filesystem/workspace context

The share session object can include local directory and project identifiers.
OpenCode docs warn users not to share sensitive conversations, but the code
does not enforce redaction.

9. Stale old Worker share API remains in the repo

`packages/function/src/api.ts` still carries `/share_create`, `/share_sync`,
and `/s` URL logic. Future readers can easily copy the wrong implementation.

10. Tests do not cover the public route end to end

The storage and client sync logic is tested. The actual route page, metadata,
not-found behavior, canonical URL behavior, and SSR payload boundary are not
covered locally in the files I inspected.

## OpenAgents product surface Relevance

OpenCode's implementation is useful as a reference for:

- public session/workroom projection
- write-secret separation from public URL
- full sync followed by event-driven incremental sync
- coalescing updates by semantic data key
- serving public shares from a dedicated host backed by object storage
- using a direct JSON endpoint for import/replay
- using shared UI components to render sessions and diffs consistently

It is not directly compatible with OpenAgents product surface's higher-integrity goals:

- It exposes broad raw session data publicly.
- It lacks deep schema validation at the hosted payload boundary.
- It does not model approval, receipt, Source Authority, or public/private
  projection boundaries.
- It derives share ID from session ID suffix.
- It allows stale URL constructors to remain in docs/action code.
- It uses prose warnings for privacy rather than a typed redaction policy.

OpenAgents product surface should treat this as a pattern library, not as an authority model.

## Proposed OpenAgents product surface Version

The OpenAgents product surface version should keep the one thing OpenCode gets right, which is a
stable, copyable share URL, but it should change the authority model. The
canonical page route should be:

```txt
GET /share/{uuid}
```

Use a real independent UUID for `{uuid}`. Do not derive it from an agent run ID,
team thread ID, local session ID, or any other private object ID. The page URL
is the stable product URL for humans. Any JSON/API URLs are implementation
surfaces behind that page.

The share feature should not require a separate Worker. It should live in the
usual OpenAgents product surface application stack:

- route parsing in the existing web app route model
- API handlers in the existing API Worker route modules
- D1/R2 access behind normal repository services
- all orchestration and validation through Effect services and Effect Schema
- normal sync scopes and receipts where the projection needs live updates

OpenCode split the hosted share service into an enterprise package because its
local runtime is a separate product. OpenAgents product surface already has one application that owns
authenticated workrooms, team rooms, run records, public projections, sync, and
receipts. Keeping `/share/{uuid}` inside that application avoids a second
deployment surface, a second origin policy, and a second source of canonical URL
truth.

### Audience Model

OpenAgents product surface should treat a share as a projection with an explicit audience, not as a
single public-link flag. A share can be:

```txt
public
team
users
```

`public` means anyone with the URL can view the redacted projection. It does not
mean the raw workroom or run is public. Public shares can still be `noindex`,
but `noindex` is not the control boundary.

`team` means only active members of a specific team can view the projection.
The share record should store the team ID and display team name at projection
time, then the request path should require an authenticated browser session and
an active membership check before returning the private projection payload.

`users` means only named recipient users can view the projection. The request
path should require authentication, then match the viewer's user ID or canonical
email against the recorded recipient list. The owner can be included as an
explicit recipient or allowed through a separate owner rule, but the behavior
should be represented in the access service rather than implied by UI.

This suggests a discriminated audience shape:

```txt
ShareAudience =
  | { _tag: "Public" }
  | { _tag: "TeamMembers", teamId, teamName }
  | { _tag: "Users", recipients: [{ userId, email, displayName }] }
```

The share record may also need:

```txt
shareId
canonicalUrl
sourceKind
sourceId
ownerUserId
teamId
projectId
audience
projectionVersion
redactionPolicyId
status
createdAt
updatedAt
revokedAt
expiresAt
receiptRefs
```

### Top-of-Page Share Label

The first visible line on the share page should make the audience legible before
the transcript begins. Example copy:

```txt
Shared publicly
Shared with members of OpenAgents Core Team
Shared with you
Shared with Chris David
Shared with 3 people
```

The label should be derived from the access decision plus the share projection,
not from the viewer guessing based on the URL. Suggested rules:

- public audience: `Shared publicly`
- team audience with a known team display name:
  `Shared with members of {teamName}`
- team audience without a usable display name: `Shared with this team`
- users audience where the authenticated viewer is a recipient:
  `Shared with you`
- users audience with exactly one named recipient and the owner/admin is
  viewing: `Shared with {displayName}`
- users audience with several recipients and the owner/admin is viewing:
  `Shared with {n} people`

The share page should also show a small secondary context row, such as:

```txt
Autopilot run · Pylon release plan · Completed Jun 5, 2026
```

That row can contain source title, source kind, status, and timestamp. It should
not expose private repository paths, internal runner IDs, bearer capabilities,
provider account refs, or callback URLs.

### Access Behavior

The `/share/{uuid}` route needs three clean states:

1. View allowed.
2. Authentication required.
3. Not found or not authorized.

For public shares, the server can render immediately with the public projection.
For team and user-targeted shares, the server should not embed the restricted
projection in HTML until the viewer is authenticated and authorized. If the
viewer is signed out, show a sign-in prompt tied to the share URL and return to
the same URL after login. If the viewer is signed in but not allowed, use a
branded not-authorized page that does not reveal the full underlying source.

Cache policy should follow audience:

- public share HTML/data: short public cache is acceptable if revocation uses
  explicit invalidation or a small TTL
- team/user share HTML/data: no-store or authenticated session-bound caching
- revoked/expired share: no-store

### Data Model

Do not make share pages read raw workroom, team chat, or agent run tables
directly. Store or derive a public projection with a versioned schema. A D1
table plus optional R2 object payload is enough:

```txt
share_projections
  id TEXT PRIMARY KEY
  canonical_url TEXT NOT NULL
  source_kind TEXT NOT NULL
  source_id TEXT NOT NULL
  owner_user_id TEXT NOT NULL
  team_id TEXT
  project_id TEXT
  audience_json TEXT NOT NULL
  title TEXT NOT NULL
  summary TEXT
  status TEXT NOT NULL
  projection_version INTEGER NOT NULL
  projection_json TEXT
  projection_object_key TEXT
  redaction_policy_id TEXT NOT NULL
  created_at TEXT NOT NULL
  updated_at TEXT NOT NULL
  revoked_at TEXT
  expires_at TEXT
```

For named users, either encode recipients in `audience_json` for the first pass
or add a separate table when invitation and audit flows need row-level querying:

```txt
share_projection_recipients
  share_id TEXT NOT NULL
  subject_kind TEXT NOT NULL -- user | email | team
  subject_id TEXT NOT NULL
  display_name TEXT
  created_at TEXT NOT NULL
```

The first implementation should bias toward a simple D1-backed projection row
unless the payload size forces R2. OpenCode's example HTML was large because it
embedded the full route payload. OpenAgents product surface should keep the schema version explicit
and choose whether the HTML embeds a small initial projection or fetches the
timeline from `GET /api/share/{uuid}/v1/data`.

### API Boundary

The API should be part of the existing API Worker, not a new Worker:

```txt
POST   /api/share
GET    /api/share/{uuid}/v1/data
PATCH  /api/share/{uuid}
DELETE /api/share/{uuid}
```

`POST /api/share` creates a share projection. It accepts browser-session auth,
programmatic agent bearer auth, and the operator admin API bearer token. Browser
and agent callers create shares as their own user. Admin-token callers must
include an explicit target selector (`email`, `login`, `githubLogin`, or
`userId`) so the share owner and source-access checks run as that selected
human user instead of as an anonymous operator.

The typed request is:

```txt
{
  email?: "chris@openagents.com",        // admin-token path only
  target?: { email?: string, userId?: string, login?: string },
  source: { kind: "agent-run" | "team-thread" | "team-project-thread", id },
  audience: { _tag: "Public" | "TeamMembers" | "Users", ... },
  redactionPolicyId?: string,
  expiresAt?: string
}
```

It should return:

```txt
{
  id: "...uuid...",
  url: "https://openagents.com/share/...uuid...",
  audienceLabel: "Shared with members of OpenAgents Core Team",
  status: "active"
}
```

`GET /api/share/{uuid}/v1/data` returns the versioned projection. Public
audiences can be unauthenticated. Team/user audiences can be authorized with a
browser session, the admin bearer token, or a programmatic agent bearer token;
the same membership/recipient rules still apply except the admin token uses an
admin viewer.

`PATCH /api/share/{uuid}` updates audience, title, expiry, or revocation state.
It accepts the same browser-session, admin-token, and agent-token auth surfaces.
It should not mutate the source run/thread. It should emit a receipt or audit
event for audience changes because changing a share from team-only to public is
a material privacy/security action.

`DELETE /api/share/{uuid}` should revoke the share. Prefer a tombstone over a
hard delete so old links can render a clear revoked state and audit trails can
survive.

All endpoints should decode with Effect Schema and return typed errors. Avoid
the OpenCode pattern where the hosted sync route accepts broad unvalidated
arrays and trusts downstream call sites to have already shaped the payload.

### Effect Services

The implementation should be an Effect service stack rather than route-local
logic. Useful service boundaries:

```txt
ShareProjectionRepository
  create
  readById
  updateAudience
  updateProjection
  revoke

ShareProjectionBuilder
  fromAgentRun
  fromTeamThread
  fromTeamProjectThread

ShareAccessService
  authorizeCreate
  authorizeView
  authorizeUpdate
  authorizeRevoke

ShareUrlService
  canonicalUrlForShareId

ShareReceiptService
  recordCreated
  recordAudienceChanged
  recordRevoked
```

The canonical URL constructor should have exactly one owner:

```txt
ShareUrlService.canonicalUrlForShareId(id) => /share/{uuid}
```

No web component, sync client, CLI command, issue automation, or email renderer
should construct a share URL by hand. They should use the returned canonical URL
from the create endpoint or the same service behind that endpoint.

### Projection Shape

The projection should be close enough to the existing workroom timeline model
that the share page can reuse OpenAgents product surface's chat components:

```txt
ShareProjectionV1
  id
  url
  audience
  audienceLabel
  title
  subtitle
  source
  status
  createdAt
  updatedAt
  messages: WorkroomTimelineMessage[]
  files: WorkroomFileItem[]
  artifacts
  approvals
  receipts
  metrics
```

The projection builder should decide field by field what is visible:

- user and assistant messages
- tool summaries
- shell/status events
- diffs and changed files
- artifacts
- approvals
- receipts
- model/provider labels
- timestamps
- repository owner/repo/ref
- file paths
- token/cost metrics
- team/project names

The rule should not be "copy the run bundle." The rule should be "construct a
public or restricted projection from the run bundle." Even team-only shares need
redaction, because teams are not the same boundary as runtime credentials.

### Web UI Parity Target

To reach parity with the OpenCode share page, OpenAgents product surface needs a full read-only
workroom page, not just a text transcript. The minimum page structure should be:

```txt
top share header
  audience label
  title/source/status row
  copy link / open source / revoke or manage controls when authorized

main split layout
  center: read-only chat/workroom timeline
  right: files, diffs, artifacts, approvals, receipts, metadata
```

The route should reuse existing workroom primitives where possible:

- `apps/web/src/ui/workroom.ts` for shell, timeline, side panels, file rows,
  metadata rows, and status rows
- `apps/web/src/page/loggedIn/run-timeline/projection.ts` for converting
  agent run events into `WorkroomTimelineMessage`
- `apps/web/src/page/loggedIn/page/chat.ts` for team chat and team project chat
  projection ideas

Missing or likely-needed web UI pieces:

- share page shell that works for signed-out, signed-in, public, team, and
  user-targeted viewers
- audience banner/header component
- read-only share top bar with copy-link and management affordances
- read-only workroom timeline composition that does not require a logged-in
  `Model`
- share side panel for files, diffs, artifacts, approvals, receipts, and source
  metadata
- empty, loading, revoked, expired, sign-in-required, and forbidden states
- mobile layout where the transcript remains primary and side panels become
  tabs or drawers

The composer should not appear on the share page unless OpenAgents product surface later adds a
commenting/follow-up workflow. If comments are added later, they should be a
separate explicit feature with their own access model.

### Concrete Porting Boundary For OpenAgents product surface

The narrowest useful implementation slice is:

```txt
GET    /share/{uuid}
POST   /api/share
GET    /api/share/{uuid}/v1/data
PATCH  /api/share/{uuid}
DELETE /api/share/{uuid}
```

The route changes live in the normal web route model:

- add `ShareRoute`
- add `shareRouter`
- parse `/share/:shareId`
- render a top-level share page for both anonymous and authenticated viewers

The Worker changes live in normal Effect route modules:

- add `workers/api/src/share-projections.ts`
- add `workers/api/src/share-routes.ts`
- wire them from `workers/api/src/index.ts`
- add D1 migrations for projection and recipient storage
- add tests for create, view, audience authorization, revoke, and malformed IDs

The web changes should include:

- `apps/web/src/page/share/model.ts`
- `apps/web/src/page/share/view.ts`
- `apps/web/src/page/share/update.ts`
- shared projection utilities that can be used without the logged-in workroom
  model
- scene tests for public, team-only, user-only, revoked, and unauthorized
  states

The most important acceptance checks:

- `/share/{uuid}` is the only generated page URL
- public shares render without auth and display `Shared publicly`
- team shares require membership and display
  `Shared with members of OpenAgents Core Team` for the example team
- named-recipient shares require auth and display `Shared with you` to the
  recipient
- restricted share data is never embedded before authorization
- revoked shares stop rendering projection data
- canonical URLs are returned from the API and not reconstructed elsewhere
- chat transcript, tool/status rows, diffs/files, artifacts, approvals, and
  receipts render with parity to the OpenCode share page
- Effect Schema decoders reject invalid create/update/data payloads
- redaction tests prevent provider secrets, bearer tokens, callback URLs,
  auth-grant refs, and raw runtime payloads from entering projections

## OpenAgents product surface Implementation Addendum

Date: 2026-06-05

Issues covered:

- <https://github.com/OpenAgentsInc/openagents/issues/85>
- <https://github.com/OpenAgentsInc/openagents/issues/86>
- <https://github.com/OpenAgentsInc/openagents/issues/87>
- <https://github.com/OpenAgentsInc/openagents/issues/88>

OpenAgents product surface now owns a first implementation pass for the canonical share route:

```txt
/share/{uuid}
```

The share page is part of the normal OpenAgents product surface web application. There is no
separate share Worker or hosted sidecar. The page route is parsed by
`apps/web/src/route.ts`, rendered through the logged-out public route path, and
loads projection data from the normal API Worker:

```txt
GET /api/share/{uuid}/v1/data
```

The Worker API surface now includes:

```txt
POST   /api/share
GET    /api/share/{uuid}/v1/data
PATCH  /api/share/{uuid}
DELETE /api/share/{uuid}
```

The shared schema contract lives in `packages/sync-schema/src/share.ts`.
Important boundary types:

- `ShareAudience`
- `ShareSource`
- `ShareCreateRequest`
- `ShareUpdateRequest`
- `ShareProjectionV1`
- `WorkroomTimelineMessage`
- `WorkroomFileItem`

The D1 persistence migration is:

```txt
workers/api/migrations/0037_share_projections.sql
```

It creates `share_projections` and `share_projection_recipients`. The main row
stores the canonical URL, source kind/id, owner, team/project scope, serialized
audience, serialized projection, redaction policy id, lifecycle timestamps, and
revocation/expiry fields. The recipient table stores team/user/email subjects
for explicit share-audience lookup.

The Worker implementation is split as:

```txt
workers/api/src/share-projections.ts
workers/api/src/share-routes.ts
workers/api/src/worker-routes.ts
workers/api/src/index.ts
```

`share-projections.ts` owns typed errors, D1 repository functions, audience
labels, authorization, source projection builders, and redaction checks.
`share-routes.ts` maps those typed errors to HTTP responses and keeps the route
layer behind Effect services and local route aliases so it does not grow the
zero-tech-debt route budgets.

Audience labels are produced centrally:

```txt
Public      -> Shared publicly
TeamMembers -> Shared with members of <team name>
Users       -> Shared with you, Shared with <name>, or Shared with <N> people
```

The page header renders that label before the title and source metadata. The
team example therefore appears as:

```txt
Shared with members of OpenAgents Core Team
```

Restricted shares are not embedded into the HTML shell. The browser shell
loads `GET /api/share/{uuid}/v1/data` with credentials. The Worker returns:

- `200` with `{ projection }` for public shares and authorized restricted
  viewers
- `401` for restricted shares when no browser session or API bearer authority is
  present
- `403` for restricted shares when the viewer is signed in but unauthorized
- `410` for revoked or expired shares
- `404` for missing shares
- `400` for malformed share ids or invalid create/update payloads
- `422` when a built projection still appears to contain provider secret
  material

The web implementation lives in the logged-out public model because share URLs
must stay public-shell routes even for authenticated users:

```txt
apps/web/src/page/loggedOut/model.ts
apps/web/src/page/loggedOut/message.ts
apps/web/src/page/loggedOut/update.ts
apps/web/src/page/loggedOut/view.ts
apps/web/src/page/loggedOut/page/share.ts
apps/web/src/view.ts
apps/web/src/routing/startup.ts
apps/web/src/product-policy.ts
```

The share view uses existing OpenAgents product surface workroom primitives for the timeline, split
layout, side panel, metadata rows, files, artifacts, and receipts. It has
loading, sign-in-required, forbidden, revoked/expired, not-found, and loaded
states. The sign-in-required state links to:

```txt
/login/github?returnTo=/share/{uuid}
```

Implementation update after the OpenCode-structure UI pass:

- `apps/web/src/page/loggedOut/page/share.ts` now follows the OpenCode public
  share shape more closely: a slim global share bar, a session title block
  inside the transcript surface, a read-only transcript column, message
  navigation when multiple user messages exist, an optional right review pane,
  and a mobile review disclosure.
- The OpenAgents product surface version keeps the Vortex/OpenAgents dark mono theme, audience-first
  share labeling, existing copy/open-source actions, and the existing
  workroom/timeline primitives rather than copying OpenCode's Solid UI
  components.
- Review rows now include files, artifacts, approvals, and receipts, with share
  metadata shown both in the desktop rail and the mobile review disclosure.

`workers/api/src/index.ts` now stores a validated `oa_login_return_to` cookie
for clean first-party return paths and redirects successful OAuth callbacks
back to that share route. The cookie is scoped to `/auth` and cleared on
callback.

Implemented tests:

```txt
apps/web/src/route.test.ts
apps/web/src/routing/startup.test.ts
apps/web/src/main.test.ts
workers/api/src/share-projections.test.ts
workers/api/src/admin-access.test.ts
```

Those tests cover share route parsing, public-shell startup for anonymous and
authenticated users, no auth-bootstrap fetch for share URLs, loading
`LoadShareProjection`, audience-label formatting, public/user-restricted access
outcomes, revoked projection status, app-shell routing, share URL cleanup, and
OAuth return-target cookie storage.

Share UI naming update:

- The share UI normalizes internal `Adjutant` copy at the display boundary.
- Historical titles, subtitles, transcript text, tool details, and review rows
  can still originate from `agent_adjutant` data, but public share chrome should
  render that surface as `Autopilot`.

Known follow-up work after this pass:

- The create endpoint can project existing agent runs and team threads, but the
  logged-in UI still needs explicit "Share" controls on run/thread surfaces.
- The side panel currently lists files/artifacts/receipts; deeper unified and
  split diff rendering should be added when OpenAgents product surface's file/diff projection model
  is finalized.
- Approval and receipt projection should become richer typed objects instead
  of string refs once the approval/receipt domain model is stable.
- Redaction currently uses provider-secret material detection plus field-level
  safe text extraction. Higher-assurance redaction should add direct fixtures
  for bearer tokens, callback URLs, auth grant refs, provider account refs, and
  raw runtime payload samples.
- Expiry is enforced at read time. A later cleanup job can compact expired or
  revoked projection payloads if storage pressure appears.

## Bottom Line

The OpenCode share page is a SolidStart public read projection backed by an
object-store snapshot API. The publishing pipeline is an Effect service in the
local runtime that creates the hosted share, stores a write secret locally, and
syncs session/message/part/diff/model records as session events occur.

The implementation is pragmatic and useful to study, but it is intentionally
public-link sharing, not controlled collaboration. The exact code path for the
provided URL is `packages/enterprise/src/routes/share/[shareID].tsx`, with
read data supplied by `packages/enterprise/src/core/share.ts` and writes driven
by `packages/opencode/src/share/share-next.ts`.

The most important OpenAgents product surface lessons are:

- use `/share/{uuid}` as the canonical page route
- model share audience explicitly as public, team, or named users
- show the audience at the top of the page before the transcript
- centralize canonical URL construction
- never derive public URLs in peripheral automation
- separate public URL from write secret
- validate sync payloads deeply
- publish redacted, versioned projections instead of raw internal sessions
- keep the implementation in the normal OpenAgents product surface Effect application, not a
  separate Worker
- treat share-link privacy as a product/security policy, not just a docs note
