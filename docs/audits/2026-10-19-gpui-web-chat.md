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
| No WebGPU | | wgpu falls back to WebGL2. With neither, or with JavaScript off, the server HTML stays |
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

## Deploy note for `/demo/gpui`

The website image now builds the demo in the `gpuiweb` stages of
`crates/openagents-web/Dockerfile` (Rust 1.98.1) and copies it to
`/srv/gpui-demo`. The server uses that directory without a flag, so the next
image deploy serves `/demo/gpui`. The Docker stage hasn't run in Cloud Build
yet; the first staging build is its check.
