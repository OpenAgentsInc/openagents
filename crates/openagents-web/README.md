# OpenAgents web

This Rust server is the OpenAgents website: the public pages that
openagents.com serves, the terms of service and the privacy policy, the one
download page, and the landing page for the pairing QR code. It also serves a
local, read-only task browser at `/app`.

The site and the Cloud app use the Coder Light / Coder Noir design language
from `openagents-ui` (see "Styles"). The `/components` catalog, `/demo`, and
the full-screen canvas pages keep Coder Noir from `coder_ui::coder_noir`:
Superlogical's Static Noir base with neutral Coder accents and cursors, whose
`--noir-*` CSS variables preserve the native palette, including status colors
and translucent control states. The game scenes retain
their content colors. All text uses the system font stacks from Apps SDK UI: the system sans for
UI text and the system monospace for code, terminal grids, and the wordmark.
The site serves no font files.
The interactive pages run scripts under a
policy that allows same-site scripts and same-origin requests: the
homepage and chat composer (HTMX with a small Rust/Wasm interaction adapter), `/live`'s
map (`static/flow.js`, requests to `/api/flow/*`, drawn on a canvas in the
application theme's colors), and `/everglade`'s loader (`static/everglade.js`),
whose policy also allows `'wasm-unsafe-eval'` to compile the Everglade
build. The component catalog permits its same-origin Rust/Wasm loader when
configured. Other responses carry a policy that allows no script.

The pages follow the private Coder service's site (`bins/coder-serve` in
the `coder` repository), reimplemented here. See
[the deployment record](../../docs/deployment/openagents-web.md) for the
public site's Rust image and its remaining proxied services.

The [Coder Cloud specification](../../docs/cloud/coder-cloud.md) defines the
proposed authenticated web workspace, Verse connection, and commercial
interfaces. `/cloud/app` uses explicit native account sessions and separately
granted resident observation; the local task browser keeps its loopback-only scope.

Its first deliverable is the public
[`/components` catalog](../../docs/coder/rust-native/coder-components.md): web
versions of every `coder-new` presentation component and state, composed from
the shared Rust Native Coder library. The catalog composes `coder-ui` views through the reusable `rust-native-web`
adapter. Synthetic interactive fixtures and full screen previews run their
local state controller in Rust/Wasm. These examples connect to no live host,
provider, or account.

The direct-link `/demo` page opens a synthetic environment onboarding chat,
from repository discovery and install repair through clean image build,
fresh verification, Save, and a first task on the saved version. Its chat
sidebar also keeps the original five demo conversations, with independent
drafts, messages, and scroll positions. **Beginning** and **Latest** navigate
the selected history. Keyboard controls, plugin settings, and the model picker
remain available. Axum and Maud render the shared `coder-ui::demo` fixtures as
HTML; HTTP fragments and SSE update the selected conversation. The small
`coder-chat-web` Rust/Wasm adapter preserves drafts, caret, and scroll in page
memory. Ordinary chat and demo pages do not load the catalog's Wasm build.
The demo has no site header or footer and starts no provider work.

## Durable public chat

The homepage submits `POST /chat` and redirects to `/chat/{uuid}`. Axum owns
conversation commands and worker observation; Maud renders HTML, and HTMX
performs HTTP fragment swaps and receives SSE updates. The small
`coder-chat-web` adapter keeps immediate editing, drafts, focus, and scroll in
the browser. It uses no persistent browser storage and grants no execution
rights. The existing chat worker still answers with the `web` policy.

Each server-issued form carries a UUID request identity and cookie-bound CSRF
ticket. The `oa_visitor` HttpOnly cookie scopes every conversation read and
write. Exact retries recover the existing request; a changed message with the
same identity is refused. A durable visitor lease admits one answer at a time
across replicas. Answers continue independently of the browser's SSE connection.

`GET /chat/{uuid}/workspace` swaps the selected chat, `/transcript` pages its
retained messages, and `/events` sends revision-tagged transcript snapshots.
Superseded projections produce an explicit gap notice. Original message text
remains available through `/messages/{index}/original` in bounded chunks;
rendering escapes source text and does not execute returned markup.

Use `--chat-store DIRECTORY` for restart-durable local records. It uses private
files, operating-system locks, fsync, and atomic replacement. Production uses
`--chat-bucket BUCKET`, a private Google Cloud Storage bucket under the
`conversations/` prefix. Metadata-service OAuth uses the runtime service account,
which needs object read, create, delete, and list permissions. Enable bucket object versioning so a read can finish against its captured
generation while another replica writes. Object-generation
preconditions fence concurrent writes; storage failure never selects a local
fallback. Set the same secret `OPENAGENTS_WEB_ASK_SALT` (64 hex characters) on
all replicas and revisions for stable worker identities and CSRF tickets.

Build `coder-chat-web` for `wasm32-unknown-unknown`, generate its JavaScript glue
with `scripts/build-coder-chat-web.sh OUTPUT`, and pass that output as
`--chat-build DIRECTORY`.
The directory supplies `coder_chat_web.js` and `coder_chat_web_bg.wasm` only.
The worker door cannot resume a lost process or recover ephemeral relay events.
An interrupted pending request becomes `Unknown` when next observed after
180 seconds, preserving its partial answer and preventing automatic replay.
Public deployments require the bucket and shared salt at startup.
Production must retain CPU outside requests for background answer observation.
The former `POST /ask` route returns `410` and links to the homepage.

## Source and runtime selections

The homepage and chat share repository, branch, and Environment controls.
`GET /composer/{kind}` renders a bounded selector; `POST` updates signed,
visitor-bound selection state and, for an existing chat, saves it with a
generation precondition. Selection changes preserve the textarea. Each accepted
message freezes its source and runtime pins; exact retries include those pins.

Public source selection uses GitHub's public repository and branch endpoints
at `api.github.com`, with the [versioned REST API](https://docs.github.com/en/rest/about-the-rest-api/api-versions)
header `2026-03-10`, no credential, no redirects, a 512 KiB response bound, and
at most 100 listed branches. A branch selection resolves its current 40-character
commit. Metadata supplies question context and authorizes no repository commands.

An authenticated native operator can choose admitted projects and base runtimes.
The server rechecks the current account, workspace membership, grant, profile,
and exact source before staging the existing reviewed Cloud request. A retained
chat reference opens its actual native review or canonical job at `/chat/{uuid}`;
the native journal owns confirmation, continuation, recovery, and original output.
Frozen native references keep the chat private after a later selection change.
Cloud session cookies cover the site, and the host guard refuses to forward them
to the legacy upstream. Without configured native access, the selector explains
availability and links to Cloud access. It starts no simulated job.

**Context** opens source controls. **Model** explains the current web policy or
native profile and links to runtime selection. **Voice input** reports its
current availability; browser speech capture is not implemented. A base runtime
does not establish a saved environment or an awake development server. See the
[managed computer plan](../../docs/cloud/managed-computers.md) and
[glossary](../../docs/glossary.md#cloud-computers-and-repository-environments).

## Run the component catalog

From the monorepo root, use an external build directory and a scratch asset
output directory:

```sh
export CARGO_TARGET_DIR="$HOME/work/openagents-target-agent0"
scripts/build-coder-components-web.sh "$(openagents scratch)/components-build"
openagents lease build --keep-target-dir -- cargo build -p openagents-web
"$CARGO_TARGET_DIR/debug/openagents-web" --store "$(openagents scratch)/unused-tasks" \
  --components-build "$(openagents scratch)/components-build"
```

Open `http://127.0.0.1:4300/components`. Select a component, its fixture, and
viewport dimensions. The full screen link isolates its Coder presentation.
Without the Wasm build, the same routes render readable HTML previews.
`/components/manifest.json` lists the source references and named variants.

The build script requires the pinned `wasm32-unknown-unknown` target and the
`wasm-bindgen` CLI version in `Cargo.lock`; it emits no TypeScript.

## Run it

From the monorepo root:

```sh
cargo run -p openagents-web -- --store "$HOME/.openagents/tasks"
```

Open `http://127.0.0.1:4300`. The server binds to loopback and answers only
the `Host` headers `127.0.0.1:4300` and `localhost:4300`.

| Option | Default | Meaning |
| --- | --- | --- |
| `--store DIRECTORY` | `~/.openagents/tasks` | The task store `/app` reads. It is never created. |
| `--listen ADDRESS` | `127.0.0.1:4300` | The address to bind. |
| `--public-host HOST` | none | Another `Host` header the public pages answer, such as `openagents.com`. Repeatable. The task browser still answers only the local hosts. |
| `--chat-store DIRECTORY` | A sibling `web-chats` directory next to `--store` | Private, restart-durable public conversation records on one machine. |
| `--chat-bucket BUCKET` | none | Private shared conversation records and visitor leases in Google Cloud Storage; replaces local chat storage. |
| `--chat-build DIRECTORY` | none | Generated `coder_chat_web.js` and `coder_chat_web_bg.wasm` for composer and scroll interaction. |
| `--components-build DIRECTORY` | none | The generated `coder_components_web.js` and `coder_components_web_bg.wasm` files for local catalog interaction. Only these names are served. |
| `--cloud-build DIRECTORY` | none | Rust/Wasm private-view lifecycle assets built by `scripts/build-coder-cloud-web.sh`, plus the granted workbench terminal from `scripts/build-coder-browser-web.sh`; the site images build both into `/srv/cloud`. Private content waits for current account and resource standing before display. |
| `--cloud-config PRIVATE_JSON` | none | Explicit account-service origin, public origin, and protected CSRF key. Native user sessions and current workspace membership scope each request. |
| `--cloud-hosts PRIVATE_JSON` | none | Protected account/workspace/epoch bindings to host-signed Observe grants, device keys, and exact host routes and generations. No host enrollment or task effect comes from sign-in. |
| `--cloud-retail PRIVATE_JSON` | none | Protected account/workspace/epoch delegations to the retail service through native `retail-client` configurations, plus a private site directory for key custody and request journals. No funding, quote, confirmation, or cancellation right comes from sign-in. |
| `--cloud-sales PRIVATE_JSON` | none | Protected account/workspace/epoch delegations to the separate sales-owner remote adapter (endpoint, binding, and that binding's bearer file), plus a private site directory for request journals. The site never opens the pipeline; no sales right comes from sign-in or membership. |
| `--cloud-byo PRIVATE_DIR` | none | An owner-only directory for customers' own Claude credentials (Anthropic API key, Bedrock, Vertex, or Foundry) behind **Settings → Manage Claude credential**. |
| `--cloud-team PRIVATE_JSON` | none | The owner-written browser qualification that enables `/cloud/app/team` lanes; see "Team membership, policy, limits, and reports". |
| `--everglade DIRECTORY` | none | The Everglade web build (`scripts/build-everglade-web.sh`'s output, `everglade_web.js` and `everglade_web_bg.wasm`) with the pinned pack under `pack/`, served at `/everglade`. Without it, `/everglade` says Everglade is unavailable. |
| `--pilot-config PRIVATE_JSON` | none | Explicit task root and create-only intake credential. `/pilot` answers 404; POST intake stays available to the configured pipeline. |

A development server renders public pages without secrets. Sending a real
chat message reaches the OpenAgents chat worker; synthetic demo conversations
connect to no provider or account.

## Styles

Every page renders through `src/ui_page.rs` (`UiPage`) and the
`openagents-ui` components, and links one stylesheet: `/static/ui.css`, the
`openagents-ui` bundle (Coder Light and Coder Noir from the same tokens).
Add styles there, as a component stylesheet in
`crates/openagents-ui/static/components/`, never as page CSS here. There is
no Tailwind build and no inline `style`.

Area stylesheets that remain, each loaded only by its own pages:

- `static/cloud.css`: the Cloud app's workspace layout and shared parts
  (`cloud-*` classes), on `openagents-ui` tokens.
- `static/legacy-demo.css`: the Coder Noir base `/demo` and the full-screen
  canvas pages (`/everglade`, `/druid`, `/grid`, the Verse world) keep on
  purpose, with `static/demo-html.css` for `/demo`. Served with the
  `--noir-*` variables from `src/palette.rs`.
- `static/components.css` and `static/demo.css`: the `/components` Rust
  Native catalog, also on `--noir-*`.

Tests hold the line: a `UiPage` page links only `/static/ui.css`, within
`openagents_ui::css_classes::STYLESHEET_BUDGET_BYTES`; every class it renders
has a rule (script hooks excepted); and none carries an inline `style`.

## Pages

| Route | Source | Development server |
| --- | --- | --- |
| `/` | A composer, centered between the header and the footer, that starts `/chat/{uuid}` | Renders. |
| `/chat`, `/chat/{uuid}` | Durable visitor-owned worker conversations started from the homepage composer | Local storage survives restarts; answers require the live chat worker. |
| `/download` | `src/pages/download.rs`: the notarized OpenAgents for Mac `.dmg` in `openagentsgemini-oa-updates`, OpenAgents Terminal's install commands, and one link to build everything else from source | Renders. |
| `/pilot`, `/pilot/install` | Archived Coder-pilot offer copy in `src/pilot.rs` (`ARCHIVED_OFFER`, `ARCHIVED_INSTALL`) | `404`. |
| `/install`, `/desktop` | Permanent redirect (`308`) to `/download`, so older links keep working | Redirects. |
| `POST /ask` | Retired homepage terminal route; links to the homepage without dispatching work | `410`. |
| `/docs`, `/docs/{slug}` | `content/docs/*.md`, short guides in reading order, compiled in: what OpenAgents is, download (`/docs/install` redirects to `/docs/download`), connecting a computer, chat, Coder, plugins (what they are, writing, testing, publishing and sharing), the Verse, the Grid (with its screenshot, `static/verse-grid.jpg`, captured from the live relay with `crates/verse/examples/overlook_capture.rs`), privacy and security, and help | Renders. |
| `/live` | `src/pages/live.rs` and `static/flow.js` (#10197): the route map drawn from the pay host's flow snapshot, each streamed event animated as the desktop deck's `routes-live` scene does (white request out, gold payment back, gold share to the author, gold payout to the wallet, a ring for a bonus), a totals ticker, the last event's time, and the recent events. Reads `/api/flow/snapshot` and `/api/flow/stream` on this origin (#10195); never draws synthetic traffic. The mapping's tests are `static/flow.test.js` (`node --test`) | Says the flow stream is unreachable until `/api/flow/*` answers. |
| `/everglade` | `src/pages/everglade.rs` and `static/everglade.js` (#10525): a canvas that fills the window, with no site header or footer and no page zoom, and a loader that imports the Everglade web build's glue (#10524) and calls its `init()`. `/everglade/{file}` serves the `.js` and `.wasm` files in the `--everglade` directory (five minutes' cache) and `/everglade/pack/{sha256}.vtp` the digest-named pack in its `pack/` (a year's immutable cache); nothing else on disk. Policy: same-origin scripts and requests and `'wasm-unsafe-eval'`. The Verse guide links it. The glue's file name is `GLUE` in `src/pages/everglade.rs` and must match the build script's output | Says Everglade is unavailable unless started with `--everglade DIR`. |
| `/stats` | `src/pages/stats.rs` (#10196): drawn on the server from the pay host's public `/stats` and `/flow/snapshot` (#10195): received, paid out, pending, calls, and author earnings; plugins (calls, earned, paid out); authors (earned, paid out, pending); the 20 most recent author payouts; 24 hour and 30 day bars of sats received (inline SVG); the reconciliation state and the last event's time. Linked from `/live` and linking back. No script; public fields only, never a payer | Says the statistics are unreachable without a pay host, and "No payments yet" with an empty ledger. |
| `/terms`, `/privacy` | `content/legal/*.md`, the published text (2026-09-03), compiled in | Renders. |
| `/connect` | Landing page for `https://openagents.com/connect#<code>` | Renders; no script, no referrer. |
| `/.well-known/apple-app-site-association`, `/.well-known/assetlinks.json` | Universal link and App Link claims for `/connect` | Serves. |
| `/u/{login}` | `Backend::profile` | Says the backend isn't connected. |
| `/components`, `/components/{component}` | Shared Coder components, named synthetic variants, typed controls, source references, and full screen previews | Renders; Rust/Wasm interaction requires `--components-build`. |
| `/demo` | Server-rendered environment onboarding and original Coder fixture conversations, with HTTP and SSE updates | Renders without the catalog Wasm build; local editing requires `--chat-build`. |
| `/cloud`, `/cloud/sign-in`, `/cloud/app` | Public availability and the native account/workspace shell | Public entry renders; private pages require explicit native account configuration and the Cloud Wasm build. |
| `/cloud/app/hosts/{binding}/tasks`, `/cloud/app/hosts/{binding}/tasks/{task}` | Bounded, signed resident task reads under current Observe authority; original ATIF messages, tools, child references, checks, cost, and source pins | Requires a separately provisioned host binding. It reads no local `/app` records. |
| `/app`, `/app/tasks/{id}` | The local task store | Reads the store; local hosts only. |

The header links Download and Docs; the footer links the terms and the
privacy policy. Cloud, Components, and Demo remain direct-link pages.

The Forum, Gym, Traces, Earn, Weights, and QA sections of the old site are
not served and not linked (owner-directed, 2026-09-29). Neither is anything
for the old Coder Terminal product, which is not connected to OpenAgents:
the `/releases/{name}` proxy to its release bucket, the
`/install-terminal.sh` and `/install-terminal.ps1` install commands, and the
old Docs (`/doc`) and Blog (`/blog`) sections, whose every document was
about it (owner-directed, 2026-09-29). Today's `/docs` is new: guides to the
apps we launch and to plugins, none about Coder Terminal.

Pages that read accounts go through the
`Backend` trait in `src/backend.rs`. The development backend is connected to
nothing: those pages render, say that their data needs the production
backend, and show no records. A production backend answers `connected()`
with `true`; then a missing profile answers `404`.

## Resident Cloud observation

Cloud host bindings use schema `openagents.cloud.host-bindings.v1` and a
`bindings` array. Each binding names `id`, native account `account`, selected
tenancy `workspace`, `members_epoch`, resident `host_workspace`,
`host_generation`, `route`, `access_file`, and `device_secret`. The last two
are absolute paths to a native saved Access record and a private device key.
The route is verified `wss://HOST/`, or literal loopback `tcp://IP:PORT` for
local adapters and isolated fixtures. Configuration and key files require
private ownership and permissions; replacement fences reads until reload.
Browser input chooses no path, key, endpoint, or execution configuration.

Native `task.list`, `task.read`, and `task.original` scope every response to
the admitted resident workspace. Prefix cursors preserve exact task, attempt,
revision, and source identity. Oversized records leave explicit gaps and
bounded original byte reads. Losing the grant, account, workspace, source pin,
or active page clears the private mount; reconnect requires fresh navigation.

## Reviewed resident controls

To admit server custody, add `controls` to the protected host configuration:
`{"directory":"ABSOLUTE_PRIVATE_DIRECTORY","bindings":["BINDING_ID"]}`.
The directory must already exist, be owned by the operator, and have mode
`0700`. A native host owner separately issues the device grant. Sign-in never
issues that grant. Each browser session reviews the exact account, workspace,
host generation, device, rights, expiry, and server custody at
`/cloud/app/hosts/{binding}` before controls become available.

Creation, commands, queue edits, and publication first retain a reviewed
request and its original signed native packet. Confirmation dispatches that
packet under current account and host authority. Repeated confirmation never
creates another packet. The resident checks task revisions, queue digests, and
publication candidates. A create request submits an inert task unless the
resident owner has separately enabled auto-start. Explicit executor selection
stays in the native task request.

`/cloud/app/hosts/{binding}/requests/{request}` recovers the original sealed
native result after reconnect or restart. Unknown execution or cleanup stays
unknown; an expired packet requires observation rather than redispatch.
Request journals contain private operation content and must remain private.
The browser keeps no credential, prompt, or action in persistent storage.

## Projects and operator Cloud jobs

`/cloud/app/projects` links the current resident bindings. Project pages use
the resident's separately configured `--project-observer PRIVATE_JSON` policy
over retained `coder-project` supervisor records. Reads preserve native claims,
dependency IDs, capacity, review pressure, exclusions, and worktree evidence;
they never open a scheduler writer or start issue work. Missing goal, tracker,
provider-reset, or build-wait evidence remains unknown. Snapshot-pinned pages
and original byte chunks refuse a changed source.

Operator jobs use the resident's `--cloud-operator PRIVATE_JSON` policy over
`coder-cloud`. It admits exact devices, workspaces, projects, source revisions,
pools, executor profiles, and credential names. The browser can choose only
the admitted aliases. Native configuration supplies source paths, provider
endpoints, and credential files. Native retail admission remains separate.

Profile forms and existing job pages stage submit, continue, stop, or reconcile
requests in the same private control journal. Confirmation dispatches the
original packet; repeated confirmation recovers its original job identity.
Ordinary reads never drive the worker. Requested and served models, usage,
retained artifacts, cancellation, and cleanup retain independent evidence;
missing delivery, publication, and cost remain unknown. Continuation requires
the native owner's terminal-state and cleanup admission.

Private pages pin the original bounded native projection and current policy.
Policy, source, membership, grant, or enrollment changes retire the mounted
view and drafts. Even a prepared request rechecks its operator policy before
revealing private action content. Original records are available in bounded
chunks with downloads that preserve their exact bytes.

## Alice and Agent Studio

`/cloud/app/agents` links the current resident bindings. On
`/cloud/app/hosts/{binding}/agents`, Observe reads the host's workshop agents
and its Agent Studio snapshot. Each agent shows her key, attested, unattested,
or expired owner attestation with the 14-day renewal warning, engine route,
running step, waiting proposal, coding-task change, jobs, preferences, and day
plan. The page states the current one-running, four-waiting host limit and that
coding-task runs bypass her terminal-request budget meter. Configuration stays
unavailable until a reviewed host operation exists.

`/cloud/app/hosts/{binding}/agents/{agent}` adds standing jobs and memory. An
enrolled browser with Operate chooses **Coding task** or **Terminal request**
explicitly; the host's automatic mode is never sent. The request identity is
fixed when the form renders. The control journal reuses the exact packet, and
changed bytes under that identity conflict. The resident retains the signed
reply, and the agent host keeps a durable ledger of request identity and exact
content. A lost reply or restart therefore recovers the original result without
queueing the work again.

Studio controls carry the displayed stream and sequence. Before staging, the
adapter reads `studio.update` from that point. A gap, restarted host stream, or
changed decision or task refuses until a fresh snapshot. Decisions bind their
native basis. Merge decisions bind the review's base, head commit, and tree.
The adapter rereads the review and refuses a changed candidate. Merge is not
offered for an unreadable change. The host refuses dirty, detached, or
conflicting checkouts. Merge implies no deploy; any push appears only as the
host's publication state.

## Granted native workbench

Build `coder-browser-web` with `scripts/build-coder-browser-web.sh DIRECTORY`
into the same `--cloud-build` directory as the Cloud privacy adapter. Add
`browser` to an explicitly admitted host binding, with an optional secure
WebSocket `route` and the native host's `capabilities`. The binding's existing
signed access record supplies the relay identity. These public connection pins
grant no terminal rights and expose no server device key.

`/cloud/app/workbench` links configured native hosts. On
`/cloud/app/hosts/{binding}/workbench`, the browser creates a fresh page-memory
device key and redeems a separately issued native host invitation. Its Terminal
right is host-wide; the account workspace is navigation context. An additional
Observe right permits the original retained native thread reader. Sign-in and
the BFF's own device grant do not enroll the browser.

The Rust mount reads existing native saved sessions and preserves their original
work references, member states, pane identity, and revisions. It mounts the
shared `terminal-core` and `terminal-gfx` renderer through WebGPU or WebGL2,
with readable text and keyboard accessories when needed. Watch attachments
remain read-only; only the current typist can input or resize. Exact native
proposals retain their command, directory, context, and revision before review.
IME commits once, and clipboard controls require a gesture.

Route loss drops input without replay, clears private terminal state, and
requires fresh enrollment and a retained snapshot. Hidden or retired pages
cancel pending work and erase keys, drafts, and mounted private output. Closing
the page detaches its viewer and leaves the PTY under the native host lifecycle.
Retail customer tasks expose no shell. HTTP and insecure WebSockets are accepted
only for explicit isolated loopback fixtures.

## Verse connections

`/cloud/app/verse` always shows three separate connections for each admitted
host binding: World, Computer, and Private work. It needs no 3D view. Computer
state comes from a current native observation check. Private work lists only
the binding's Observe, Operate, Review, and Terminal rights; the `world` right is
never one of them, and joining a world adds none. Public worlds (`/grid`,
`/everglade`, `/druid`) stay linked and supply no host or Studio connection.

To offer **Join**, add `world` to the binding: an absolute private directory
(every containing directory `0700`, files `0600`) holding the browser
`chamber.json` from [platform clients](../../docs/verse/platform-clients.md) and
its pack, scene, and assets. Its `host` and `generation` must match the binding,
and the binding's native grant must carry `world`. The host still admits only an
enrolled character bound to the chamber's instance and content.
`/cloud/app/hosts/{binding}/verse` issues a ten-minute world ticket and opens the
existing `everglade-web` chamber renderer under `/cloud/world/{binding}/{ticket}/`
(`--everglade` must be set). The renderer omits credentials, so its
configuration and content reads are scoped by that ticket, which binds the
session, account, workspace, membership epoch, and exact binding identity. A
changed chamber configuration, binding, or expired grant refuses them. **Leave**
returns to the connections page.

`/cloud/app/verse/open?host=BINDING&resource=REF` resolves a station's
[workbench reference](../../docs/terminal/workbench-resources.md) to the same app
view: a terminal opens the workbench, a run opens its task, and other kinds show
the exact reference without an action until their owner viewer lands.

## Retail delegation

`/cloud/app/billing/retail` reaches the [retail service](../../docs/cloud/retail-service.md)
only through an operator-provisioned delegation. `--cloud-retail` names a
private `openagents.cloud.retail-delegations.v1` file:

```json
{"schema":"openagents.cloud.retail-delegations.v1","directory":"/abs/private/site",
 "delegations":[{"id":"alice-retail","account":"alice","workspace":"alice-personal",
   "members_epoch":3,"client":"/abs/private/alice/client.json","read_only":false}]}
```

`client` is a native `openagents.compute-retail-client.v1` configuration (its
endpoint, principal, bearer file, and state directory); every file is private
and pinned, and a changed bearer, epoch, or configuration refuses. The server
calls the service with that principal and no `Origin`, so the service keeps
refusing browsers; the bearer never reaches the page. `read_only` narrows the
principal to observation, and the service's current observe, spend, execute,
and disclose rights are rechecked on every request.

Funding, quotes, confirmations, and stop requests carry a request identity
journaled under `directory/requests` before the native call. An exact retry,
also after a site restart, recovers the original invoice or funded execution;
other bytes under that identity conflict. A lost reply shows **Outcome
unknown** with **Retry the same request**.

The customer's own OpenAI key enters through a password field with explicit
custody consent and is kept in `directory/custody` (`0600`), scoped to the
account, workspace, epoch, and delegation. The page shows only its digest. It
is released only to the exact quote and confirmation that reviewed that
digest, and **Remove key from custody** zeroes and deletes it. The vault
([`cloud::custody`](src/cloud/custody.rs)) is the shared custody class for a
customer's own provider API keys; it refuses Claude.ai OAuth and
`claude setup-token` values for every material.

### Purchases

`/cloud/app/billing/retail/{delegation}/purchases` lists the delegation's
funded executions from the retail service; each opens a canonical purchase
page with its payer, the exact review confirmed on this page, the approval and
funded request, the admitted work and sandbox, retained progress, the meter,
hold and settlement, retained artifacts, the stop record, and cleanup
evidence. Payment, completion, acceptance, and publication are reported as
four separate lines: a checks verdict is not acceptance, and nothing here
applies or publishes a patch.

The first observation of each immutable part (payer, quote, approval,
request, sandbox, settled charge, artifact source, acknowledged cleanup) is
kept in `directory/requests/*.purchase.json` (`0600`) with the progress
events read so far and their source cursor. A reload or a site restart shows
the same purchase and reads progress on from that cursor; a later service
answer that differs is refused as **Purchase record changed**. Unknown usage
shows as held funds, unacknowledged deletion as **Cleanup unconfirmed**, and
provider loss as an ending that needs a new offer. Artifacts are shown only
when retention is complete and their SHA-256 matches the retained manifest.
The stop control on a purchase returns to it; observation-only scopes see no
control, and other accounts reach none of these pages.


## Sales delegation

`/cloud/app/sales` reaches the private sales pipeline only through the
resident owner adapter ([`coder::task::sales::remote`](../coder/src/task/sales/remote.rs)),
which runs on the sales-owner host beside the canonical
[`Store`](../coder/src/task/sales.rs). The site never opens that store and
retains no contact.

On the owner host, issue the bound human a principal credential with the
existing pipeline tools, then write a private (`0600`)
`openagents.sales.remote-bindings.v1` file:

```json
{"schema":"openagents.sales.remote-bindings.v1","root":"/abs/HOST_TASK_ROOT",
 "journal":"/abs/private/sales-remote-journal",
 "bindings":[{"id":"alice-sales","account":"alice","workspace":"alice-personal",
   "members_epoch":3,"principal":"writer-a","credential":"/abs/private/writer-a",
   "client_digest":"<sha256 hex of the site bearer>","effects":["update"]}]}
```

and serve it on numeric loopback behind authenticated TLS:

```sh
cargo run -p openagents-web --bin sales-remote -- serve PRIVATE_BINDINGS_JSON 127.0.0.1:4410
```

Each binding pins one browser actor, workspace, and membership epoch to one
existing sales principal and may only narrow it: `effects` lists the
operations it admits (`create`, `update`, `propose_handoff`,
`accept_handoff`, `reject_handoff`, `suppress`, `delete`; empty is
observation only), and the principal's recorded role still applies. Service
sales, acquisition, partners, and funnel journeys are not admitted here.
Every call reopens the store and rereads the credential, so revocation,
rotation, and revisions are rechecked on each read and effect. Requests with
an `Origin` or cookie are refused. List answers are summaries without
contact or record text; refusals are fixed codes.

The site's `--cloud-sales` file is `openagents.cloud.sales-delegations.v1`:

```json
{"schema":"openagents.cloud.sales-delegations.v1","directory":"/abs/private/site",
 "delegations":[{"id":"alice-sales","account":"alice","workspace":"alice-personal",
   "members_epoch":3,"endpoint":"https://sales-owner.example/v1/sales",
   "binding":"alice-sales","bearer_file":"/abs/private/alice-sales.bearer"}]}
```

A changed bearer, epoch, or configuration refuses; the bearer never reaches
the page. A stage change journals its request identity, parameters, and exact
command digest under `directory/sales-requests` before dispatch; the owner
adapter journals the exact bytes beside the store before applying them and
clears them once settled. An exact retry recovers the original receipt,
changed parameters conflict, a form from an older revision is refused, and a
lost reply shows **Outcome unknown** with **Retry the same request**, which
reconciles with the owner by identity and digest. Public pilot intake stays
create-only and separate.

### Sales modules

Each delegation links the read-only modules **Evidence and claims**
(`/cloud/app/sales/ID/evidence`), **Pilots and delivery** (`.../pilots`, with
`.../leads/LEAD/services/SALE` for one delivery), **Invoices and
fulfillment** (`.../invoices`), and **Journeys and weekly review**
(`.../journeys`), plus a per-record audit (`.../leads/LEAD/audit`). They read
the adapter's `records`, `delivery`, `claims`, `weekly`, and `audit`
operations; the owner's retention, suppression, and recipient checks fence
every answer. Agreements, acceptances, support, invoices, payments, and
fulfillment appear as separate exact records; unknown, disputed, and failed
outcomes stay listed. The claim register, weekly review, and audit are
owner-role only, and the audit returns one record's entries. No module has a
form: none signs, pays, publishes, books, qualifies, accepts, or cleans up.
Pilot views compare against the pinned `docs/sales/` kits by digest.

Two optional owner-side binding fields enable more detail. `"evidence"` names
the private service evidence root; delivery handoffs are reread there by
their retained digest (dependencies, known limits, retained artifacts,
support, and the planned offboarding, which stays unverified until a
verified cleanup record exists). `"weekly":{"input":...,"evidence_root":...}`
names the owner's private weekly manifest; the review is rebuilt on each read
and refused when its sources are stale against current custody.

### Sales floor supervision

A binding may also carry `"supervise": true`, an optional private
`"hires"` crew hiring book (read only), an optional `"mailbox_key"` (kept on
the owner host; without it an approval refuses), and the outbox effects
`outbox_decide` and `outbox_stop`. Supervision answers only when the bound
principal is the pipeline owner. `/cloud/app/sales/{id}/floor` then shows
Paul's queue, the crew and stations, certification, model reservations with
unknown holds, the floor report and escalations, outbox states (unknown
delivery is never resent), untrusted replies (no payload; they authorize
nothing), and meeting proposals (they book nothing), all without contacts or
message bodies, against the America/Chicago business day and the fixed USD 5
floor-wide ceiling.

`/cloud/app/sales/{id}/outbox/{proposal}` shows one exact subject verbatim
and offers no editing. **Approve** or **Reject** binds that subject digest and
the original outbox revision in the form and its CSRF target; the site
rereads the proposal before dispatch, and the owner rechecks the subject, the
controller, the reserved day, and current authority. Approval is not
dispatch. **Stop dispatch** pauses the outbox controller (an `owner_stop`
incident): pending handoffs are fenced and unknown deliveries keep their
state; restart stays an owner correction on the sales host. Both effects use
the same request journal, exact retry, and **Outcome unknown** recovery as
stage changes (`directory/sales-requests/outbox-*.json`).

The private Agora board (`.../floor/board`, refreshed every second) shows
counts only, never a bell event, record, or person-linked amount. An
observation older than three seconds, a failed refresh, or a lost session
renders a cleared board, and the page hides any board three seconds after it
arrived, so an inactive view does not keep one.

## Billing statements and commercial lanes

`/cloud/app/billing` is reachable once a workspace is selected. Each lane is
read from its own native owner with the viewer's current session; nothing on
these pages grants a spending, invocation, or publication right.

- `/cloud/app/billing/statements` reads the gateway's joined
  `openagents.joined-statement.v1` page (`joined=true` on the workspace usage
  route) and shows every original row typed in full: native source and
  canonical mapping (attribution only), source unit and units, conversion,
  quote, terms, reserved, charged, released (never a refund), returned, loss,
  recovered, conversion fee and remainder, allocations including plugin
  release author fees and payout references, and the original gateway
  projection joined by row key. Millisatoshis, satoshis, and currency
  millionths are never added together; a missing charge reads `unknown`; a
  row the page cannot type exactly refuses as **Statement record
  unreadable**. Payee earnings stay a separate section with their own
  cursors. `/statements/export` returns the same page as private NDJSON.
  Without a reviewed statement grant the section reads **Unavailable**.
- The same page lists each current retail delegation with the native account,
  each journaled funding invoice checked against the service's current
  record (a differing amount or hash refuses as changed), and each purchase's
  retained first-observed quote and settlement. A purchase not yet retained
  says so instead of estimating a charge.
- `/cloud/app/billing/decisions?door=ID` shows the gateway's current purchase
  context for one decision resource: payer workspace, role and membership
  epochs, invocation right, team policy, canonical mapping, and the exact
  price reference. A context naming another account, workspace, or membership
  epoch refuses. `/decisions/{door}/receipt?digest=sha256:…` reads the
  original verified receipt and its settlement claim. The browser offers no
  quote approval, payment, or invocation for these lanes; those stay with the
  installed customer client and its own native controls.

## Team membership, policy, limits, and reports

`/cloud/app/team` is enabled only by `--cloud-team PRIVATE_JSON`, an
owner-written `openagents.cloud.team-browser-qualification.v1` document whose
`surface` is exactly `browser`, whose `origin` is this site's public origin,
and whose `lanes` name any of `membership`, `recovery`, `policy`, `budgets`,
and `reports` (plus an `evidence` reference). A native, desktop, or mobile
qualification is refused at load, a changed file fences the lane, and each
section appears only for its own lane.

Every read and change uses the viewer's own native session through `jev`'s
team client, on the one selected workspace; no workspace comes from a URL.
Each form is a CSRF ticket bound to the account, session, selected workspace,
its membership epoch, and the exact subject, so a form reviewed before any
accepted invitation, role change, or removal refuses. A member's role is
read-only: the page offers no change, and a forged ticket refuses before the
native owner.

- Members and invitations: admins invite (single-use, expiring token shown
  once), withdraw, change roles, and remove; `/cloud/app/team/accept` accepts
  only the reviewed workspace and role. Each membership change explains that
  stored Claude credentials (BYO-04) belong to the earlier epoch and stay
  hidden until each member adds theirs again. `/cloud/app/team/watch` keeps
  connected observers rechecking standing and retires them when the role,
  membership, or epoch changes.
- Recovery: admins issue a single-use recovery token; `/cloud/recover`
  redeems it once (signed out) and shows the replacement key once. Recovery
  never restores a removed membership.
- Policy: the exact team policy reference, enabled and unsupported lanes,
  and, for admins, its rules. A browser change can only narrow: drop rules or
  bring the expiry earlier, under the exact reviewed digest.
- Limits: cumulative caps with reserved (in-flight holds), unknown, and
  settled amounts per workspace, team, and person; only the owner may lower
  caps or thresholds for the same roster, and nothing is reset.
- Reports: `/cloud/app/team/reports` shows the native team report under
  current read rights; `/cloud/app/team/export` returns that report and the
  workspace access history as one bounded private JSON file, refused if the
  membership changed during the read.

Department knowledge is admitted documents, workflows, and evaluations under
their own grants; it is not model training or an enterprise certification.

## Task browser

The browser reads the same durable task store and paged ATIF view as
`coder task list` and `coder task view`. It doesn't submit work or grant
execution rights. The task store contains private prompts, workspace paths,
and trace content, so the browser answers only a local `Host`, even when
`--public-host` is set. Do not expose it through a public reverse proxy. If
you have not created a task store, the browser shows an empty state without
creating one. Reload to read newer task state.

## Permissioned pilot intake

Intake runs beside the canonical pipeline and writes directly to
`coder::task::sales` under the explicit host task root. A deployment without
that private durable root keeps intake unavailable; this change adds no
remote pipeline transport or replica-local contact store.
It creates a `New` lead with the owner as the responsible human, a
dated review, recorded email permission, and immutable offer, source, and
unverified referral provenance. It grants no execution, outbound automation,
model disclosure, qualification, invoice, or payment authority. The website
holds a create-only token and cannot read or modify private leads.

The pipeline owner first accepts O1's commercial terms, public contact,
consent, and operating responsibility. In a private mode-`0600` policy JSON,
set `schema` to `openagents.sales.intake-policy.v1`, a fresh `id`, `offer` to
`openagents.sales.coder-pilot.v1`, the exact HTTPS `origin`, `public_owner`,
`support_email`, private `commercial_approval` and
`responsibility_acceptance` references, and a fresh `consent_version`.
Set `expires_at` to a Unix timestamp within 90 days,
`retention_seconds` to one through 30 whole days,
`review_within_seconds` to a positive duration within seven days and retention,
and `max_leads` to a lifetime cap from 1 through 32. Loopback HTTP origins
are allowed for isolated fixtures. The owner authenticates locally:

```sh
cargo run -p openagents-web --bin sales-intake -- grant HOST_TASK_ROOT OWNER_CREDENTIAL PRIVATE_POLICY_JSON INTAKE_CREDENTIAL
```

The new credential must be outside the pipeline's `sales/` directory and
distinct from human credentials. Create a separate mode-`0600` server JSON
containing `{"root":"HOST_TASK_ROOT","credential":"INTAKE_CREDENTIAL"}`;
both paths are explicit. Run `openagents-web --pilot-config PRIVATE_JSON`.
Revoke intake with `cargo run -p openagents-web --bin sales-intake -- revoke HOST_TASK_ROOT OWNER_CREDENTIAL POLICY_ID`.
Revocation and expired responsibility refuse new submissions immediately.
Provisioning and tests do not publish a campaign or approve a buyer's agreement.

The form requires explicit request-only email consent and contains no source
upload. Its signed, cookie-bound request lasts 30 minutes. Same-origin checks,
8 KiB bodies, field bounds, a honeypot, per-visitor and global minute limits,
four concurrent private-store operations, and the durable admission cap bound
abuse. Exact retries recover the original acknowledgment after a lost response
or server restart; a new form for the same contact and offer preserves the first
lead without replacing its consent, source, or referral. Deletion does not reset
the cap. An existing manual contact requires human reconciliation; public intake
cannot overwrite its scope or create a competing lead. Existing pipeline
suppression and retention remove lead content.
Public responses show only an opaque request reference and carry `no-store`
and `strict-origin` (no path or query in the referrer); failures never echo
contact fields or private errors. The server emits no contact log or public event. Configure deployment access logs
to omit bodies, cookies, and request queries before activating real intake.

## Tests

```sh
cargo test -p openagents-web
```

The tests check that every public page answers `200` on a development
server with the header, the footer's links to the terms and the policy, and
no script except the homepage terminal's, `/live`'s map, and
`/everglade`'s loader (`/stats` is drawn on the server); `/everglade`'s
policy, its build files' and pack's content types and caches, and that no
other file or path outside its directory is served; the homepage's single download
link and its composer; The Grid guide's screenshot; `/ask`'s retirement,
homepage link, and absence of visitor-cookie creation;
the download page and the `/install`, `/desktop`, and `/docs/install` redirects; that the legal pages carry the
published text; that application styles share the native Coder Noir tokens and
that primary and secondary text meet WCAG AA; the `/connect` page's policy and the association
files; that the removed sections, the release proxy, the Terminal install
commands, the old `/doc` pages, and the blog answer `404` and are never
linked; that the docs list every guide and every site link in a guide
resolves; that the plugin guides name a plugin's five parts and no guide
says *tool*; that no page but the published legal text names Coder
Terminal or its install command; a connected test backend's pages and escaping; and the task
browser.
