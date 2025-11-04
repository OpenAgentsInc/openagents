# ADR 0008 — Shared Cross‑Platform Theme and Headless UI Strategy

 - Date: 2025-11-02
 - Status: Accepted — Updated (shared tokens implemented; desktop UI to load Expo Web in Tauri; Swift app maps tokens)

## Context

We now have two UI surfaces:
- Mobile: Expo/React Native (Router, RN components, `expo-font` for Berkeley Mono)
- Desktop: Tauri (WebView + React DOM)

 Both experiences should present the same visual language (dark theme, colors, typography). Until now, tokens and font wiring were duplicated:
- Expo defined `Colors` and `Typography` under `expo/constants/*` (with font loading and RN defaults)
- Tauri inlined CSS variables and `@font-face`
 - Swift app was separate; now it maps the same tokens via a thin `OATheme` shim.

We also discussed deeper sharing approaches: using React Native Web (RNW) inside Tauri or loading Expo Web into Tauri. Those options increase reuse but require bundling and platform‑API integration work.

## Decision

Adopt a staged approach that starts with shared tokens and a headless core. Update (2025‑11‑03): un‑defer the desktop UI path by loading the Expo app’s Web build inside Tauri as the desktop renderer (dev and prod). Cross‑platform component rendering via RNW is achieved through Expo Web; a direct “RNW without Expo” path remains optional for later.

- Create a dedicated workspace package `@openagentsinc/theme` that exports:
  - `Colors` (typed, constant tokens) — identical across platforms
  - `Typography` (family names) — shared identifiers; platform loaders remain app‑local
  - `web/theme.css` — web `:root` variables for Tauri/DOM
- Migrate Expo and Tauri to import these shared tokens:
  - Expo continues to load fonts via `expo-font` and applies RN defaults
  - Tauri imports the web CSS and retains `@font-face` entries locally (for now)
  - Swift (iOS/macOS) bundles Berkeley Mono TTFs and applies a global default (`.environment(\.font, ...)`), with a small `OATheme.Colors` shim mirroring `openagents-theme/colors`.
- Prefer a headless core for logic/state/renderless concerns next, enabling high reuse without forcing RNW immediately.
- Track a medium‑term RNW proof‑of‑concept to evaluate further UI sharing in Tauri.

Desktop Strategy Update (2025‑11‑03)

- Renderer: Use the Expo app’s Web build as the Tauri window content.
  - Development: point Tauri `devUrl` to the Expo Web dev server (e.g., `http://localhost:19006`) and run `expo start --web` as the pre‑dev command.
  - Production: bundle the Expo static web export as Tauri’s `frontendDist` (e.g., `expo export -p web` to `expo/web-dist`).
- No custom RNW shims in desktop: remove reliance on placeholder RN shims in `tauri/` in favor of rendering the actual Expo RN component tree through Expo Web (RNW under the hood).
- Feature detection: call desktop‑only functionality via Tauri APIs behind a small `Desktop.isTauri`/`invoke` helper in the Expo app.
- Security/routing:
  - Dev CSP allows the Expo dev server origins (http/ws) only.
  - Prod CSP is restrictive (no `localhost`/`ws`).
  - Ensure SPA history fallback (hash routing or `200.html`).
  - Keep service workers disabled for the bundled desktop asset protocol.

## Rationale

- Immediate alignment: unify colors and typography names with minimal risk to either platform.
- Low integration cost: no bundler gymnastics or polyfills; each surface stays idiomatic (RN vs Web).
- Clear path forward: headless + tokens now; optional RNW POC later if it proves beneficial.
- Desktop update benefit: Expo Web already renders RN components via RNW and preserves Expo Router and SDKs, minimizing desktop‑specific code and avoiding a second React app.

## Alternatives Considered

1) React Native Web in Tauri
   - Pros: maximum component reuse
   - Cons: bundling/aliasing complexity; API parity gaps; requires shims for Tauri APIs or a parallel web app; duplicates routing and assets if not using Expo Web

2) Load Expo Web inside Tauri
   - Pros: reuse the full app (routing + components); lowest integration risk; single source of truth for RN components
   - Cons: some runtime overhead; ensure web fallbacks for any RN‑only modules; CSP and routing must be configured correctly in Tauri

3) Keep duplication
   - Pros: simplest immediately
   - Cons: guaranteed drift; higher maintenance cost

## Consequences

- Pros
  - Single source of truth for tokens; consistent dark theme and typography names
  - Minimal runtime risk; easy rollback
  - Enables incremental migration of shared logic to a headless package
  - Desktop renders the real Expo RN component tree (via RNW) with no shim placeholders; reduces drift and duplication
- Cons
  - Fonts still duplicated per app until licensing/packaging is finalized
  - Presentational components remain RN on mobile and RNW on desktop; any DOM‑only components should be minimized
  - Tauri must be configured to proxy dev to Expo Web and bundle the exported static assets for production

## Acceptance

- `packages/@openagentsinc/theme` exists with `colors`, `typography`, and `web/theme.css`.
 - Expo imports `Colors`/`Typography` from the package; font loading remains via `expo-font`.
 - Tauri imports `web/theme.css` and uses Berkeley Mono everywhere via local `@font-face`.
 - SwiftUI app defines `OATheme` (colors, selection tint) based on `packages/openagents-theme/colors.js` values and uses Berkeley Mono globally.
- Desktop: Tauri `devUrl` points to Expo Web dev server; `beforeDevCommand` launches `expo start --web` in `expo/`.
- Desktop: `frontendDist` points to the Expo static web export directory; `beforeBuildCommand` runs the export.
- CSP and routing are configured per environment; service workers disabled in the desktop bundle.
- The prior Tauri RNW shims are deprecated/removed once the Expo Web path is stable.
- PRs reference this ADR and Issue #1390 for future work.

## References

- Issue #1390 — Shared theming package: packages/openagents-theme (Expo + Tauri)
- PR #1392 — feat(theme): shared @openagentsinc/theme and migrate Expo/Tauri
- ADR 0006 — Component Organization
 - ADR 0009 — Desktop‑Managed Bridge
 - Desktop implementation guide (internal issue to be filed): "Adopt Expo Web inside Tauri"
