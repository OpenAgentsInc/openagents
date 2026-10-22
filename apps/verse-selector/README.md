# Verse selector preview

The browser island for `/new-lander/verse`: an original direct-WebGPU map with lit planets, orbital groups, destination selection, drag/keyboard orbit, zoom, and a still-image fallback. Effect scopes the GPU lifetime. No live network traffic, user planet ownership, or population counts are simulated.

The marketing homepage and preview routes are Rust/Maud in `crates/openagents-web/src/pages/new_lander.rs`. The current `/` route stays unchanged. Neither preview route is indexed. The marketing page has no script or game startup.

## Build

```sh
cd apps/verse-selector
npm ci
npm run build
npm test
```

The build writes `crates/openagents-web/src/pages/new_lander.js`, which is checked in and embedded by Rust. Keep that generated file with its source changes. The production Rust image needs no Node installation or extra assets to serve this preview.

Run the existing `openagents-web` server with scratch storage, then open `/new-lander`. The selector links to the existing Blue Rush page. It exposes Haunted House entry only when the current server has a house build or hosted tenant. The central company planet and future destinations stay locked. When the account service is configured, the login card uses the existing login route and returns to the selector; this preview does not implement a second account system.

This slice does not implement the full voice-led tutorial, publishing, or personal planet award. Their design remains in the [landing assessment](../../docs/audits/2026-10-22-new-lander-stack-and-loading.md).

## Browser verification

```sh
NEW_LANDER_URL=http://127.0.0.1:PORT \
NEW_LANDER_EVIDENCE="$(openagents scratch)/new-lander-browser" \
openagents browser run -- node apps/verse-selector/scripts/verify-browser.mjs
```

The check uses an isolated Chrome profile. It verifies real Rust routes, rendered GPU pixels, selection, camera controls, root preservation, mobile layout, still preview, and script-free marketing. Account sign-in and existing game services need separate verification with dedicated test accounts.
