# Verse Packaged Launch Smoke Proof - 2026-06-20

Issue: #5827

## Scope

This receipt proves the Autopilot Desktop dev package opens to the Verse
first-paint contract: Pylon/Tassadar/chat first, nonblank 3D scene, packaged
Pylon node present, and no Codex/Claude/session chrome on the launch viewport.

This is a render/proof receipt, not a claim that the full Desktop `tsc` debt is
closed. Keep the #5556 caveat until that issue is explicitly resolved.

## Command

```sh
cd apps/autopilot-desktop
bun run build
bun run smoke:verse-launch
```

`bun run smoke:verse-launch` serves the built Electrobun `.app` view bundle from:

- `build/dev-macos-arm64/Autopilot-dev.app/Contents/Resources/app/views/autopilot-desktop/index.html`
- `build/dev-macos-arm64/Autopilot-dev.app/Contents/Resources/app/views/autopilot-desktop/styles.css`
- `build/dev-macos-arm64/Autopilot-dev.app/Contents/Resources/app/views/autopilot-desktop/main.js`
- `build/dev-macos-arm64/Autopilot-dev.app/Contents/Resources/app/pylon-node/index.js`

The smoke only shims the Electrobun bridge globals so the packaged webview can
run in headless Chrome without a native window.

## Receipt

Captured at: `2026-06-20T20:04:49.741Z`

- Result: `ok`
- Viewport: `1280x800`
- Canvas: `1280x800`, full viewport
- Screenshot: `1280x800`
- Screenshot SHA-256: `a6f220221f18c9466d298335cc21cbd39b0c11a34d61a0c49936ff601f1a7292`
- Bright pixels: `74737 / 1024000`
- Distinct luma buckets: `24`
- Forbidden first-paint text hits: none
- Overlap pairs: none

DOM checks:

- `appShellVerse`: true
- `chatPaneWorld`: true
- `chatInput`: true
- `pylonBaseStatus`: true
- `characterCreation`: true
- `trainingCanvas`: true
- `advancedButton`: true
- `noAdvancedChrome`: true
- `noPanelOverlap`: true

Visible text sample:

```text
Advanced My Pylon Base 0/5 Choose or create a Pylon identity · 0 sats character creation 0 pylons online Pylon online Waiting for the local Pylon to come online. Agent spawned Agent registration unlocks the spawn beat. Customize Choose the agent identity. Forum intro Waiting for registered presence before preparing the
```

Generated local artifacts are written under the ignored build directory:

- `apps/autopilot-desktop/build/verse-launch-smoke/verse-launch-smoke.json`
- `apps/autopilot-desktop/build/verse-launch-smoke/verse-launch-smoke.png`

