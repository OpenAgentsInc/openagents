# OpenAgents web

This Rust server is the OpenAgents website: the public pages that
openagents.com serves, the terms of service and the privacy policy, the one
download page, and the landing page for the pairing QR code. It also serves a
local, read-only task browser at `/app`.

The site is drawn in four intensities of white on
near-black (`src/palette.rs`). The interactive pages run scripts under a
policy that allows its one same-site script and same-origin requests: the
homepage's terminal (`static/ask.js`, requests to `/ask`), `/live`'s
map (`static/flow.js`, requests to `/api/flow/*`, drawn on a canvas in the
desktop map's colors), and `/everglade`'s loader (`static/everglade.js`),
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
| `--components-build DIRECTORY` | none | The generated `coder_components_web.js` and `coder_components_web_bg.wasm` files for local catalog interaction. Only these names are served. |
| `--cloud-build DIRECTORY` | none | Rust/Wasm private-view lifecycle assets built by `scripts/build-coder-cloud-web.sh`. Private content waits for current account and resource standing before display. |
| `--cloud-config PRIVATE_JSON` | none | Explicit account-service origin, public origin, and protected CSRF key. Native user sessions and current workspace membership scope each request. |
| `--cloud-hosts PRIVATE_JSON` | none | Protected account/workspace/epoch bindings to host-signed Observe grants, device keys, and exact host routes and generations. No host enrollment or task effect comes from sign-in. |
| `--everglade DIRECTORY` | none | The Everglade web build (`scripts/build-everglade-web.sh`'s output, `everglade_web.js` and `everglade_web_bg.wasm`) with the pinned pack under `pack/`, served at `/everglade`. Without it, `/everglade` says Everglade is unavailable. |
| `--pilot-config PRIVATE_JSON` | none | Explicit task root and create-only intake credential for `/pilot`. The pipeline owner provisions the capability separately. Without accepted terms, the proposed offer renders with intake unavailable. |

A development server needs no secrets and makes no network requests:
everything it serves is compiled in or read from this repository.

## Pages

| Route | Source | Development server |
| --- | --- | --- |
| `/` | What OpenAgents is, one `[ Download OpenAgents ]` link, and the **Ask OpenAgents** terminal | Renders. |
| `/download` | `src/pages/download.rs`: the notarized OpenAgents for Mac `.dmg` in `openagentsgemini-oa-updates`, OpenAgents Terminal's install commands, and one link to build everything else from source | Renders. |
| `/pilot`, `/pilot/install` | `src/pilot.rs`: [Coder pilot v1](../../docs/sales/README.md#first-workflow-offer-v1), proposed USD 250 service terms, the selected source-built macOS arm64 installation path, and bounded private intake. General downloads do not qualify the pilot. No comparative or customer-result claims. | Renders proposed terms; intake is unavailable without owner configuration. |
| `/install`, `/desktop` | Permanent redirect (`308`) to `/download`, so older links keep working | Redirects. |
| `POST /ask` | The homepage terminal's questions (`src/ask.rs`, #10106): a NIP-CJ job to the OpenAgents chat worker through `relay.openagents.com`, surface `web`, signed with a key derived from the visitor's `oa_visitor` cookie and the server's secret (`OPENAGENTS_WEB_ASK_SALT`, random per process when unset). The worker answers about OpenAgents only and never offers Coder, a computer, a command, or a screen. One question at a time and 6 a minute per visitor, 32 waiting at once for everyone, besides the worker's quotas. Streams newline-delimited JSON | Answers from the live chat worker. |
| `/docs`, `/docs/{slug}` | `content/docs/*.md`, short guides in reading order, compiled in: what OpenAgents is, download (`/docs/install` redirects to `/docs/download`), connecting a computer, chat, Coder, plugins (what they are, writing, testing, publishing and sharing), the Verse, the Grid (with its screenshot, `static/verse-grid.jpg`, captured from the live relay with `crates/verse/examples/overlook_capture.rs`), privacy and security, and help | Renders. |
| `/live` | `src/pages/live.rs` and `static/flow.js` (#10197): the route map drawn from the pay host's flow snapshot, each streamed event animated as the desktop deck's `routes-live` scene does (white request out, gold payment back, gold share to the author, gold payout to the wallet, a ring for a bonus), a totals ticker, the last event's time, and the recent events. Reads `/api/flow/snapshot` and `/api/flow/stream` on this origin (#10195); never draws synthetic traffic. The mapping's tests are `static/flow.test.js` (`node --test`) | Says the flow stream is unreachable until `/api/flow/*` answers. |
| `/everglade` | `src/pages/everglade.rs` and `static/everglade.js` (#10525): a canvas that fills the window, with no site header or footer and no page zoom, and a loader that imports the Everglade web build's glue (#10524) and calls its `init()`. `/everglade/{file}` serves the `.js` and `.wasm` files in the `--everglade` directory (five minutes' cache) and `/everglade/pack/{sha256}.vtp` the digest-named pack in its `pack/` (a year's immutable cache); nothing else on disk. Policy: same-origin scripts and requests and `'wasm-unsafe-eval'`. The Verse guide links it. The glue's file name is `GLUE` in `src/pages/everglade.rs` and must match the build script's output | Says Everglade is unavailable unless started with `--everglade DIR`. |
| `/stats` | `src/pages/stats.rs` (#10196): drawn on the server from the pay host's public `/stats` and `/flow/snapshot` (#10195): received, paid out, pending, calls, and author earnings; plugins (calls, earned, paid out); authors (earned, paid out, pending); the 20 most recent author payouts; 24 hour and 30 day bars of sats received (inline SVG); the reconciliation state and the last event's time. Linked from `/live` and linking back. No script; public fields only, never a payer | Says the statistics are unreachable without a pay host, and "No payments yet" with an empty ledger. |
| `/terms`, `/privacy` | `content/legal/*.md`, the published text (2026-09-03), compiled in | Renders. |
| `/connect` | Landing page for `https://openagents.com/connect#<code>` | Renders; no script, no referrer. |
| `/.well-known/apple-app-site-association`, `/.well-known/assetlinks.json` | Universal link and App Link claims for `/connect` | Serves. |
| `/u/{login}` | `Backend::profile` | Says the backend isn't connected. |
| `/components`, `/components/{component}` | Shared Coder components, named synthetic variants, typed controls, source references, and full screen previews | Renders; Rust/Wasm interaction requires `--components-build`. |
| `/cloud`, `/cloud/sign-in`, `/cloud/app` | Public availability and the native account/workspace shell | Public entry renders; private pages require explicit native account configuration and the Cloud Wasm build. |
| `/cloud/app/hosts/{binding}/tasks`, `/cloud/app/hosts/{binding}/tasks/{task}` | Bounded, signed resident task reads under current Observe authority; original ATIF messages, tools, child references, checks, cost, and source pins | Requires a separately provisioned host binding. It reads no local `/app` records. |
| `/app`, `/app/tasks/{id}` | The local task store | Reads the store; local hosts only. |

The header links Download, Cloud, Verse, Components, Docs, and Pilot; the footer links the terms and the
privacy policy.

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
link and its terminal; The Grid guide's screenshot; `/ask`'s stream, cookie,
bounds, and one-at-a-time rule, against an in-process door;
the download page and the `/install`, `/desktop`, and `/docs/install` redirects; that the legal pages carry the
published text; that every color in the stylesheet is a gray and that the
text steps meet WCAG AA; the `/connect` page's policy and the association
files; that the removed sections, the release proxy, the Terminal install
commands, the old `/doc` pages, and the blog answer `404` and are never
linked; that the docs list every guide and every site link in a guide
resolves; that the plugin guides name a plugin's five parts and no guide
says *tool*; that no page but the published legal text names Coder
Terminal or its install command; a connected test backend's pages and escaping; and the task
browser.
