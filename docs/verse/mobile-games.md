# Verse games in the OpenAgents mobile app

Status: proposed, 2026-10-18. This document specifies an integration into
`apps/openagents`, based on source at `cd79148f9a`. It does not describe a
shipped mobile feature. The first game is **The Ice Cream Man**; **Grow Little
Bunny** is the second-game acceptance case for the same app architecture.

## Recommendation

Add **Games** to the mobile app's navigation and open each game full screen.
Ship the existing Rust/Wasm builds as app resources and run them in an isolated
WebView. Reuse the standalone Ice Cream Man iOS wrapper's offline-loading
approach, but put game selection, session lifecycle, saves, and app-facing
views in shared Rust. UIKit and Kotlin mount the player and forward platform
events. Rust Native supplies the catalog, game details, pause menu, settings,
and result cards on both platforms.

This gets the actual games onto phones without first porting their WebGL2
renderers. Keep a native Verse renderer as a later implementation of the same
player boundary, gated by measured WebView limitations. Do not require a
walkable Verse world, a computer connection, or a model call to play.

The first release includes Ice Cream Man offline on iOS and Android, bundled
with the app. Add Bunny after its offline and presence behavior is separated.
Game Lab versions follow through the same player. Multiplayer, public scores,
downloadable executable game packages, purchases, and cross-device run resume
are separate work.

## What exists today

| Piece | Evidence | Consequence for the mobile app |
| --- | --- | --- |
| Ice Cream Man rules | [`icecream-rules`](../../crates/icecream-rules/src/lib.rs), [`Game` and `Input`](../../crates/icecream-rules/src/game.rs) | Reuse the 30 Hz simulation, seeded house, combat, difficulty, and outcomes. Do not translate them into Swift or Kotlin. |
| Ice Cream Man player | [`icecream-web`](../../crates/icecream-web/src/lib.rs), [`manifest`](../../crates/icecream-web/Cargo.toml) | Game rules are portable; the event loop, DOM HUD, WebGL2, Web Audio, and browser storage are browser adapters. Linking the crate natively does not produce a native player. |
| Existing iPhone/iPad wrapper | [`bins/icecream-ios`](../../bins/icecream-ios/README.md), [`GameViewController.swift`](../../bins/icecream-ios/host/App/GameViewController.swift), [`GameScheme.swift`](../../bins/icecream-ios/host/App/GameScheme.swift) | A bundled build already loads offline in `WKWebView`, with MIME handling, rotation, persistent storage, and controller support. Reuse that experience inside the current app; its separate bundle ID is not the destination. |
| Build packaging | [`build-icecream-web.sh`](../../scripts/build-icecream-web.sh) | Generates Wasm, wasm-bindgen glue, the existing startup script, a page, and `version.txt`. Extend packaging rather than creating another game build pipeline. |
| Bunny | [`design and implementation`](games/grow-little-bunny.md), [`bunny-web` dependencies](../../crates/bunny-web/Cargo.toml) | Reuse its rules and presentation. The meadow currently has its own relay presence path; offline mobile play must not start that connection automatically. |
| Snap Station | [`design and implementation`](games/snap-station.md) | A later catalog entry with different touch and save needs. The host must not assume every game has a joystick or an escape timer. |
| Shared game contract | [`verse-game`](../../crates/verse-game/src/lib.rs) | Supplies discrete input, HUD, `CommunityGame`, and replay contracts. It does not yet express Ice Cream Man's continuous movement, aim, and held tool inputs. |
| Current mobile core | [`openagents-mobile`](../../apps/openagents/crates/mobile/Cargo.toml) | The shared UniFFI core already links domain crates and Rust Native. Add the game host here; it does not currently link a Verse game player. |
| Current phone navigation | [`SplitRootController.swift`](../../apps/openagents/apps/ios/OpenAgents/Shell/SplitRootController.swift) | The app lands in chat and uses drawer/sidebar navigation. Add a Games destination without replacing that landing behavior. |
| Shared cards and media | [`lab-kit` cards](../../crates/lab-kit/src/card.rs), [`mobile visuals`](../../apps/openagents/crates/mobile/src/visual/mod.rs), [`iOS Lab cells`](../../apps/openagents/apps/ios/OpenAgents/Transcript/LabCardView.swift) | Reuse the card-to-player pattern. Existing Lab media and visual surfaces are not a complete game runtime. |
| Native Verse rendering | [`verse::render`](../../crates/verse/src/render.rs), [`engine core`](../../crates/verse-engine/README.md) | Metal-layer and Android-window entry points exist. They draw Verse geometry, not Ice Cream Man's WebGL renderer unchanged. |

The [Ice Cream Man design](games/the-ice-cream-man.md) records phone regressions
already fixed in the browser: sprint while another finger moves, reliable item
switching, and accidental zoom. Preserve those behaviors. The older
[Verse mobile guide](mobile.md) includes retired app paths; its recorded
renderer and lifecycle experience is useful, but its old mobile host is not a
new integration target.

## Player experience

1. Open **Games** from the drawer or sidebar. Show installed games as cards
   with artwork, title, a short description, and **Play**. Do not show
   unfinished games or controls.
2. Open a game's details to see **Play**, **How to play**, and its settings.
   Ice Cream Man includes difficulty, timer, jumpscares, and hold-or-toggle
   sprint. Explain the cartoon chase before the first run.
3. Tap **Play**. Present the player full screen, with a persistent, reachable
   menu control. Support portrait and landscape, safe areas, iPad split
   sizes, and Android window resizing. Do not change chat's orientation rules.
4. Open the menu or press Android Back to pause. Offer **Continue**,
   **Restart**, **Settings**, and **Leave game**. Leaving returns to the same
   app destination and preserves the chat draft.
5. At the end, show the game's result and **Play again**. Ice Cream Man also
   preserves its earned Sprinkle Staff and free-roam continuation. Changing
   difficulty starts a fresh run; changing sound or timer visibility does not.

A chat's game card opens this same full-screen player. Keep an image and a
Play button in the transcript, so scrolling a chat never starts a simulation
or creates multiple live WebViews. Only one game session runs at a time.
Returning to chat pauses it; reopening resumes it while it remains in memory.

## Architecture and ownership

```text
Games navigation / chat game card
                |
                v
openagents-mobile::games                 [proposed]
  catalog, GameSession, save store, Rust Native views
                |
        typed player boundary
                |
        +-------+-----------------------+
        |                               |
UIKit player adapter             Kotlin player adapter
WKWebView + local resources      WebView + local resources
        |                               |
        +------- bundled Rust/Wasm -----+
                  game adapter
                  rules + presentation + audio
```

Add `apps/openagents/crates/mobile/src/games/` for app-specific coordination.
Keep reusable game rules and presentation under root `crates/`. Link them by
path when shared code is needed; do not copy them into the app workspace.

| Layer | Owns |
| --- | --- |
| Shared mobile Rust | Catalog eligibility, validated launch request, session state, lifecycle ordering, save selection, result handling, and Rust Native views and intents. |
| Game Rust, compiled to Wasm | Simulation, camera, drawing, audio cues, gameplay HUD, touch interpretation, and game-specific save encoding. |
| UIKit and Kotlin | WebView creation, bundled resource delivery, platform lifecycle and controller events, safe-area measurements, accessibility mounting, and forwarding typed messages. |
| Existing account and Lab clients | Authorization and fetching an explicitly selected private game document. They never hand account credentials to the game. |

Gameplay HUD elements such as the aim point, ammo, and thumbstick stay with
the existing game renderer. App cards, menus, settings forms, and result sheets
use Rust Native, as required by the [app contract](../../AGENTS.md). Add an
embedded mode to the Rust web adapter that suppresses its DOM cards and
exposes those transitions to the host. Reuse the game's copy and settings
logic; do not maintain competing native and browser menu state.

Follow the [Rust Native view contract](../../crates/rust-native/docs/spec.md)
and [ownership boundaries](../coder/rust-native/architecture.md). A local
`Surface` resource can identify a registered player mount; it must not become
an arbitrary URL or code-loading control. Determine which existing phone
adapter can mount it during the first implementation step, and add the missing
mount support there rather than changing the semantic view into a WebView API.

## Catalog, builds, and isolation

Define a Rust catalog entry with a stable game ID, title, description, artwork,
controls profile, supported orientations, save schema, runtime build ID, and
bundled entry point. Initially register `the-ice-cream-man`; register
`grow-little-bunny` only when its acceptance checks pass. Content comes from a
checked-in allowlist, not a URL supplied by a chat message.

Package generated resources under one directory per game. Record the source
commit and full file hashes in a packaging manifest; keep the existing short
`version.txt` value for display. Include the page and asset hashes as well as
the Wasm and glue. Fail a release build if a required file is missing or stale.
Game runtime updates ship with app releases in the first version. A game's
build ID and its tuning-document version are separate values.

On iOS, adapt the standalone app's scheme-handler approach. On Android, add a
local resource origin with correct HTML, JavaScript, and Wasm MIME types.
Prove module loading and Wasm startup on both platforms before building the
catalog UI. Accept only the registered game's resource paths; reject traversal
and cross-game access instead of retaining the standalone handler's basename
fallback. Do not start a listening HTTP server on the phone.

Use a game-only browser data store/profile, separate from authenticated app
browsing. Permit bundled resources and required in-memory resources; reject
remote subresources and frames in the initial offline player. Open approved
How to play links in the system browser after a user action. Disable popups,
file access outside the resource root, and camera, microphone, location, and
other unrelated permission requests. Keep account tokens, wallet access,
computer grants, and app RPC unavailable to the player.

Retain existing generated wasm-bindgen glue and startup assets. Implement new
game behavior and message handling in Rust, with narrow platform adapters;
do not add TypeScript or a second handwritten JavaScript game implementation.

## Typed player boundary

The following names describe proposed interfaces, not APIs that exist today.
Use a shared, versioned Rust message schema, serialized through a bounded
platform bridge. UniFFI connects the native shell to `GameSession`; a separate
bridge connects that session to the Wasm adapter.

| Direction | Messages |
| --- | --- |
| Host to player | `Start` with game/build IDs, session generation, options, seed, and save; `Pause`; `Resume`; `Resize`; `SetSettings`; `Stop`. |
| Player to host | `Ready`; `Paused` with latest save; `MenuRequested`; `SettingsChanged`; `SaveChanged`; `RunEnded` with summary and save; `Failed`. |

Every message carries its schema version and session generation. Accept
messages only from the active main-frame local origin and the current player.
Reject unknown actions, nonfinite numbers, oversized payloads, stale events,
and mismatched game/build IDs. Start with a 64 KiB serialized message limit;
if a future game needs larger saves, transfer them separately with explicit
bounds. No general-purpose evaluate-code or open-URL command crosses this
boundary. Results update local records; they never authorize payments or
computer actions.

Keep display frames and ordinary touch events inside the player. Send state
changes and completed save snapshots through the bridge, rather than polling
or copying meshes at frame rate. Native controller input, if supported in the
first release, forwards raw buttons and axes; Rust maps them to game actions.
Do not copy the standalone Swift controller-to-game mapping into both shells.

## Input, timing, and lifecycle

Ice Cream Man's `Input` needs a movement vector, aim, held sprint, jump, fire,
interact, and tool selection. Preserve that shape. The existing discrete
`verse-game::GameInput` cannot replace it by emitting repeated direction keys.
The first embedded player can use the existing Rust web input adapter; a
future shared input extension must version its replay encoding and leave old
replays readable. Bunny keeps its discrete and jump-release behavior.

Track each touch by pointer ID. Moving with one thumb must work while another
looks, sprints, fires, interacts, or changes tools. Clear every held action on
pointer cancellation, focus loss, a modal opening, or backgrounding. Preserve
44-point iOS and 48-dp Android minimum touch targets, readable hot-bar spacing,
and the browser game's zoom fixes. Expose labels and focus for menus; do not
claim fully accessible 3D gameplay from accessible buttons alone.

`GameSession` follows `loading → ready → playing → paused → finished`, with
failure and disposal paths from every state. Free roam is an explicit
continuation of the finished run, not a second scored result.

- On backgrounding or an interruption, request pause, clear inputs, suspend
  game audio and frame work, and persist the latest completed save. Do not
  rely on browser blur or throttled animation callbacks to pause the rules.
- On return, show the paused state and require **Continue**. Reset the frame
  clock; elapsed background time never advances the run or its speedrun timer.
- Advance Ice Cream Man at its existing 30 Hz rules rate. Bound foreground
  catch-up work after a stall and interpolate presentation independently.
  Preserve simulation-based run timing when presentation drops frames.
- Treat pause acknowledgments as bounded operations. If a WebView stops
  responding, retain the last saved progress and offer to reopen the game;
  do not freeze app navigation waiting for it.
- On WebView process death or memory eviction, recreate the player with saved
  settings and progression, then offer a new run. The first release does not
  promise to restore an interrupted house mid-run.
- On leaving or switching games, stop frame callbacks, sound, and controller
  listeners and dispose of the old player. Ignore late messages by generation.

A fixed seed plus floating-point rules is not proof of identical results
across native and Wasm builds. Ice Cream Man best times remain local until
cross-platform replay verification exists.

## Saves and account boundaries

Use a host-owned, versioned Rust save store, keyed by local profile and game.
Keep settings, tutorial completion, progression, and personal bests separate
from temporary run state. Write atomically and preserve the previous readable
save if an upgrade or write fails. The game serializes its own payload; the
host validates the envelope and supplies it on launch. Send `SaveChanged` after settings or progression changes, not only at the end
of a run. Embedded mode uses this store instead of maintaining a second
localStorage authority.

Reuse Ice Cream Man's [`settings`](../../crates/icecream-web/src/settings.rs)
parsing and best-time rules. Preserve Easy, Normal, and Hard records separately;
keep modified Game Lab runs and bot runs out of ordinary personal bests. Record
the rules identity with a best so later balance changes do not silently mix
incompatible times. Bunny saves wins and garden progress using its existing
save format and upgrades.

Allow offline play without a new sign-in step. Keep guest progress local.
Signing out pauses and disposes of the current player before another profile
opens it; never show one account's private Game Lab document to another.
Specify an explicit import if guest saves later move into an account. Do not
automatically merge them. Browser saves and the standalone Ice Cream Man app's
sandbox are not readable from the OpenAgents app; no automatic migration is
promised. Reinstall recovery and cloud save sync are later features.

## Game Lab and additional games

The existing [Game Lab](../game-lab/README.md) edits bounded `game.v1` data,
keeps versions, and measures them with the rules bot. A mobile card should
resolve an authorized document through the existing Lab/account path, check
that the installed runtime supports it, and launch that document through
`GameSession`. It must not load a computer's localhost URL, download a private
web page with app credentials, or evaluate code returned in a chat.

Pin the selected document version and digest for the whole run. If the runtime
cannot understand it, show **Update OpenAgents to play this version** rather
than substituting different rules. Keep the last playable bundled game
available offline. Notes and comparisons remain the existing Game Lab tools;
this integration does not put the bot or editing service in Swift or Kotlin.

Bunny's second-game work includes an explicit offline launch mode that does
not create or publish a meadow identity. Presence can return later as an
explicit feature backed by the shared Rust networking crates. Ice Cream Man's
proposed co-op also stays separate from this single-player release. Do not
create another session-sync service beside the app's existing Loro rooms and
device grants; choose a game-authority protocol before adding live co-op.

## Native renderer follow-up

A native player can replace the WebView without changing catalog entries,
app navigation, save ownership, or cards. It still requires real work:

1. Extract portable Ice Cream Man camera, meshes, scene construction, and
   settings into a root Rust presentation crate, keeping browser adapters in
   `icecream-web`. Both clients consume the same code.
2. Adapt that scene to Verse's renderer. Port its materials, outlines, fog,
   lights, particles, and audio; existing Metal/Android constructors do not
   translate WebGL shaders or Web Audio calls.
3. Mount native surfaces through the current app's UIKit and Kotlin adapters,
   with Rust owning input, rules, GPU lifecycle, and resource limits. Do not
   restore a retired mobile crate or run a second app.
4. Check the root and apps Cargo workspaces independently. Verify wgpu,
   toolchain, native-library, and feature compatibility before promising a
   direct renderer dependency. Keep desktop-only features out of phone builds.
5. Compare fixed scenes, touch behavior, saves, and sustained device performance
   against the embedded player before switching the default.

Choose this work when WebView measurements show unacceptable frame pacing,
memory, audio interruption behavior, or a need for seamless Verse-world
transitions. A native conversion is not a prerequisite for the offline catalog.

## Delivery and acceptance

These are implementation milestones, not completed checks or release dates.

| Step | Deliverable | Exit check |
| --- | --- | --- |
| 1. Player proof | One bundled Ice Cream Man build in the current iOS and Android shells; verify Rust Native mounting and bridge transport. | Cold-start and play in airplane mode on both platforms; report build ID, startup time, package-size increase, and peak memory. |
| 2. Shared host | Rust catalog, session state, saves, message validation, and Games navigation. | Launch, pause, resume, restart, leave, and reopen through the same Rust state transitions; preserve chat drafts. |
| 3. Complete Ice Cream Man | Embedded-mode menus/cards, settings, results, touch, rotation, and interruption handling. | Complete a run on each platform; verify concurrent touches, difficulty records, gentle ending, and free roam; kill the WebView and recover without losing saved progress. |
| 4. Second game | Bunny entry using the same host and packaging conventions. | Complete and save a garden offline, reopen its progression, and observe no relay connection in offline mode. No game-specific host controller fork. |
| 5. Game Lab | Authorized pinned data versions open from chat cards. | Two versions produce their expected settings; stale and unsupported versions fail clearly; modified runs do not change ordinary best times. |
| 6. Delivery | Record attribution, package resources, and update app release instructions. | A clean app release build includes the exact checked manifest; store metadata and physical-device checks are recorded separately. |

For implementation, run targeted Rust tests for changed packages, plus format
only changed files. Test meaningful boundaries: malformed/stale bridge events,
background timing, failed writes, save upgrades, account switches, and resource
path rejection. Build Wasm with the root workspace's pinned toolchain and
matching wasm-bindgen CLI; build the mobile library from `apps/openagents`
with its own toolchain and lockfile. Use the shared Cargo targets and compiler
state helper required by `AGENTS.md`.

Before release, record real-device results on iPhone, iPad, and Android,
including a lower-end supported Android device. Proposed targets are input
response within 100 ms, 60 fps on representative devices with a 30 fps quality
fallback, and stable behavior through a 15-minute run. Measure startup and
memory before setting device-specific budgets; simulator captures cannot
establish thermals or battery use. Check audio interruptions, large text in
menus, screen-reader navigation, small viewports, and two-thumb controls.

Use local fixtures for offline work and the dedicated test account for private
Lab checks. Leave no production chats or drafts behind. Record the integration
in `apps/openagents/docs/downstream-patches.md` when code lands. Follow the
[current iOS release guide](../../apps/openagents/apps/ios/release/README.md)
and [Android release guide](../../apps/openagents/apps/android/release/README.md).
Review store age/content and privacy declarations for the added games before
submission; this proposal makes no claim of store approval.
