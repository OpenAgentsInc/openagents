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

`GET /chat/{uuid}/delete` asks "Delete this chat? This can't be undone." and
its form posts back with the chat's CSRF ticket. The POST removes the record
at the generation it read (a chat that changed since is not removed), waits
while an answer is still being written, and on a versioned bucket also
removes every older version of the record. Nothing writes the chat again
after that: a late answer's compare-and-swap fails.

`--chat-retention-days DAYS` (or `OPENAGENTS_WEB_CHAT_RETENTION_DAYS`) turns
on removal of chats untouched for that many days, at start and every six
hours. It is off by default. See
[web chat retention](../../docs/deployment/web-chat-retention.md) for the
bucket lifecycle rule and soft delete.

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
| `--project-upload-config FILE` | default policy | Project file limits, account storage quota, and resumable upload lifetime; also `OPENAGENTS_PROJECT_UPLOAD_CONFIG`. See [large project files](../../docs/api/projects.md#large-project-files-and-resume). |
| `--chat-store DIRECTORY` | A sibling `web-chats` directory next to `--store` | Private, restart-durable public conversation records on one machine. |
| `--chat-bucket BUCKET` | none | Private shared conversation records and visitor leases in Google Cloud Storage; replaces local chat storage. |
| `--chat-retention-days DAYS` | off | Remove chats untouched for DAYS days (1 to 3650), from either store, at start and every six hours. Also `OPENAGENTS_WEB_CHAT_RETENTION_DAYS`. |
| `--chat-build DIRECTORY` | none | Generated `coder_chat_web.js` and `coder_chat_web_bg.wasm` for composer and scroll interaction. |
| `--components-build DIRECTORY` | none | The generated `coder_components_web.js` and `coder_components_web_bg.wasm` files for local catalog interaction. Only these names are served. |
| `--cloud-build DIRECTORY` | none | Accepted and unused since the old Cloud pages left; the site images still pass it. |
| `--cloud-config PRIVATE_JSON` | none | Explicit account-service origin, public origin, and protected CSRF key. Turns on `/sign-in` and `/settings`; native user sessions and current workspace membership scope each request. |
| `--cloud-hosts PRIVATE_JSON` | none | Protected account/workspace/epoch bindings to host-signed grants, device keys, and exact host routes and generations, kept for `/environments`. No host enrollment or task effect comes from sign-in. |
| `--cloud-byo PRIVATE_DIR` | none | An owner-only directory for customers' own Claude credentials (Anthropic API key, Bedrock, Vertex, or Foundry) behind **Settings → Claude → Manage** (`/settings/claude`). Saved credentials are encrypted, so it needs a keyring (below). |
| `--cloud-byo-keys PRIVATE_JSON` | none | The keyring that encrypts saved Claude credentials (#11041), an owner-only (`chmod 600`) file kept outside the `--cloud-byo` directory. Or put the same document in `OPENAGENTS_WEB_CLOUD_BYO_KEYS` (a Secret Manager secret on Cloud Run). `--cloud-byo` refuses to start without one. See [Bring your own Claude](../../docs/cloud/claude-code-byo.md#how-saved-credentials-are-kept). |
| `--everglade DIRECTORY` | none | The Everglade web build (`scripts/build-everglade-web.sh`'s output, `everglade_web.js` and `everglade_web_bg.wasm`) with the pinned pack under `pack/`, served at `/everglade`. Without it, `/everglade` says Everglade is unavailable. |
| `--layertree DIRECTORY` | `/srv/layertree` when it exists | The LayerTree demo's web build (`scripts/build-layertree-web.sh`'s output: `layertree_web.js`, `layertree_web_bg.wasm`, `start.js`, and gzip copies), served at `/layertree`. Without it, `/layertree` says the demo is unavailable. |
| `--lander5 DIRECTORY` | `/srv/lander5` when it exists | `/lander5`'s WebGL2 runner (`scripts/build-lander5-web.sh`'s output: `lander5_web.js` and `lander5_web_bg.wasm`), inlined into the page. Without it, `/lander5` shows its title and link over a still background. |
| `--lander6 DIRECTORY` | `/srv/lander6` when it exists | `/lander6`'s agent demo card (`apps/openagents/scripts/build-web-card.sh`'s output: `webgpu/` and `webgl/`, each with `openagents_web_card.js`, `openagents_web_card_bg.wasm`, and brotli and gzip copies), served at `/lander6/card/`. Without it, `/lander6` is `/lander5` with no card. |
| `--lander7 DIRECTORY` | `/srv/lander7` when it exists | `/lander7`'s AgentGraph demo (`apps/openagents/scripts/build-web-graph-card.sh`'s output: `webgpu/` and `webgl/`, each with `openagents_web_graph_card.js`, `openagents_web_graph_card_bg.wasm`, and brotli and gzip copies), served at `/lander7/graph/`. Without it, `/lander7` is the Grid and the name. |
| `--pylons DIRECTORY` | `/srv/pylons` when it exists | `/pylons`' builds: `layer/` (`scripts/build-pylons-web.sh`'s output: `pylons_web.js`, `pylons_web_bg.wasm`) and `card/` (`apps/openagents/scripts/build-web-pylon-card.sh`'s output: `webgpu/` and `webgl/`), each with brotli and gzip copies. Without it, `/pylons` is the Grid and the list. |
| `--pylons-relay URL` | `wss://relay.openagents.com` | The relay `/pylons` reads NIP-PYLON beacons and receipts from, through one subscription per server. `off` connects to nothing, and `/pylons/live` answers 404. `OPENAGENTS_PYLON_CHECKERS` and `OPENAGENTS_PYLON_PAYMENT_ISSUERS` (keys, comma-separated) name whose check verdicts and payment reports count; by default none do, and the card shows neither. |
| `--lander8 DIRECTORY` | `/srv/lander8` when it exists | `/lander8`'s chat (`apps/openagents/scripts/build-web-lander-chat.sh`'s output: `webgpu/` and `webgl/`, each with `openagents_web_lander_chat.js`, `openagents_web_lander_chat_bg.wasm`, and brotli and gzip copies), served at `/lander8/build/`. Without it, `/lander8` is the Grid, the name, and a link to the home page's chat. |
| `--lander9 DIRECTORY` | `/srv/lander9` when it exists | `/lander9`'s hand-tracking demo: `graph/` (`apps/openagents/scripts/build-web-graph-card.sh OUTDIR --hands`) served at `/lander9/graph/`, and `hands/` (`scripts/fetch-hand-landmarker.sh`, the pinned MediaPipe hand landmarker) served at `/lander9/hands/`. Without it, `/lander9` is the Grid and the name. |
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
| `/layertree` | `src/pages/layertree.rs` (#11987): a canvas that fills the window and the build's one start script, which draws the LayerTree demo (`crates/layertree-web`): a seeded, simulated run with thousands of members, or the real signet run with `?run=signet`. `/layertree/{file}` serves the `.js` and `.wasm` files in the build directory (five minutes' cache, gzip when asked). Policy: same-origin scripts and requests and `'wasm-unsafe-eval'` | Says the LayerTree demo is unavailable unless the build directory is there. |
| `/lander5` | `src/pages/lander5.rs`: a landing page over the Grid at twilight, a full-screen procedural shader (`crates/lander5-web`, built from [Shader Skills](https://github.com/gokayfem/shader-skills), MIT). One response: the `<h1>` and the Download link as HTML over the canvas, both shaders, and the runner's glue and module (base64) inline, assembled once per build and sent brotli or gzip. Policy: `default-src 'none'`, the inline style and script by SHA-256, `'wasm-unsafe-eval'`, and `img-src data:` for the inline icon. |
| `/lander6` | `src/pages/lander6.rs`: `/lander5`'s one response (same background, title, and link, assembled the same way) plus an agent demo card: the desktop chat views drawn by GPUI (`apps/openagents/web/card`) playing the desktop demo's scripted reply in a loop. An inline loader waits for the background's first full frame, hands the Grid to a dedicated worker (the `--lander5` directory's `worker/` build, on a canvas transferred with `transferControlToOffscreen`) so GPUI's main-thread work can't hold up its frames, then waits for an idle moment, compiles the WebGPU-only build (or the WebGL2 one without `navigator.gpu`) at low priority with streaming compilation, instantiates and starts it in separate idle slices, and fades the card in after its first frame. `/lander6/card/{webgpu,webgl}/{file}` serves the builds (brotli or gzip when asked). Policy: as `/lander5`'s, plus the loader's hash, `'self'` scripts, `connect-src 'self'`, and `worker-src blob:`. |
| `/lander7` | `src/pages/lander7.rs`: `/lander5`'s one response with the OpenAgents mark and name in the top-left corner and no Download link, plus an AgentGraph demo (`apps/openagents/web/graph-card`): one GPUI module with a demo chat at the center of a graph of GitHub, Gmail, Google Drive, Google Calendar, web search, a relay, a Pylon, and a Spark wallet. Data and demo sats move along the links as the scripted chat plays; a part's details show on hover or tap; the composer's text field at the bottom takes typing and sends nothing. The loader hands the Grid to a worker as `/lander6`'s does, compiles the build at low priority as it streams, and instantiates and mounts it in separate idle slices. `/lander7/graph/{webgpu,webgl}/{file}` serves the builds. Policy: as `/lander6`'s. |
| `/pylons` | `src/pages/pylons.rs`: every pylon online now, on `/lander5`'s Grid drawn in black and white with the camera held still. One response carries the Grid and the list of online pylons as HTML (name, status, class, busy slots), made from the server's live field (`src/pages/pylons_field.rs`: `pylon::field` over the relay; every beacon and receipt verified by `nostr::pylon`, a pylon shown only while its newest beacon is fresh and online or draining). After the Grid's first frame the loader starts the Grid layer (`crates/pylons-web`), which reads `/pylons/live` (server-sent `pylons` events, a snapshot at once and on each change; public data, no sign-in), keeps the list current, announces arrivals and departures, and stands a glowing pillar per pylon on the floor: it rises when the beacon arrives, fades when the pylon goes offline or its beacon expires, and pulses when a receipt for one of its jobs arrives. A click or tap opens the pylon's GPUI card (`apps/openagents/web/pylon-card`), loaded on first open after the Grid moves to a worker as on `/lander6`; the card shows only the rows its events give (`crates/pylons-view`), and the same details are HTML in the dialog for screen readers and when GPUI can't start. `/pylons/layer/{file}` and `/pylons/card/{webgpu,webgl}/{file}` serve the builds. Policy: as `/lander6`'s. | Lists no pylons and says so without a relay. |
| `/lander8`, `/lander8/chat/{id}` | `src/pages/lander8.rs`: `/lander7`'s one response (the Grid and the name in the corner) with a real chat instead of the demo (`apps/openagents/web/lander-chat`): the composer at the bottom, a welcome line before the first message, and the conversation drawn by the shared GPUI chat views in a card that grows as it goes, with Stop while an answer streams. It uses the chat API the apps use (`/v1/threads`, its events, and stop) through `crate::cookie_api`: a signed-in person's chats are their account's, and a signed-out browser's `oa_visitor` cookie goes on as its visitor token, so it chats with the apps' signed-out limits and is sent to sign in, with its message kept, when they run out. A chat's address is `/lander8/chat/{id}`, so a reload shows it. `/lander8/session` gives the chat its CSRF value (and a visitor cookie when the browser has none); `/lander8/build/{webgpu,webgl}/{file}` serves the builds. Loaded as `/lander7`'s demo is. Policy: as `/lander6`'s. |
| `/lander10`, `/lander10/chat/{id}` | `src/pages/lander10.rs`: `/lander8`'s chat and `/pylons`' live pylons on one page, over the still black-and-white Grid facing straight ahead, with the mark and name in the corner and only the composer before the first message. The chat (`/lander8/build/`) says where its card and composer are (`#lander-chat-keep`, marked `data-pylons-keep`), and the pillars (`/pylons/layer/`, fed by `/pylons/live`) stand clear of them, moving as the card grows. A click on a pillar opens its GPUI card (`/pylons/card/`, loaded on the first open) above the composer; Escape or a click outside closes it. The list of online pylons is in the response, visually hidden, for screen readers. Policy: as `/lander6`'s. |
| `/lander11`, `/lander11/chat/{id}` | `src/pages/lander11.rs`: `/lander8`'s chat (a glass card, only the composer before the first message) over Verse's own Grid. One response with the shader Grid for the first paint; the loader hands the shader to a worker, starts the chat (`/lander8/build/`) in idle slices, then fetches the Verse web build `/verse` serves (`/everglade/everglade_web_bg.wasm`) at low priority and starts it. On this path `everglade-web` opens the Grid with its camera circling the plaza, joins no one, and leaves the keyboard and mouse to the chat; its canvas cross-fades in behind the chat and the shader stops. A Walk button hands Verse the keyboard and mouse (`verse_walk`); Escape gives them back. Policy: as `/lander6`'s. |
| `/home` | `src/pages/grid_home.rs`: the Grid homepage; `/` stays unchanged. One response: `/lander5`'s Grid held still in black and white, a top bar with the mark, the name, and Log in and Sign up (returning to `/home`), and the online pylons' list as HTML (visible without script, for screen readers with it). Then, each in its own module and in idle moments: the shader moves to a worker, `/pylons`' layer stands a pillar per pylon (`/pylons/live`; a click opens the GPUI pylon card), and the Verse build `/verse` serves fades in behind with its camera circling (`data-ambient`). No text input. `/home/me` tells the top bar who is signed in (their name, profile, and Open chat, through `/new` so the feature flags and saved chat UI choice apply). The classic home page moved to `/home/classic`. Policy: as `/lander6`'s. |
| `/lander9` | `src/pages/lander9.rs`: `/lander7` with a hand button in the top-right corner. A click asks for the camera and starts MediaPipe's hand landmarker in the browser (pinned, served from this site, loaded only then; see `apps/openagents/web/hands/README.md` for the exception); the points go to the demo's wasm module, where the One Euro smoothing, gestures, and interactions run in Rust. Point at a part to read it, pinch to select it or to type, pinch and hold or make a fist to move a part or the chat, drop a tool on the chat to start a demo turn, pinch with both hands to spread the parts, hold an open palm to let go. The mouse does the same. Frames never leave the page. Policy: as `/lander7`'s. |
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

The landing queue runs these tests inside its write boundary (`bwrap` on
Linux), where `/` and `/tmp` belong to the overflow user instead of root.
The private-file checks in `cloud/` therefore take the directories above a
test's temporary directory on trust in test builds only; to run the tests
the way the queue does:

```sh
S=$(mktemp -d); T=${CARGO_TARGET_DIR:-$PWD/target}
bwrap --die-with-parent --ro-bind / / --dev /dev --proc /proc \
  --bind "$PWD" "$PWD" --bind "$T" "$T" --bind "$S" "$S" \
  --setenv TMPDIR "$S" --setenv CARGO_TARGET_DIR "$T" -- \
  cargo test -p openagents-web
```

## Blue Rush sand garden

Build the editor with `scripts/build-sand-garden.sh OUTDIR`, then start the
site with `--sand-build OUTDIR`. The production image includes it at
`/srv/sand`. Shared drawings and votes use the site's chat storage under
`sand-garden/gallery.json`. Creators can delete from their original browser;
site admins can sign in on openagents.com and delete any gallery entry.

The research-based V2 landing page is at `/studios/blue-rush-v2`, with a
parent interview at `/studios/blue-rush-v2/interview`. On the studio domain,
these paths are `/v2` and `/v2/interview`. V1 stays at its existing address.
The interview returns a text download and does not save responses.
