# Show, don't type: Scrawl-style captures in the desktop app

2026-10-19. Spec for bringing [Scrawl](https://github.com/kitlangton/scrawl)
(Kit Langton, MIT, Rust and GPUI with a Swift helper) into the OpenAgents
desktop app, so a person can show an agent a problem instead of describing
it.

## What Scrawl does

You press one shortcut (`⌥⇧S`), then talk, click around, and draw on the
screen. You press it again to stop. Scrawl doesn't hand the agent a video. It
builds a small bundle an agent can read:

| File | What it holds |
|---|---|
| `contact-sheet.png` | 3 to 6 annotated keyframes on one image, each with its time and the words spoken around it |
| `frames/frame-XX.png` | The same keyframes at full resolution |
| `summary.md` | A time-aligned transcript and an interaction timeline: clicks, keys (no secrets), strokes, and app and window changes |
| `scrawl.json` | The manifest that ties it together, including where the capture was sent |

Keyframes come from events: a stroke, a click, a scene change, or a spoken
cue ("now", "here", "look", "watch"). They don't come from a fixed interval.
Transcription runs on device: Apple SpeechAnalyzer by default, with Hex as an
optional engine. The agent can ask for frames in a time window
(`scrawl frames --from 0:02 --to 0:07`). Captures of a local dev site go to
that project's coding session automatically. Scrawl finds it from a one-line
page snippet, or from the process listening on the port. Other features: a
cabled iPhone mirror, an in-app iOS recorder (ScrawlKit), auto-crop to the
active area, MP4 export, and retention and cleanup.

Source layout (`src/`, about 1.9 MB of Rust): `capture.rs` (ScreenCaptureKit),
`sketch.rs` and `laser_overlay.rs` (the ink overlay, Metal), `keyframes.rs`,
`render.rs` (the contact sheet), `transcribe.rs` and `hex.rs`, `routing.rs`
and `inbox.rs` and `delivery.rs` and `handoff.rs` (sending), `autocrop.rs`,
`phone.rs`, `export/`, `retention.rs`, `ui.rs` (the GPUI review window and
menu bar), `onboarding.rs` and `settings.rs`. `helper/` is a Swift executable
for the frameworks Rust can't reach (SpeechAnalyzer, the iPhone mirror,
movie decoding, and the SwiftUI welcome window), spoken to as JSON over
stdio.

## Why it fits us

- **Same stack.** Scrawl is Rust on GPUI, with a Swift helper process. That's
  exactly our desktop app's shape (`apps/openagents`, GPUI) and our
  `swift/lev-bridge` precedent. It isn't a port from another language.
- **It closes the loop we already have.** Our chats already take
  attachments, and agents already read images. A capture is a better
  attachment: pictures, plus what was said and done while looking at them.
- **The routing matches our model.** Scrawl's "which session should this go
  to" is our "which chat or project owns this dev server", and we already
  know every chat's checkout and computer.
- **The phone.** ScrawlKit's in-app recorder maps onto our iOS and Android
  apps reporting a bug from the phone into the chat that's building it.

## What we build

### 1. A capture crate: `apps/openagents/crates/capture`

Import Scrawl's capture core as a library crate. Don't vendor it as a
second app. Our tree uses OpenAgents names, so record the port in
`apps/openagents/docs/downstream-patches.md`, keep the MIT notice and
"Based on Scrawl by Kit Langton", and port upstream fixes by hand, as we do
for Zeron.

- **Bring over:** the bundle model and manifest (`model.rs`), keyframe
  selection (`keyframes.rs`), contact-sheet rendering (`render.rs`),
  auto-crop (`autocrop.rs`), timeline building, MP4 export (`export/`), and
  retention.
- **Platform capture behind one trait,** `CaptureSource`: frames, audio,
  pointer and key events, with timestamps on one clock.
  - **macOS:** Scrawl's ScreenCaptureKit code (`capture.rs`) and its global
    mouse monitor and event tap.
  - **Linux (new):** the `xdg-desktop-portal` ScreenCast interface over
    PipeWire, plus input events from the compositor. On CoderOS, our own
    compositor (`crates/coder-compositor`) can provide frames and input
    directly through the desk protocol, with no portal prompt.
- **The ink overlay** (`sketch.rs`, `laser_overlay.rs`): on macOS it's a
  transparent always-on-top window, as in Scrawl. On Linux it's a
  layer-shell overlay surface, and the CoderOS compositor can draw marks
  natively.
- **Transcription:** route it through our voice stack (`crates/voice`,
  `voice-media`) rather than adding a third engine. Use Apple SpeechAnalyzer
  through a Swift helper on macOS, extending the `lev-bridge` pattern or a
  sibling helper, and the voice stack's local model on Linux. Keep word
  timings: keyframe text and spoken cues depend on them.
- **Not imported:** Scrawl's own menu-bar app shell, Sparkle updates, its
  onboarding window (we have ours), the marketing site (TypeScript), and
  `sdk.js` as shipped. See routing below.

### 2. In the desktop app

- **Shortcut:** a global `⌥⇧S`, rebindable in Settings, plus a Record
  button in the composer and the command palette. Tapping records the
  screen under the pointer. Holding opens the Area · Window · Screen ·
  Phone switcher. `Esc` cancels.
- **While recording:** a small island, our equivalent of Scrawl's pill, shows
  the time and mode. Hold Shift to draw, and `⌘Z` undoes a mark. Clicks pass
  through to the app underneath.
- **When it stops:** the bundle lands in the current chat's composer as one
  "Capture" attachment card. The card shows the contact sheet and transcript,
  with a play button for the low-res video. The person can edit the
  transcript or add a line, then send. Agents receive `contact-sheet.png` and
  `summary.md` as message parts, with the manifest as structured data.
- **Look closer (MCP):** a `capture_frames` tool on the OpenAgents MCP server
  (`apps/openagents/crates/mcp`), with arguments capture ID, `from`, `to`,
  and an optional `max`. It lets the agent pull more frames from a time
  window, so the first message stays small. A second tool,
  `capture_timeline`, returns events in a window.
- **Paste shortcut:** `⌥⇧V` pastes the last capture's contact sheet and
  summary into whatever app is in front, for agents outside our app.

### 3. Routing a capture to the right chat

The bundle's `destination` names one of our chats, not an OpenCode session.

1. If the capture ends on a browser tab served from `localhost` (macOS reads
   the front tab's address through AppleScript, as Scrawl does; on Linux, use
   the browser extension or the compositor's window title), find the process
   listening on that port. That gives its working directory and its git root.
   The capture goes to the most recently active chat whose checkout is in
   that repository, on this computer.
2. The window or app that was captured, mapped through the same rule.
3. Otherwise, the chat that was in front in our app, or the composer of a
   new chat.

Scrawl's page snippet (`<script src="http://127.0.0.1:47777/sdk.js"
data-session=…>`) lets a page register itself. We'd serve the same idea
from the engine, keyed by chat ID. It stays loopback-only without a token,
as in Scrawl. The snippet is a tiny script injected into the user's own dev
page, not product code, so it falls under the same "adapter glue" exception
as the harness shims. Mark it that way in AGENTS.md before adding it.

### 4. Phones

- **iOS and Android:** a Record button in the app's bug-report path records
  the app's own screen (ReplayKit on iOS, MediaProjection on Android) with
  narration and touches. It packs the same bundle in the shared Rust core
  (`apps/openagents/crates/mobile`) and sends it to the chat or project
  building the app, which pairs naturally with our Design Lab and parity
  runs.
- **Cabled iPhone mirror on macOS:** later, through the helper, as Scrawl
  does with CoreMediaIO.

## Privacy and permissions

- **Nothing leaves the computer** until the person sends the capture to a
  chat. Transcription is on device, and bundles live under the app's data
  folder with retention: 14 days by default, and pinned captures are kept.
- **No secrets:** key presses in password fields and secure-input mode are
  never logged, matching Scrawl.
- **macOS permissions:** Screen and System Audio Recording, Microphone,
  Accessibility for the event tap, and Camera only for the iPhone mirror.
  Each is requested from our onboarding, in plain words, the first time the
  feature is used, not at install.
- **Linux:** the portal's own consent dialog. On CoderOS the owner can grant
  it once.

## Phases

| Phase | Scope | Size |
|---|---|---|
| 1 | Capture crate with the macOS source, bundle, keyframes, contact sheet, transcript (Apple via helper), composer attachment, and agents reading it | 1 to 2 weeks |
| 2 | Routing to chats by dev-server port, the `capture_frames` and `capture_timeline` MCP tools, and `⌥⇧V` paste | about 1 week |
| 3 | Linux: portal ScreenCast over PipeWire, layer-shell ink, the CoderOS compositor's native source, and Linux transcription | 1 to 2 weeks |
| 4 | Phones: the in-app recorder on iOS and Android through the shared core | 1 to 2 weeks |
| 5 | Auto-crop tuning, MP4 export for sharing, the iPhone mirror, and retention settings | as needed |

## Open questions for the owner

- Should captures also reach agents outside our app (Claude Code or Codex in
  a terminal) by writing the bundle path into the session, or only through
  our chats?
- Default retention: 14 days, or keep everything until deleted?
- Do we contact Kit about collaborating on upstream instead of a one-way
  port? Scrawl is active: updated today, with 346 stars.

## References

- Scrawl: https://github.com/kitlangton/scrawl (README, `docs/brief.md`,
  `src/`, `helper/`)
- Our desktop app: `apps/openagents/OPENAGENTS.md`;
  `apps/openagents/docs/downstream-patches.md` for recording imports
- The Swift helper precedent: `swift/lev-bridge`, `crates/lev`
