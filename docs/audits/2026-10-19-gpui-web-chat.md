# Replace the web app's chat with GPUI on the web

Recorded 2026-10-19, at `7a7ca1df5a`. Owner request: after the `/demo/gpui`
demo, replace every chat surface on openagents.com with the desktop app's
GPUI chat UI compiled to WebAssembly.

This document lists the chat surfaces as they are today, proposes one
architecture, sets a size and first-paint budget from measurements of the
demo, says what a canvas loses and how to cover it, and splits the work into
phases behind a flag. Each phase has a GitHub issue.

## Summary

- Build **one** GPUI web app from the desktop's own crates, not from the
  demo's copies. That means splitting `apps/openagents/crates/ui` and
  `client` into portable and native parts first.
- Get data from the APIs the web app already serves (`/v1/threads`, its SSE
  stream, `/chats/events`) and, for Coder chats, from `coder-sync` Loro rooms.
  Add no second chat backend.
- The demo module is 2.64 MB with brotli. The budget for the real app is
  1.5 MB brotli before the first chat draws, with fonts loaded separately.
- Keep server HTML as the first paint, the fallback, and the only renderer
  for public share pages. The canvas takes over once it is ready.
- Roll out behind a per-account flag, one surface at a time. The HTMX chat
  path is removed only after the last phase passes.

## Chat surfaces today

Every chat surface is server-rendered HTML (maud, `crates/openagents-ui`)
driven by HTMX 2. Live updates come from SSE streams in the web app that
re-read the chat store. Paths are under `crates/openagents-web/src/`.

| Surface | Routes | Source | How it works | Auth |
| --- | --- | --- | --- | --- |
| New chat | `/`, `POST /chat`, `/composer/*` | `pages/home.rs`, `pages/chat.rs` (`send_new`, `start`), `composer_row.rs`, `composer.rs` | Composer plus project, branch, and runtime row. A post creates the chat and moves to `/chat/{id}`. | Account (`oa_cloud_session`) or visitor (`oa_visitor`) cookie; CSRF HMAC plus `Sec-Fetch-Site` and `Origin` checks |
| Chat page | `/chat/{id}`, `POST /chat/{id}`, `/chat/{id}/transcript` | `pages/chat.rs` (`show_page`, `chat_page`, `follow`) | Server transcript; windowed history fragments | Same |
| Chat stream | `/chat/{id}/events` | `pages/chat.rs` (`events`) | SSE. Polls the store each second; each new revision sends the whole re-rendered transcript as HTML | Same |
| Sidebar status | `/chats/events`, sidebar actions | `pages/chat_live.rs`, `pages/chat_sidebar.rs` | One SSE stream per tab swaps row status out of band | Same |
| Composer helpers | `/chat/assets/*` | `crates/coder-chat-web` (wasm) | Drafts, Enter and queue keys, scroll, moves between `/` and `/chat/{id}`. HTMX owns requests | None |
| Chat side surfaces | `/chat/{id}/files`, export, agents, approval, computer asks, Claude Code run, continue, GitHub tools, Lab, Game, CAD, and Robot cards | `chat_files.rs`, `chat_export.rs`, `pages/chat_agents.rs`, `chat_approval.rs`, `chat_computer.rs`, `chat_work.rs`, `chat_continued.rs`, `github_tools/`, `lab.rs`, `game_lab.rs`, `cad_lab.rs`, `robot_lab.rs`, `lab_card_view.rs` | HTML fragments and HTMX; approvals re-read every few seconds; cards load their own JS | Owner or project member |
| Coder chats | `/chat/{id}` with a terminal record | `pages/chat.rs` (`show_terminal`, `reply_to_coder`), `coder_sync.rs` | Read-only unless Coder checked in recently; web replies wait for Coder to pull them | Account; Coder pushes with a Bearer token |
| Computer sessions | `/chat/sessions/{id}` | `pages/chat_sessions.rs`, `session_index.rs` | Summary, and the transcript when the owner opted in | Account |
| Shared transcripts | `/share/{token}`, `/chat/{id}/share` | `pages/chat_share.rs` | Public, read-only, no stream; OG card; `noindex` | Token |
| Scripted demo | `/demo`, `/demo/{chat}` | `demo/mod.rs` | Synthetic chats in the HTML shell | Public |
| GPUI demo | `/demo/gpui` | `pages/gpui_demo.rs` | The desktop UI in wasm with offline demo chats | Public |
| Environments | `/environments/*` | `environments/` | Setup agent's log over SSE, with message, stop, and choose | Trusted accounts |
| Work runs, Mac jobs, tasks | `/work/{id}`, `/settings/mac-jobs/{id}`, `/app/tasks/{id}` | `work_runs.rs`, `mac_jobs_page.rs`, `tasks.rs` | Progress streams over SSE or read-only views | Account |
| Playground chat | `/playground/chat` | `crates/gateway/src/playground.rs` | Plain form round trip; internal | Gateway session |

The JSON chat API already exists for the apps: `/v1/threads` (list, create,
read, move, delete), `/v1/threads/{id}/messages`, files, `stop`, `continue`,
`share`, and `GET /v1/threads/{id}/events`, an SSE stream of message deltas,
cards, `done`, and `error` (`phone_api.rs`, `chat_api.rs`). It takes a Bearer
token or a visitor token, not the site's cookie.

`coder-sync` (`sync.openagents.com`) serves Loro chat rooms at
`/chat2/{id}/ws`. The browser can't reach it today: the chat CSP allows
`connect-src 'self'` only, and the room token is a Bearer token.

Out of scope: the playground (internal, not a product chat), `/att` (a
sealed-inference demo), and the game pages' Nostr presence.

## Architecture

### One app, built from the desktop crates

The demo (`apps/openagents/web/demo`) reuses the desktop's `doc`, `proto`,
`markdown`, and `brand` crates and its demo fixtures. Its sidebar,
transcript, and composer are ports, because `openagents-ui` links tokio,
reqwest, alacritty, wry, AppKit, and the engine. Shipping those ports would
leave two chat UIs to keep in step. Instead:

1. **Split `ui`.** Move the transcript, composer, sidebar, theme, and the
   Rust Native GPUI mount (`ui/src/native_actions.rs`) into a portable crate,
   `openagents-chat-view`, that depends on `gpui`, `doc`, `proto`, `markdown`,
   and `theme` only. Native pieces (terminal, webviews, voice, files on disk,
   update) stay in `ui` behind its native platform. The desktop keeps
   building from `ui`, which re-exports the moved modules.
2. **Split `client`.** Put its transport behind a trait with two backends:
   `native` (tokio, reqwest, tokio-tungstenite, today's code) and `web`
   (`fetch`, `EventSource`, and `WebSocket` through `web-sys`, on GPUI's
   foreground executor). Room logic in `client/src/live/` stays shared.
   `sync`'s SQLite store and Keychain stay native-only features.
3. **One web app.** `apps/openagents/web/chat` (`openagents-web-chat`)
   mounts `openagents-chat-view` over the `web` client. The demo becomes this
   app with a fixture client, so `/demo/gpui` keeps working and the ported
   components are deleted.

`apps/openagents/web` stays its own Cargo workspace (Rust 1.98.1, its own
lock), so wasm-only dependencies stay out of the desktop lock and its hakari
crate.

### Data and auth

Nothing is reimplemented. The web app is a client of what exists:

- **Web chats.** `/v1/threads` and `/v1/threads/{id}/events`. Add cookie auth
  to these routes for same-origin requests: the `oa_cloud_session` or
  `oa_visitor` cookie, plus the existing CSRF value sent as a header on
  writes. The Bearer path for the apps stays as it is.
- **Sidebar.** `/chats/events` gains a JSON variant (`Accept:
  text/event-stream` with `?format=json`), so the canvas gets row state
  instead of HTML.
- **Coder chats.** Join the chat's `coder-sync` room with the same `client`
  room code the desktop uses. The web app mints a short-lived room token for
  the signed-in account (NIP-HOST grant rules unchanged), and the chat page's
  CSP adds `connect-src https://sync.openagents.com`.
- **Uploads and downloads.** The existing files routes.

### Rust Native cards and sheets

Approval, wallet, Lab, Game, CAD, Robot, and settings cards are Rust Native
views. The GPUI adapter in `ui/src/native_actions.rs` moves into
`openagents-chat-view` as `rust_native_gpui`, so the desktop and the web draw
cards with the same code. Cards that host their own surface (CAD, Robot) keep
their wasm modules and mount over the canvas in a DOM layer the adapter
places. No card is written separately for the web.

## Size and first paint

Measured on the demo build (`build-web-demo.sh`, `opt-level = "s"`, LTO,
`wasm-opt -Os`), 2026-10-19:

| File | Raw | gzip -9 | brotli -q 11 |
| --- | --- | --- | --- |
| `openagents_web_demo_bg.wasm` | 8,889,900 B | 3,797,408 B | 2,635,798 B |
| `openagents_web_demo.js` | 79,263 B | 14,174 B | 12,093 B |
| Fonts compiled into the module | 2,002,420 B | | 763,341 B |

Time until the canvas replaced the loading line (module compiled, app
started), headless Chrome on an Apple silicon Mac with WebGPU, cache off,
served gzip:

| Network | Ready |
| --- | --- |
| Loopback | 253, 255, 405 ms |
| 20 Mbit/s, 40 ms | 2.14 s |
| 9 Mbit/s, 150 ms | 4.68 s |

Brotli, which the image now ships, saves about 30% over gzip.

Budget for the chat app:

- At most 1.5 MB brotli for the module needed to draw a chat.
- Ready within 1.5 s at 20 Mbit/s on a mid-range laptop.
- The server HTML transcript paints first, so first contentful paint doesn't
  wait on the module.

How to meet it:

- Load fonts as separate cached files. Fetch the CJK and emoji subsets only
  when a chat needs them.
- Serve files under content-hashed names with `immutable` caching, so a
  return visit downloads nothing.
- Use `WebAssembly.compileStreaming` and start the download from a
  `<link rel="modulepreload">` in the page head.
- Keep heavy cards (CAD, Robot, Game) in their own modules, loaded when a
  card appears.
- Track size per commit with `twiggy` and fail the build over budget.

## What a canvas loses

A canvas is one opaque element to the browser. Be plain about it:

| Need | Canvas alone | Plan |
| --- | --- | --- |
| Screen readers | Nothing is read | A hidden DOM mirror of the transcript and controls, rendered by `rust-native-web` from the same view tree, with roles, labels, and live regions for new messages |
| Keyboard focus | GPUI's own focus only | The mirror holds real focusable elements; focus moves between the mirror and the canvas in step |
| IME | Works through the hidden text field `gpui-web` already uses | Test Japanese, Chinese, and Korean input and dead keys in every phase |
| Text selection | GPUI selection only, not the browser's | Keep GPUI selection; copy writes plain text and Markdown to the clipboard |
| Copy and paste | Works through clipboard events | Test images and files on paste |
| Find in page | Browser find sees nothing | Map Cmd+F and Ctrl+F to the app's own find, and keep the mirror's text findable |
| Links | No real anchors | The mirror holds real `<a>` elements; the canvas opens links with `window.open` on click |
| Zoom and text size | Ignores browser text size | Read `devicePixelRatio` and the root font size; scale the UI |
| SEO and link previews | No text | Public pages stay server HTML (`/share/{token}`, `/demo`). Chat pages are `noindex` already |
| No WebGPU | | The modules are WebGPU-only: GPUI's renderer reads storage buffers, which WebGL2 lacks, so wgpu can't fall back. Without WebGPU, or with JavaScript off, the server HTML stays (#12237) |
| Phones | Touch is minimal in `gpui-web`; memory is tight on iOS | Phones keep the HTML chat until a phase passes on iOS Safari and Android Chrome |

## Rollout

A per-account setting (`chat_ui = gpui`) and a `?ui=gpui` query switch each
phase. The server serves both renderers until the last phase. Each phase is
done when its acceptance checks pass with the flag on, on Chrome, Safari, and
Firefox, desktop size.

| Phase | Work | Acceptance | Issue |
| --- | --- | --- | --- |
| 0a | Split `ui` into `openagents-chat-view`; the demo uses it and drops its copies | Desktop builds and its tests pass; `/demo/gpui` looks the same; no ported component is left in `web/demo` | [#12216](https://github.com/OpenAgentsInc/openagents/issues/12216) |
| 0b | `client` web transport; cookie auth on `/v1/threads` | A wasm test lists, reads, and streams a chat on staging with the test account | [#12217](https://github.com/OpenAgentsInc/openagents/issues/12217) |
| 0c | Rust Native GPUI adapter in the portable crate | Desktop cards unchanged; the web app draws an approval card from a fixture | [#12218](https://github.com/OpenAgentsInc/openagents/issues/12218) |
| 0d | Size budget: fonts out of the module, hashed names, preload, size check | Module at most 1.5 MB brotli; ready within 1.5 s at 20 Mbit/s | [#12219](https://github.com/OpenAgentsInc/openagents/issues/12219) |
| 0e | Accessibility mirror, find, links, zoom | VoiceOver and NVDA read new messages; Tab reaches every control; find finds transcript text | [#12220](https://github.com/OpenAgentsInc/openagents/issues/12220) |
| 1 | `/chat/{id}`: read, stream, reply, stop | A test-account chat streams a reply; reload resumes; IME and paste work | [#12221](https://github.com/OpenAgentsInc/openagents/issues/12221) |
| 2 | New chat at `/` and the sidebar | Start a chat, see it in the sidebar, rename, pin, archive, and move it; signed-out visitors too | [#12222](https://github.com/OpenAgentsInc/openagents/issues/12222) |
| 3 | Files, approvals, agents panel, computer asks, Lab, Game, CAD, and Robot cards | Each card works from the canvas; heavy cards load on demand | [#12223](https://github.com/OpenAgentsInc/openagents/issues/12223) |
| 4 | Coder chats and computer sessions through `coder-sync` rooms | A Coder chat updates live from the room; a web reply reaches Coder | [#12224](https://github.com/OpenAgentsInc/openagents/issues/12224) |
| 5 | Environments, work runs, and Mac jobs | Each stream draws live; stop and choose work | [#12225](https://github.com/OpenAgentsInc/openagents/issues/12225) |
| 6 | Default on for desktop browsers; phones after their own check; remove the HTMX chat path | A week with the flag on by default and no open regressions; `/share/{token}` and the no-JavaScript fallback stay server HTML | [#12226](https://github.com/OpenAgentsInc/openagents/issues/12226) |

Phases 0a to 0e can run in parallel. Phase 1 needs 0a, 0b, and 0d. Phase 3
needs 0c. Phase 6 needs every other phase.

This plan covers the chat surfaces. The rest of the desktop app (computers,
files, settings, check-ins, Agent Lab, voice, and Earn) is in the
[desktop and web parity plan](2026-10-20-desktop-web-parity.md), as phases 7
to 13 (#12262 to #12294).

The wider site plan, which puts these phases under profiles, agent
identities, and one front door at `/`, is
[One openagents.com](2026-10-20-openagents-com-unified-plan.md) (#12254 to
#12261).

## Risks

- **Two chat UIs during the split.** Moving code out of `ui` (16,000-line
  transcript and composer files) touches desktop work in flight. Move files
  whole and re-export them, without behavior changes.
- **Upstream GPUI.** `gpui-web` is a hand port of zui's platform at one
  revision. Desktop GPUI upgrades now need a matching web port. Record each
  in `apps/openagents/docs/downstream-patches.md`.
- **Size growth.** The real UI is larger than the demo. The size check in
  0d keeps it honest.
- **Accessibility debt.** A mirror that falls out of step with the canvas is
  worse than none. Generate both from one view tree, and test with a screen
  reader in every phase.
- **Single thread.** The build runs everything on the page's thread
  (`multithreaded` needs nightly). Long Markdown or syntax work can drop
  frames; budget it per frame.
- **Mobile Safari.** Memory limits and WebGPU support vary. Phones stay on
  HTML until checked.
- **Auth surface.** Cookie auth on `/v1/threads` and a browser room token
  for `coder-sync` are new paths. Keep the CSRF checks, short token
  lifetimes, and the grant rules.

## The `/gpui` chat (2026-10-19)

A complete signed-in chat now runs at `/gpui` and `/gpui/chat/{id}`, beside
the HTML chat, which is unchanged. It is phases 0b, 1, and 2 delivered on
their own route instead of behind the flag.

- `apps/openagents/web/chat-view` (`openagents-web-chat-view`) holds the
  views the demo and the chat share: theme, markdown, the composer's text
  field, transcript pieces (`assistant_body`, tool groups, user bubbles),
  fonts, and icons. `web/demo` uses it and keeps only its fixtures and
  scripted replies.
- `apps/openagents/web/client` (`openagents-web-client`) is the web data
  client: `/v1/threads`, `/v1/threads/{id}/messages`, `/v1/projects`,
  `PATCH` and `DELETE /v1/threads/{id}`, `stop`, and the
  `/v1/threads/{id}/events` stream over `fetch` and `EventSource`.
- `apps/openagents/web/chat` (`openagents-web-chat`) is the app. The page
  (`crates/openagents-web/src/pages/gpui_chat.rs`) paints an HTML outline
  of the chat list, checks for WebGPU, and fetches the module and
  the Geist faces together.
- `crates/openagents-web/src/cookie_api.rs` takes the sign-in cookie on
  `/v1/threads*` and `/v1/projects` for same-origin requests
  (`Sec-Fetch-Site: same-origin`); writes also carry the chat's CSRF value
  in `x-openagents-csrf`. The cookie's session token then goes on as the
  Bearer token, so each route checks it as it checks an app's.

Still duplicated from the desktop, until #12216 moves the views into a
portable crate:

- `chat-view/src/theme.rs`: the dark theme's tokens as sRGB values, from
  `crates/ui/src/theme`.
- `chat-view/src/markdown.rs`: the GPUI markdown renderer. The block tree
  is the shared `openagents-markdown`; the drawing is a port of
  `crates/ui/src/markdown`.
- `chat-view/src/transcript.rs`: user bubbles, tool groups and rows, error
  and notice lines, and the labels. The group summary is the shared
  `openagents_proto::view::tool_group_summary`.
- `chat-view/src/composer.rs`: a single-line field after GPUI's
  `examples/input.rs`, not the desktop's multi-line composer.
- `web/chat/src/app.rs`: the sidebar, header, row menu, and composer row.
  The desktop's own sidebar and composer are in `crates/ui`.

Not done in `/gpui` yet:

- Cards draw as a named card that opens the regular chat; Rust Native cards
  wait on #12218.
- Approvals, files and uploads, the agents panel, and computer asks
  (#12223); Coder chats are read-only with their computer named, and live
  rooms wait on #12224.
- The accessibility mirror, find in page, and zoom (#12220).
- The module is 1.90 MB with brotli, over the 1.5 MB budget, and the
  files aren't content-hashed yet (#12219).
- Multi-line messages, and phones beyond a basic layout.

### How to swap it in

The chat is complete on its own route, so making it the default is a
routing change in `crates/openagents-web`:

1. Serve the canvas at the regular addresses. In `pages/chat.rs`, have
   `show` (`/chat/{id}`) and the home page's chat (`/`) answer with
   `gpui_chat::page(app, headers, chat)` for signed-in accounts, while the
   HTML stays for signed-out visitors, `/share/{token}`, and browsers
   without JavaScript. The app reads its own address from the page's
   `openagents-chat-base` meta tag (`/gpui` today, in `gpui_chat::shell`);
   give it `""` there and the app opens and pushes `/` and `/chat/{id}`
   with no rebuild of the module.
2. Behind a flag first: check a per-account setting (`chat_ui = gpui`) or
   a `?ui=gpui` query in those handlers, and keep `/gpui` as an address
   that always shows the canvas.
3. To undo, remove the check; nothing else changes, because both views
   read the same chats through the same API.

Done on 2026-10-20 (#12221, #12222). `/`, `/new`, and `/chat/{id}` serve
the canvas, with `""` as its base, when the request says `?ui=gpui` or
carries the `oa_chat_ui=gpui` cookie that `?ui=gpui` sets for a year;
`?ui=html` shows the HTML chat once, and `?ui=off` shows it and clears the
cookie (`gpui_chat::choice`). The HTML chat stays the default and the fallback
(an HTMX swap, a server without the build, a browser without WebGPU,
and JavaScript off all keep it), and the canvas links to it with
`?ui=html`. Signed out, the canvas chats as the browser's visitor, as
`/lander8` does: `GET /v1/threads`, `GET /v1/threads/{id}/messages`,
`PATCH` (title, pin, archive), and `DELETE /v1/threads/{id}` now take a
visitor token too; moving a chat to a project still needs an account.
The chat API's values and rules (`openagents_sdk::chats::wire`) are now
the SDK's own, which builds for the browser without its `transport`
feature; `web/client` is the browser transport over them.

## The first-frame painter (2026-10-19)

`/agent/foreman` now shows AgentGraph's first frame after one round trip,
instead of after the 1.5 MB module. A measured grid benchmark showed that
first paint on a real network is set by bytes and round trips, not by
compiling, so the page carries a tiny painter and one captured frame inline.

### How it works

- **Scene format.** `apps/openagents/web/first-frame`
  (`openagents-first-frame`) holds one frame as GPUI's own primitives, in
  draw order: quads (backgrounds, borders, rounded corners), shadows, path
  triangles, and glyph sprites, with colors as GPUI's shaders compute them
  and only the glyph pixels the frame uses. It has no dependencies. Decoding
  refuses a wrong header, a truncated or padded buffer, a count longer than
  the bytes left, an index past its table, a non-finite number, a tile
  outside the atlas, and a draw command past its primitives.
- **Capture.** `apps/openagents/web/first-frame-capture` runs the AgentGraph
  view natively on GPUI's headless test platform (`HeadlessAppContext`)
  with the web build's `CosmicTextSystem` and fonts, so layout and glyphs
  match the browser. No GPU and no browser: the window's renderer records
  the frame, and its sprite atlas keeps each glyph's pixels. The browser's
  WebGPU renderer draws subpixel text where dual-source blending exists, so
  the atlas also rasterizes each glyph for subpixel rendering. The capture
  refuses any primitive the painter can't draw the same way (gradients,
  dashed borders, images, underlines, blurs, fades). `build-web-demo.sh
  --app agent-graph` runs it, and the website image's `gpuiweb` stage
  builds it.
- **Painter.** `apps/openagents/web/first-frame-paint` is a paint-only
  WebGL2 module. It ports `gpui_wgpu`'s quad SDF (rounded corners and
  borders), shadow, path rasterization, and mono and subpixel sprite math
  to GLSL ES 3.0. Paths go through a 4× multisampled layer, as in GPUI.
  Subpixel glyphs blend one color channel at a time instead of with
  dual-source blending. It's 18.5 KB brotli, and 23.5 KB with its
  wasm-bindgen glue and boot script.
- **Page.** `pages/first_frame.rs` in `openagents-web` inlines the painter
  and one frame (gzipped, base64, in inert script elements) and the glue
  and boot script as one inline script that the page's policy allows by
  hash. A phone's user agent gets the phone frame, anything else the
  desktop frame. The page is brotli-compressed. A build whose graph digest
  isn't the server's graph is left out.
- **Handoff.** The boot script paints only when the graph box measures
  exactly what the frame was captured at (990 × 736 or 356 × 605.67 layout
  pixels, at device pixel ratio 2), over the app's canvas with a progress
  cursor and no input. `gpui-web` announces the app's first frame at the
  canvas's size (a `gpui-first-frame` event and performance mark), and the
  snapshot goes in that same task, so both change in one composited frame.
  AgentGraph moves data only for real agent events (#12229), so without
  recent events its first frame is the captured one whenever it draws. Without script, the server's map and cards stay; if
  the module fails, the page goes back to them.

### Measured

Headless Chrome 155 through `openagents browser run`, cache off, brotli
HTML and module, 5 runs, the box's viewport at DPR 2. Medians; "baseline"
is the same build without the first frame. The graph is visible at the
snapshot's frame, or at the app's first frame without it.

| Profile | Class | Graph visible, snapshot | Graph visible, baseline | App's first frame, snapshot / baseline | HTML (brotli), snapshot / baseline |
| --- | --- | --- | --- | --- | --- |
| Loopback | Desktop | 59 ms | 159 ms | 159 / 159 ms | 101 / 17 KB |
| Loopback | Phone | 59 ms | 153 ms | 130 / 153 ms | 88 / 17 KB |
| 20 Mbit/s, 40 ms | Desktop | 349 ms | 1,102 ms | 1,059 / 1,102 ms | 101 / 17 KB |
| 20 Mbit/s, 40 ms | Phone | 349 ms | 1,098 ms | 1,092 / 1,098 ms | 88 / 17 KB |
| 9 Mbit/s, 150 ms | Desktop | 834 ms | 2,442 ms | 2,418 / 2,442 ms | 101 / 17 KB |
| 9 Mbit/s, 150 ms | Phone | 834 ms | 2,438 ms | 2,407 / 2,438 ms | 88 / 17 KB |
| 4× CPU, loopback | Desktop | 179 ms | 399 ms | 441 / 399 ms | 101 / 17 KB |
| 4× CPU, loopback | Phone | 184 ms | 374 ms | 413 / 374 ms | 88 / 17 KB |

- The snapshot's frame lands 11 to 48 ms after first contentful paint on
  every profile (the most with a 4× slower CPU). What holds first paint is the render-blocking site
  stylesheet, `ui.css`, which is served uncompressed: 352 KB, against
  43 KB with brotli (#12241). Bytes before first paint are the HTML plus
  that stylesheet: 453 KB with the snapshot (439 KB on phone), 369 KB
  without.
- The app's own first frame doesn't get later on a network. With a 4×
  slower CPU and no network wait, the snapshot's work delays it by about
  40 ms.
- The frame is 58.5 KB gzipped on desktop and 44.5 KB on phone, mostly
  subpixel glyph pixels (722 and 383 glyph tiles).

Pixel diff of the snapshot's frame against the app's first frame, each read
back from its canvas in the same task it drew (`first-frame-diff`):

| Class | Identical pixels | Over 8/255 | Largest channel difference | PSNR |
| --- | --- | --- | --- | --- |
| Desktop, 1980 × 1472 | 99.86% | 0.13% | 33 | 53.0 dB |
| Phone, 712 × 1211 | 99.79% | 0.20% | 27 | 50.5 dB |

Every differing pixel is on a curve's antialiased edge, where WebGL's and
WebGPU's multisample resolve differ by at most a sample. Quads, shadows,
and text match exactly. Grayscale text against the app's subpixel text was
the larger difference (96.9% identical on phone) before the capture added
subpixel glyphs.

### What `/gpui` would take from it

Generalizes as is:

- The format, the painter, and the handoff: `gpui-web` already announces
  the first frame for any app, and the painter draws any scene made of the
  primitives above.
- Capture by recording: any GPUI view runs on the headless platform with
  the web text system, and the recording atlas and scene conversion don't
  know about AgentGraph.
- The page contract: inert data elements, a hashed inline script, an exact
  size and DPR check, and the server HTML as the fallback.

Doesn't:

- A chat is per-person content, so its frame can't be captured at build
  time or cached for everyone. It has to come from the server per request
  (a capture service with the account's data) or, more simply, from the
  last visit: the app records its own settled frame in the browser and
  keeps it for the next load (#12244).
- Size classes. A frame is right only at the exact box size and DPR it was
  captured at. Chat layouts fill the window, so classes would rarely
  match; the last visit's frame has the device's own size (#12243).
- Primitives the chat draws and AgentGraph doesn't: emoji and images
  (polychrome sprites), link underlines, and edge fades. The painter and
  the capture refuse these today.
- Glyph rendering mode: the frames carry subpixel text. A browser where the
  GPU has no dual-source blending draws grayscale text, so its text's color
  fringes change at the swap (#12243).

## The first frame at any window size (2026-10-20, #12243)

- **Capture at any scale.** `first-frame-capture` now runs the view on a
  platform of its own (`first-frame-capture/src/platform.rs`) instead of
  GPUI's test window, which always has a scale of 2 and no subpixel text.
  Its window takes the page's device pixel ratio and draws subpixel text
  directly, so a frame can be captured for any box at any ratio.
- **On request.** The image ships the tool beside the frames
  (`first-frame/first-frame-capture`, built with the `capture` profile).
  When the box matches neither inline frame, the boot script asks
  `/agent/foreman/first-frame?v=DIGEST&w=…&h=…&dpr=…`; the site runs the
  tool once for that size (about 50 ms, one at a time, ten seconds at
  most), keeps the frame, and serves it for a day under the graph's
  digest. The page then paints it as it paints an inline frame.
- **Nothing moves.** AgentGraph's "Scroll past graph" control is now a
  skip link that takes no space until it has focus, so mounting the app
  no longer shifts the box.

Pixel diff of the snapshot against the app's first frame, loopback:

| Window | Snapshot painted | Identical pixels | PSNR |
| --- | --- | --- | --- |
| 1512 × 982 at 2× | 93 ms | 99.88% | 53.4 dB |
| 1728 × 1117 at 2× | 95 ms | 99.88% | 53.7 dB |
| 1280 × 800 at 2× | 96 ms | 99.85% | 52.4 dB |
| 1920 × 1080 at 2× | 83 ms | 99.88% | 53.7 dB |
| 1440 × 900 at 1× | 90 ms | 99.73% | 50.1 dB |
| 390 × 844 at 3× | 92 ms | 99.86% | 52.2 dB |
| 412 × 915 at 2.625× | 100 ms | 99.86% | 52.2 dB |

## WebGPU-only modules, preloads, and shared fonts (2026-10-20)

- **The GPUI modules were already WebGPU-only.** `gpui_wgpu` takes wgpu's
  default features, which on wasm32 include `webgpu` but not `webgl`
  (`cargo tree --target wasm32-unknown-unknown -e features -i wgpu`). So
  dropping a WebGL2 backend saves nothing for `/demo/gpui`, `/gpui`, or
  AgentGraph. A WebGL2 fallback can't come from wgpu either: GPUI's shaders
  read their instances from storage buffers, which WebGL2 doesn't have. A
  fallback would be a WebGL2 renderer of GPUI's scene, which is what the
  first-frame painter already is for the primitives it supports (#12297).
  The start scripts now check for WebGPU before downloading a module; the
  chat's check used to accept WebGL2, where the module would then fail.
- **Everglade** is the one module built with wgpu's WebGL2 backend, and
  its renderer runs on it. Its module is 37.6 MB with brotli in
  production, so the GL backend (about 0.5 MB) is under 2% of it; a split
  isn't worth a second build.
- **Preloads.** AgentGraph's, the demo's, and the chat's pages preload the
  glue (`modulepreload`) and the module (`preload as=fetch crossorigin`),
  so the module starts downloading with the page instead of after the
  start script and the glue have each made a round trip.
- **Shared fonts** (#12233): the apps take Geist and Geist Mono from
  `/gpui-fonts/` instead of compiling them in.

AgentGraph on `/agent/foreman`, measured as before (headless Chrome 155,
cache off, 5 runs, medians), with `ui.css` now served compressed (#12241):

| Profile | App's first frame before (2026-10-19) | Now | Snapshot frame now |
| --- | --- | --- | --- |
| 20 Mbit/s, 40 ms | 1,059 ms | 878 ms | 215 ms |
| 9 Mbit/s, 150 ms | 2,418 ms | 1,942 ms | 506 ms |

The module is 1.39 MB with brotli (was 1.51 MB) plus 213 KB of fonts that
any other GPUI page has already cached. On loopback and with a 4× slower
CPU the app's first frame was later than the day before (202 and 722 ms
against 159 and 441 ms) while other agents' builds were loading the Mac;
those runs aren't comparable.

## Deploy note for `/demo/gpui`

The website image now builds the demo in the `gpuiweb` stages of
`crates/openagents-web/Dockerfile` (Rust 1.98.1) and copies it to
`/srv/gpui-demo`. The server uses that directory without a flag, so the next
image deploy serves `/demo/gpui`. The Docker stage hasn't run in Cloud Build
yet; the first staging build is its check.
