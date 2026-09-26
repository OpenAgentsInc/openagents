# Shared Verse desktop/iOS verification — September 26, 2026

[Issue #9698](https://github.com/OpenAgentsInc/openagents/issues/9698) brings the
existing Verse city into the Coder iOS application using a generic Rust Native
surface. The [mobile guide](../../verse/mobile.md) defines the delivered scope
and the desktop-only panels. The app retains `com.openagents.coder`, team
`HQWSG26L43`, marketing version `0.5.0`, and the existing distribution profile.
Build `39` follows the delivered reader build `38`.

## Shared implementation

Desktop and iOS use `verse::runtime::WorldRuntime`, the existing seeded world,
collision/controller, camera, avatar/gait, following agent, shaders, and wgpu
scene. The desktop event loop supplies keyboard/mouse input; the iOS adapter
supplies bounded touch intents through an independent main-thread render handle.
The saved-history reader keeps its separate serial worker and observation grant.

Rust Native's v2 tree adds a labeled locally registered `Surface`. Its generic
viewport/lifetime contract checks dimensions and monotonic frame timing, resets
elapsed time on pause, and rejects reuse after disposal. It has no product,
GPU, networking, credential, or palette dependency. Coder's colors stay in
`coder-ui`; geometry and multiplayer stay in Verse.

The mobile dependency graph disables Verse's desktop features: no Coder model
harness, Gym, knowledge store, or winit enters that target. Existing desktop
features remain enabled by default. Only deterministic fixtures, local test
sockets, GPU rendering, native tests, and build/distribution operations were
used. No model, benchmark, or private-chat publication was performed.

## Checks and defects found

- All 16 generic Rust Native validation and surface tests pass, covering local resource IDs,
  invalid dimensions, nonfinite/backward timestamps, pause/resume, and disposal.
- All 13 mobile Rust tests pass, covering real shared movement from touch input, input
  cancellation, relay URL admission, and the retained reader's cache, paging,
  follow, revocation, and authenticated synthetic host/relay exchange.
- Verse's 103 portable and 118 desktop library tests pass, covering simulation, replay formatting,
  renderer bounds, protocol validation, injected identities, bounded queues,
  cancellation during a stalled handshake, AUTH subscription restoration,
  original-signer spawn recovery, and atomic PM queue admission. Four opt-in
  real-relay integration functions return through their existing skip paths;
  they are not counted as live-relay evidence.
- Strict affected-package Clippy and formatting checks are recorded with the
  [retained native evidence](../../../bins/coder-ios/verification/2026-09-26-verse/README.md).
- An actual offline desktop GPU capture checks the city, avatar, following
  agent, and existing HUD. Its chat text is the capture fixture, not a live
  conversation or model answer.
- Six native UI tests check four reader regressions, actual presented Metal
  frames and horizontal-plane movement, and Chats/Verse/background resume.
  Native decoder checks include the v2 Surface variant. The final six tests
  passed in 79.259 seconds with matching before/after source digests.

The first iOS render check failed before drawing: default wgpu device limits
requested 16 inter-stage variables while the simulator supports 15. The scene
needs three. The renderer now requests its actual portable requirements and
verifies them against the adapter; a regression accepts 15 and refuses fewer
than three. The initial failure remains recorded. Frame counters increment
only after a drawable is submitted and presented, not when a timer fires.

The immediate XCTest screenshot after foregrounding omitted cached static
native labels while the world and changing counters were visible. A settled
independent simulator capture after reopening showed the labels, tabs, and
status bar restored while frames continued. Both images remain in the receipt;
the immediate capture is not presented as a complete visual result.

The extraction also corrected unbounded relay queues, noncancellable workers,
TLS provider ambiguity, AUTH resubscription, unsigned metadata, another
publisher's state being mistaken for one's own spawn, and stale-session motion
replay. Chat queue admission remains distinct from relay delivery: an admitted
local send does not prove acknowledgment, persistence, or exactly-once delivery.

## Distribution

Build 39 is prepared for an optimized archive from committed source. The final
App Store Connect processing and internal TestFlight status will be recorded
here and in the distribution receipt after upload succeeds.

## Coverage limits

- Native checks use iPhone 17 Pro Simulator on iOS 26.5 and Xcode 26.6. They do
  not establish physical-device thermals, sustained frame rate, battery use,
  VoiceOver gameplay, or behavior on the minimum supported iOS 17 runtime.
- A synthetic local relay test verifies transport behavior. A real two-device
  mobile/desktop world session is not claimed by an offline screenshot.
- Mobile starts offline. Joining a compatible WSS relay explicitly publishes
  the separate Verse identity and motion. Background cancellation is not proof
  that an offline presence update reached the relay. Position recovery is
  bounded and can fall back before a slow connection completes.
- Mobile has movement and presence, not the desktop XP, replay-selection,
  model-chat, public-feed, and world-chat panels. Rust Native has a drawing
  surface contract, not a universal 3D engine or a complete native widget set.
- Targeted development and native delivery checks were used. The full
  repository release matrix was not run or made a blocker for other work.
