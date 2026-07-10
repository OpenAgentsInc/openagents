# OpenAgents mobile — v0.5.1 build 108: surface-mode dropdown + Sarah video surface (GL-2 #8648)

Owner iteration on the glass shell ("looks good" + follow-ups): the pill is
now a DROPDOWN, "Sarah" mode plays a fullscreen demo video under the glass,
and the main surface is stripped of all status text.

## What shipped

- **Pill dropdown** (`GlassPill` + SwiftUI `Menu`, iOS 26 glass): options
  `OpenAgents` (default) / `Sarah`, checkmark on the selected mode, chevron
  affordance. Selection dispatches the typed `SurfaceModeSelected` intent
  (`program.chrome.selectSurfaceMode`) — never a callback. Non-iOS fallback:
  tapping the pill cycles the mode.
- **Sarah surface** — `expo-video` (bundled `assets/videos/sarah-demo.mp4`,
  1.75 MB): fullscreen cover-fit, looping, muted, UNDER every glass layer.
  Fade-in on first-frame-ready (`statusChange === "readyToPlay"`): opacity
  0 -> 1 over 700ms ease-out with a subtle 1.03 -> 1.0 scale settle; black
  underneath until ready — no pop, no flash. Selecting OpenAgents returns to
  the Protoss-black surface (video paused, fade state reset).
- **Clean main surface** (owner direction): the content projection renders
  NO text — in `openagents` mode it is the opaque background; in `sarah`
  mode it is transparent so the video shows through. Conversation content
  mounts here when the Sarah surface lands. Selection state lives in the ONE
  EN program (typed intent -> state -> pill label + background swap + drawer
  all re-render from the same `SubscriptionRef`).

## Simulator pixel proofs (upload gate; committed under receipts/)

1. `2026-07-09-build108-clean-shell.png` — no-touch first launch: black
   surface, glass chrome only, pill reads "OpenAgents" with chevron. (Also
   the spurious-event watch: NO self-dispatched intents this launch.)
2. `2026-07-09-build108-dropdown-open.png` — pill tap opens the native glass
   menu: checkmark on OpenAgents, Sarah below.
3. `2026-07-09-build108-sarah-video.png` — after selecting Sarah: fullscreen
   video visibly playing under the glass chrome; pill reads "Sarah".
4. `2026-07-09-build108-back-to-black.png` — selecting OpenAgents returns to
   the black surface; pill reads "OpenAgents".

Honesty note: the first-launch-after-install-over-running-app spurious-event
anomaly recurred ONCE on the pre-cleanup build (a phantom `onSelect` flipped
the mode before any touch), and did NOT occur on the clean build's no-touch
first launch above. Still watched-for; if TestFlight (clean install) shows
it, it gets a dedicated lane.

## Tests

20 pass (surface-mode projection + typed round-trip through the real
render-rn renderer; content-surface cleanliness oracle per the owner
direction; drawer/identity/OTA/catalog oracles green). Typecheck clean.

## Release receipt

Version `0.5.1`, iOS build `108`, BUNDLE_TAG `2026-07-09.embedded-108`.
Upload/VALID + OTA reseed recorded on #8648.
