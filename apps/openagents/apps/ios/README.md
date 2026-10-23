# The OpenAgents mobile app for iOS

The OpenAgents mobile app (formerly OpenAgents Coder) uses a UIKit shell over the Rust mobile core. The phone
is a **peer device**: it mirrors the workspace registry, joins per-chat session
rooms, and drives remote engines through the durable command ledger. No agent
runs on the phone.

**Rust decides what to paint and where; Swift paints, scrolls and handles
gestures.** Everything in Rust is platform-neutral — the future Android app
links the same library. See [`docs/mobile-rewrite.md`](../../docs/mobile-rewrite.md).

## Build & run

Requires Xcode 26+ and Rust 1.98.1 or newer. Keep the upstream scheme and
internal crate names. The installed app is **OpenAgents**
(`com.openagents.app`), alongside an existing OpenAgents installation.

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim --toolchain 1.98.1
RUSTUP_TOOLCHAIN=1.98.1 CARGO_TARGET_DIR="$HOME/work/coder-desktop-target-agent1" \
  xcodebuild -project apps/ios/OpenAgents.xcodeproj -scheme OpenAgents \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro Max' \
  CODE_SIGNING_ALLOWED=NO build
```

Signed out, the first screen is a chat answered by OpenAgents
(`Session/CloudChatViewController.swift`, #11978): a few messages work with no
account, then a sheet asks the person to sign in, and after sign-in the chat
moves to the account. **Sign in** sits in the top bar and **See a demo** under
the composer. Signed in with no computer online, **New session** runs on
OpenAgents by default (#11979); **Run on** offers the computers.
Sign-in uses the shared `openagents-account` public SDK flow: approve the code on
`openagents.com`, then return to the app. Rust stores the session in Keychain
and opens only that account’s sync cache. **Settings > Sign Out** removes the
credential and the account’s local chats. The default sync server comes from
`openagents-client` (`https://sync.openagents.com`), through the mobile facade.
See [`FORK.md`](../../FORK.md) for changed seams and verification.

The OpenAgents target's **Rust core** build phase runs `scripts/ios/build-core.sh`,
which builds `crates/mobile` for the active platform (always optimized — the
`mobile` cargo profile) and refreshes the committed UniFFI bindings in
`OpenAgents/Core/Generated/`. `OPENAGENTS_SKIP_CORE=1` reuses the last built library
when iterating on Swift only.

## Layout

```
OpenAgents/
  App/         AppDelegate, SceneDelegate, AppModel (owns CoreClient; maps
               workspace snapshots to view models; Keychain credentials)
  Core/        Generated UniFFI bindings; Fonts (the exact bytes Rust measures,
               registered with CoreText) + CoreText fallback measurer
  Design/      Palette (OpenAgents color roles → light/dark), Glass helpers,
               cell-grid status glyph, harness brand marks
  Transcript/  TranscriptListView (virtualized scroll host over LayoutFrame),
               RowView/RowModel (CoreText painter for Rust display lists,
               streaming veil), FrameRelay
  Session/     SessionViewController, SessionSource (Core/Fixture), new-session
               canvas
  Composer/    ComposerBar (glass capsule ⇄ card with inline context chips;
               send/queue/steer/stop), question and queue panels, attachments
  Threads/     Sessions (foldable sidebar sections)/folder/search lists,
               cells, new-project folder browser
  Shell/       iPhone drawer shell (one page at a time under a menu button;
               the drawer holds Sessions, search, Home and Settings; a
               floating "New session" capsule with live summary), iPad
               split, sign-in
  Debug/       Transcript lab + hitch meter
OpenAgentsLiveActivity/  Widget extension: the voice call's Live Activity (Dynamic
               Island + Lock Screen), with the orb's states exported from
               openagents-orb as vector assets
OpenAgentsShared/   Compiled into the app and the extension: the activity's
               attributes and its mute/end App Intents
```

### Transcript pipeline

1. `openagents-client` applies session-doc updates incrementally (O(changed
   entries)) and publishes immutable snapshots.
2. `TranscriptView.attach(client, chatId)` subscribes **in Rust**; the layout
   thread turns entries into rows (one per markdown block / user message / tool
   group), reparses streaming markdown incrementally, and measures every row
   with `openagents-text` at the viewport width.
3. Each pass publishes a `LayoutFrame`: exact heights + prefix-sum offsets.
   `rowsIn(y0, y1)` is a binary search; `display(i)` builds a display list
   (text runs at exact positions, boxes, links, scrollers, widgets).
4. `TranscriptListView` positions reusable `RowView`s at those offsets and
   paints runs with CoreText at the Rust coordinates — measurement and
   rendering can't disagree. Display models for rows beyond the viewport are
   prefetched off the main thread.
5. Agent visuals (`show_visual` Luau scripts) get a worker thread each
   (`crates/mobile/src/visual`): it runs the script, lays the node tree out,
   draws canvases and charts as vector paint, renders scenes and shaders with
   wgpu, and reports its height back to the row. `VisualView` paints what
   arrives (rasterized on the worker thread) and forwards touches; the demo's
   "Agent visuals" chat (`-demo -route chat:chat-visuals`) has one of each.

## Medieval Town

Open the navigation menu and choose **Medieval Town**. Use the left stick to
move, drag the right half to look, and use **Run** or **Jump**. The native
mobile scene keeps the town square, nearby buildings, and distant castle,
with simplified lighting and one texture atlas. Its first visit downloads
about 12.7 MB; later visits use a verified local cache.

`-demo -route medieval-town -town-bench` runs an offline-account simulator
check against the real scene download: five seconds of warm-up, then 30
seconds of movement and camera turns. Completed GPU frames and frame timing
are saved to `Documents/medieval-town-benchmark.json`.

The 2026-10-23 Release check on the iPhone 17 simulator (iOS 26.5, M5 Max)
completed 1,800 GPU frames in 30 seconds: 60.0 FPS, a 16.85 ms 95th-percentile
frame interval, and no intervals over 25 ms. The camera moved 21.76 m and turned
during the check. Touch movement, look, and background/resume checks also passed.

## Launch arguments

| Arg | Effect |
| --- | --- |
| `-demo` | Offline demo workspace (Rust `DemoHost`: registry, docs, streaming replies) |
| `-fast` / `-longreply` | Demo stream speed / reply length |
| `-big` / `-huge` | Demo transcripts with 120 / 600 turns |
| `-route chat:<id>` / `new` / `more` / `search` | Open a screen at launch |
| `-demo -route welcome` / `-demo -route cloud` | The demo's chat answered by OpenAgents (fixed replies, no server): as the signed-out first screen, or in the shell |
| `-signedout` | Debug only: sign out and clear this app’s account credential |
| `-dev <userId> <orgId> [-edge <url>]` | Debug only: dev bearer against local coder-sync with explicit dev auth; not persisted |
| `-harness <id>` | Default harness for new sessions (`mock` for live-stack tests) |
| `-bench` (with `-lab`) | Display-link fling benchmark; writes `Documents/bench.json` |
| `-measureopen` | Session open latency; writes `Documents/open.json` |
| `-lab [-turns N] [-autostream] [-autoscroll] [-top] [-meter]` | Transcript lab over fixture markdown, with an on-screen hitch meter |

## Tests

```sh
# Rust
cargo test -p openagents-text -p openagents-markdown -p openagents-client -p openagents-mobile
cargo test --release -p openagents-mobile --lib bench_layout -- --ignored --nocapture

# Line-break accuracy vs CoreText (same font bytes), flows, hitch benchmarks
xcodebuild … -only-testing:OpenAgentsTests/LineBreakAccuracyTests test
xcodebuild … -only-testing:OpenAgentsUITests/SessionFlowTests test
xcodebuild … -only-testing:OpenAgentsUITests/ScrollPerformanceTests test
```

### Marketing screenshots

`MarketingShotsTests` retakes the store and launch screenshots on the offline
demo: the signed-out first screen and its first answer, the "Run this on your
own computer" card, a longer chat, Home, and the session list. Nothing signs
in, and every file is named `demo-*.png`. Shots go to the folder in
`TEST_RUNNER_MARKETING_SHOTS_DIR` (and into the test result); they aren't
committed.

```sh
RUSTUP_TOOLCHAIN=1.98.1 TEST_RUNNER_MARKETING_SHOTS_DIR="$HOME/work/film-out/shots" \
  xcodebuild -project apps/ios/OpenAgents.xcodeproj -scheme OpenAgents \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro Max' \
  CODE_SIGNING_ALLOWED=NO -only-testing:OpenAgentsUITests/MarketingShotsTests test
```

For deployed Rust sync checks, use the [live desktop sync check](#live-desktop-sync-check).

## Session notifications and distribution

The first OpenAgents Coder release has no push notifications or permission
prompt. OpenAgentsInc/openagents#11853 tracks their return through our push
gateway. Voice Live Activity remains a separate extension.

Use [the release guide](release/README.md) and `scripts/ios/archive.py` for
local archives. The script stops before upload. OpenAgentsInc/openagents#11850
tracks preparation; signing, upload, and device checks belong to the owner.

## Live desktop sync check

The monorepo's `docs/mobile/coder-desktop-phone-e2e.md` documents the dedicated
account check against staging and production. The simulator-only
`OPENAGENTS_ACCOUNT_FIXTURE=1` core build and Swift's `CODER_ACCOUNT_FIXTURE` flag
allow one-use loopback credential delivery. The account token stays in Rust and
Keychain. `-account-fixture` separates test caches, account entries, display
profiles, device IDs, and drafts from ordinary app use. Neither fixture flag is
part of a distribution build.
