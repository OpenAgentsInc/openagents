# OpenAgents web

This Rust server is the OpenAgents website: the public pages that
openagents.com serves, the terms of service and the privacy policy, the one
download page, and the landing page for the pairing QR code. It also serves a
local, read-only task browser at `/app`.

The site and the Cloud app use the Coder Light / Coder Noir design language
from `openagents-ui` (see "Styles"). The `/components` catalog and the
full-screen canvas pages keep Coder Noir from `coder_ui::coder_noir`:
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

Signed-in pages are few on purpose ([the Cloud reset](../../docs/web/cloud-reset.md)):
`/sign-in`, `/settings`, and `/settings/claude`, opened from the account menu
at the bottom of the left panel. The old `/cloud/app` pages are gone and their
addresses redirect. The local task browser keeps its loopback-only scope.

Its first deliverable is the public
[`/components` catalog](../../docs/coder/rust-native/coder-components.md): web
versions of every `coder-new` presentation component and state, composed from
the shared Rust Native Coder library. The catalog composes `coder-ui` views through the reusable `rust-native-web`
adapter. Synthetic interactive fixtures and full screen previews run their
local state controller in Rust/Wasm. These examples connect to no live host,
provider, or account.

The direct-link `/demo` page shows scripted, synthetic example chats in the
shared `openagents-ui` shell: the left panel lists them (the current one
highlighted) and the main area shows the selected thread with the composer
docked. The default chat is the repository environment onboarding flow from
`docs/cloud/example-cursor-cloud-agent-onboarding/`: discovery as a folded
tool group, the install recipe as a code block, the failed install and its
repair as tool rows, build and fresh-machine verification as progress steps,
and the version to save as a result card. Other chats show a Coder fix with
changed files and tests, and a running Cloud benchmark job. Chat links work
without JavaScript; HTMX swaps only the thread and pushes its URL. A message
sent from the composer gets an honest scripted reply and is not stored. The
demo starts no provider work and connects to no account.

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

Running a chat on a connected computer left with the old Cloud pages: the
Environment selector offers nothing, and a chat that already names a runtime
answers `410`. `/environments` replaces it.

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
| `--cloud-build DIRECTORY` | none | Accepted and unused since the old Cloud pages left; the site images still pass it. |
| `--cloud-config PRIVATE_JSON` | none | Explicit account-service origin, public origin, and protected CSRF key. Turns on `/sign-in` and `/settings`; native user sessions and current workspace membership scope each request. |
| `--cloud-hosts PRIVATE_JSON` | none | Protected account/workspace/epoch bindings to host-signed grants, device keys, and exact host routes and generations, kept for `/environments`. No host enrollment or task effect comes from sign-in. |
| `--cloud-byo PRIVATE_DIR` | none | An owner-only directory for customers' own Claude credentials (Anthropic API key, Bedrock, Vertex, or Foundry) behind **Settings → Claude → Manage** (`/settings/claude`). |
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

- `static/legacy-demo.css`: the Coder Noir base the full-screen canvas pages
  (`/everglade`, `/druid`, `/grid`, the Verse world) keep on purpose. Served
  with the
  `--noir-*` variables from `src/palette.rs`.
- `static/components.css`: the `/components` Rust
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
| `/demo`, `/demo/{chat}` | Scripted, synthetic example chats (environment onboarding, a Coder fix, a Cloud job) in the `openagents-ui` shell | Renders; needs no build artifacts. |
| `/sign-in`, `POST /sign-out` | `src/cloud/mod.rs`: sign in with an OpenAgents account key; sign out from the account menu | Requires `--cloud-config`. |
| `/settings`, `/settings/claude` | `src/settings.rs`: profile, theme, and your own Claude credential | Require a signed-in session; the credential page also needs `--cloud-byo`. |
| `/cloud`, `/cloud/sign-in`, `/cloud/app/...` | Old addresses: `303` to `/`, `/sign-in`, `/settings`, or `/settings/claude` | Redirects. |
| `/app`, `/app/tasks/{id}` | The local task store | Reads the store; local hosts only. |

The header links Download and Docs; the footer links the terms and the
privacy policy. Components and Demo remain direct-link pages.

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
