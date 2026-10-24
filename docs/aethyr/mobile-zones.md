# Zones in the mobile app

Implements [#12591](https://github.com/OpenAgentsInc/openagents/issues/12591)
under ADR 0009 (zones and the Grid) and ADR 0010 (loading zones in the
mobile app). The app is `apps/openagents`. Its logic is in Rust
(`apps/openagents/crates/mobile/src/zones/`). The iOS (UIKit) and Android
(Kotlin) hosts only present it.

## What the person sees

- A **Zones** button in the sidebar toolbar on iOS (a 2×2 grid icon,
  `sidebar-zones`) and in the floating toolbar on Android. It works signed
  out, because the Haunted House opens as a guest.
- The **launcher**: one Rust Native card per zone, each with a title, a
  summary, how it opens, and an **Open** button:
  1. **The Grid**: the coding-agent zone, built into the app. It opens the
     existing sessions list (the iPhone drawer, the iPad sidebar, or the
     Android navigation pane), which leads to computers and transcripts.
     Nothing is downloaded, and it works offline.
  2. **The Haunted House**: Kiki's house, opened as a guest.
  3. **The Verse**: the universe of planets.
- The **web runner**: the zone's web release, full screen under a native bar
  with Back (web history) and Close. While the page loads, a card says
  "Opening …". If loading fails, the card offers **Try again** and
  **Close**. If the web view has no WebGPU, the card says the zone can't run
  here yet and offers **Open in browser**, which opens the website page
  (`https://openagents.com/new-lander/house` or `/new-lander/verse`). The app
  never shows a broken canvas.

## The zone model

`ZoneManifest` (UniFFI record): `id`, `title`, `summary`, `runner`
(`Builtin` or `Web`), `release_id`, `url`, `origin`, `page_url`, and
`detail_floor`. `detail_floor` is a placeholder (`Any`) until the shared
device profile lands (ADR 0007).

The web entries come from the release JSON the website pins,
`crates/openagents-web/content/demos/haunted-house.json` and
`universe.json`:

- **Bundled at build time.** `include_str!` reads the same files, so a fresh
  install and an offline launch list the pins the website served when the
  app was built.
- **Refreshed at runtime.** The launcher fetches `GET /v1/zones` from
  openagents.com and replaces the web entries if the body passes the same
  checks. On any failure the bundled list stays. A catalog cannot replace or
  shadow the built-in Grid.

`GET /v1/zones` (`crates/openagents-web/src/pages/zones.rs`) is public and
read-only, needs no sign-in, and carries no credentials. Its schema is
`openagents.zones.v1`:

```json
{"schema":"openagents.zones.v1","zones":[{"id":"haunted-house","title":"The Haunted House","summary":"…","page_url":"https://openagents.com/new-lander/house","release":{ …the pinned demo JSON… }}]}
```

The wire types (`ZoneRelease`, `ZoneEntry`, `ZoneCatalog`, `kiki_zone`) are
in `crates/aethyrforge-host/src/zones.rs`, so the website and the app apply
the same validation. A release passes the website's own embed checks: HTTPS,
SDK and engine on one origin, no credentials, query, or fragment, and a
configured release. Catalogs are capped at 64 KiB and 32 zones, with unique
IDs and HTTPS pages.

## The web runner

`CoreZoneRunner` (Rust) holds one zone's state. The hosts are
`apps/ios/OpenAgents/Shell/ZoneWebViewController.swift` (`WKWebView`) and
`apps/android/app/src/main/java/com/openagents/app/zones/ZoneWebActivity.kt`
(`android.webkit.WebView`).

- **Exact origin.** Every navigation is checked with
  `zone_navigation_allowed(origin, url)`. Scheme, host, and port must match
  the release origin, and credentials are refused. Anything else is
  cancelled. Popups and new windows are refused.
- **No credentials in the web view.** iOS uses a non-persistent website data
  store. Android turns off file and content access, multiple windows, and
  third-party cookies. The app never passes its account token, cookies, or
  headers to the page. Sign-in and payments stay native sheets. A zone asks
  for them over the bridge and gets short-lived scoped grants, as it does on
  the web.
- **WebGPU gate.** A probe script reports `'gpu' in navigator` and the
  `navigator.gpu.requestAdapter()` result, with the adapter's vendor,
  architecture, device, and description, on its own channel
  (`aethyrShell`). It runs at document end in the main frame: in the
  `.defaultClient` world on iOS, and through `evaluateJavascript` on Android.
  Rust accepts the report only from the zone's origin and main frame, within
  4 KiB. Without an adapter, the runner switches to the "Open in browser"
  card. The hosts log the result with the OS version, so a device run
  records it:
  - iOS: `log stream --predicate 'subsystem == "com.openagents.app" AND category == "zones"'`.
    The runner's hidden `zone-webgpu` element also carries it as its
    accessibility value for UI tests.
  - Android: `adb logcat -s OpenAgentsZones`. The line includes the WebView
    package version.
- **Safe areas.** The native bar sits below the status bar. The zone fills
  the rest out to the screen edges and pads itself with CSS `env()`. Cards
  sit inside the safe area.
- **Debug entry points.** iOS: `-route zones` opens the launcher, and adding
  `-zone haunted-house` opens that zone's runner. Android debug builds take
  `--es openagents.zones launcher` or `--es openagents.zones haunted-house`.

## The host bridge (native transport)

The same `aethyrforge-host` envelopes, schemas, sequence rules, and byte
bounds (524,288 bytes) run over the web view's own message channel instead of
`postMessage` between frames. See
[the protocol](../verse/aethyrforge-host-protocol.md#native-bridge-transport).

| | iOS | Android |
| --- | --- | --- |
| Page → shell | `WKScriptMessageHandler` named `aethyrHost` (page world) | `WebViewCompat.addWebMessageListener` object `aethyrHost`, allowed only on the zone origin |
| Shell → page | `callAsyncJavaScript`, with the frame passed as an argument, calling `window.aethyrHost.onmessage({data})` | `JavaScriptReplyProxy.postMessage` |
| Page-visible object | A document-start script defines the same `window.aethyrHost` object Android injects | Injected by WebView |
| Peer check | `frameInfo.securityOrigin` and `isMainFrame` | `sourceOrigin` and `isMainFrame` |

On the shell side, Rust's `NativeHost` (`crates/aethyrforge-host/src/native.rs`)
does the following:

- Refuses frames over the size bound, from another origin, or from a subframe.
- Answers the bootstrap string `openagents.aethyrforge.host/7/listening` with
  `hello`, once per document.
- Validates every later envelope with the browser host's own `Host`.
- Starts a new random session on each page load.

When the engine reports `ready`, the runner sends `open` as a guest with no
checkpoint.

On the engine side, `EngineEndpoint` (the public Wasm SDK every release
loads, `crates/aethyrforge-host/src/browser.rs`) uses the native channel when
the page is top-level and `window.aethyrHost` has `postMessage`. It sends the
bootstrap on that channel, accepts `hello` as its first frame, and answers
on the same channel. A page inside a frame keeps the `postMessage` transport.

### What still has to happen in the engine releases

The pinned releases (`z12580…`, `u12583…`) load an SDK built before this
change. They expect a parent frame and announce only to
`window.parent`. In the app they load and render, but no bridge session
starts. The follow-up for #12588 and #12590 is to build the next Haunted
House and universe releases against this repository's `aethyrforge-host`
(`scripts/build-aethyrforge-host.sh`), which needs no engine code change,
then update the pins in `crates/openagents-web/content/demos/*.json`. The
app and `/v1/zones` pick the new pins up with no app change.

A release page that reads its host origin from the embedding page also needs
a default for the top-level case. `EngineEndpoint::new` still requires a
valid HTTPS host origin different from its own, for example
`https://openagents.com`, even when it then uses the native channel.

## Verification

- `cargo test -p aethyrforge-host`: native transport (handshake, open,
  replay, wrong origin and subframe, size bounds, reload sessions) and the
  zone catalog (pin parsing, bounds, Kiki entries, exact-origin navigation).
  `cargo check -p aethyrforge-host --target wasm32-unknown-unknown` covers
  the engine-side channel.
- `cargo test -p openagents-mobile --lib zones` in `apps/openagents`: the
  bundled catalog equals the website's pins, launcher cards and routing, the
  Grid having no web runner, origin checks, the WebGPU gate and its cards,
  failure and retry, and a full bridge handshake against the real `Engine`.
- `cargo test -p openagents-web zones`: the endpoint lists the pinned
  releases and is in the site's owned routes.

## Not done yet

- WebGPU adapter records from real devices. The issue asks for the adapter
  and the iOS and Android versions where the Haunted House opens. So far
  there is one record, from a simulator, which doesn't qualify a device:
  iOS 26.5 Simulator (iPhone 17 Pro) on 2026-10-24. `navigator.gpu`
  exists there, but `requestAdapter()` returns null (`no-adapter`), so the
  runner correctly shows "Open in browser".
- Native sign-in and payment sheets driven from bridge requests. The bridge
  carries the messages, and the broker that turns `auth_required` into the
  native sign-in sheet and issues grants is the next step.
- The native runner (`CAMetalLayer` or `SurfaceView` on `af-gpu`), later,
  per ADR 0010.
