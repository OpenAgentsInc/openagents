# 0010: Loading zones in the mobile app

Status: Accepted. The owner asked for the Haunted House and the Grid in the
OpenAgents mobile app "very soon", and asked us to choose the way.

## Context

- The Haunted House and the Verse universe are web releases today: immutable
  Aethyrforge builds on Cloud Run, embedded by openagents.com in a
  separate-origin frame through the `aethyrforge-host` protocol, and pinned by
  `crates/openagents-web/content/demos/*.json`.
- The engine's native GPU backend is being written now. It is weeks of
  qualification away from rendering a full zone on a phone
  ([0004](0004-engine-core-and-gpu-interface.md)).
- The mobile app (`apps/openagents`: UIKit on iOS and Kotlin on Android over
  one Rust core) already does the coding-agent work the Grid holds.
- The interim iOS `medieval_town` renderer draws one town natively with its
  own shaders. It is a second renderer of the same content.

## Decision

The shell gets one **zone launcher** and two ways to run a zone. Every zone
uses the same manifest, capability broker, and detail floors
([0009](0009-zones-and-the-grid.md)), whichever runner it gets.

### 1. Web runner (now): the Haunted House and the Verse

- The shell opens a zone's **web release in a platform web view**:
  `WKWebView` on iOS and `android.webkit.WebView` on Android. It is the same
  release ID the website pins, so web and mobile run identical bytes.
- **The shell plays the trusted page's role.** The web view loads only the
  release's exact origin. The shell talks to it over the same
  `aethyrforge-host` messages, bridged through the web view's message channel
  (`WKScriptMessageHandler`, or `WebMessagePort` on Android) instead of
  `postMessage` between frames. Origin checks, scoped ports, size bounds, and
  server-side authorization stay as they are.
- **No credentials enter the web view.** Sign-in, payments, and account
  actions are shell-native sheets. The zone receives short-lived scoped grants
  through the bridge, as it does on the web.
- **Native chrome:** the shell draws the close and back controls, safe areas,
  and loading and failure states, using the design system tokens
  ([0006](0006-one-design-system.md)). A zone that can't start says why and
  offers a retry.
- **Gate:** WebGPU must be available in the web view on the target OS
  version. Record the adapter on real devices before shipping. If a platform's
  web view lacks WebGPU, that platform shows the zone as unavailable with an
  "Open in browser" action. It never shows a broken canvas.
- **Store rules:** zone code runs inside the platform's own web engine and
  ships no native code, which fits Apple's rules for HTML5 mini apps and games
  and Google Play's rule against downloaded native code. Verify the current
  guideline text before the first store submission.

### 2. Native runner (later): the same zones on Aethyr

- When the native `af-gpu` backend and the engine core pass image parity and
  sustained-play gates on pinned phones, a zone can run natively in a
  `CAMetalLayer` or `SurfaceView`, with the same manifest and broker.
- The launcher picks the runner per zone and per device. The native runner
  replaces the web runner one zone at a time. `medieval_town` is removed when
  the native runner draws the town.

### The Grid

The Grid is a **first-party zone compiled into the shell.** The launcher
lists it beside downloaded zones. It opens the app's existing coding-agent
surfaces (sessions, computers, transcripts, approvals), which are already
Rust Native views over `coder-sync`. Nothing is downloaded, so it works
offline and needs no web view. When Aethyr's GPU UI ships, the Grid's views
move to it like any other screen.

## Why not the alternatives

- **Wait for the native runner.** That delays the Haunted House in the app by
  the full native qualification. It isn't "very soon".
- **Rebuild the zones natively per platform,** as `medieval_town` did. It
  makes a second renderer of every zone and forks the content.
- **Open the browser (Safari View Controller or Custom Tabs).** It loses the
  shell's sign-in, chrome, and capability bridge. It stays only as the
  fallback when a web view lacks WebGPU.

## Consequences

- Zone releases keep working with no trusted parent page; the host protocol
  needs a native-bridge transport beside `postMessage`.
- The mobile app links nothing from the engine for the web runner, only the
  launcher, the bridge, and the broker.
- Kiki mobile demos ([0008](0008-launch-order.md)) use the web runner first.
