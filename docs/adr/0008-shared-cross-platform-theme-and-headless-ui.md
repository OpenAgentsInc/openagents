# ADR 0008 — Shared Cross‑Platform Theme and Headless UI Strategy

 - Date: 2025-11-02
 - Status: Accepted — Implemented (tokens package scaffolded; apps migrated)

## Context

We now have two UI surfaces:
- Mobile: Expo/React Native (Router, RN components, `expo-font` for Berkeley Mono)
- Desktop: Tauri (WebView + React DOM)

Both experiences should present the same visual language (dark theme, colors, typography). Until now, tokens and font wiring were duplicated:
- Expo defined `Colors` and `Typography` under `expo/constants/*` (with font loading and RN defaults)
- Tauri inlined CSS variables and `@font-face`

We also discussed deeper sharing approaches: using React Native Web (RNW) inside Tauri or loading Expo Web into Tauri. Those options increase reuse but require bundling and platform‑API integration work.

## Decision

Adopt a staged approach that starts with shared tokens and a headless core, deferring cross‑platform component rendering until later:

- Create a dedicated workspace package `@openagents/theme` that exports:
  - `Colors` (typed, constant tokens) — identical across platforms
  - `Typography` (family names) — shared identifiers; platform loaders remain app‑local
  - `web/theme.css` — web `:root` variables for Tauri/DOM
- Migrate Expo and Tauri to import these shared tokens:
  - Expo continues to load fonts via `expo-font` and applies RN defaults
  - Tauri imports the web CSS and retains `@font-face` entries locally (for now)
- Prefer a headless core for logic/state/renderless concerns next, enabling high reuse without forcing RNW immediately.
- Track a medium‑term RNW proof‑of‑concept to evaluate further UI sharing in Tauri.

## Rationale

- Immediate alignment: unify colors and typography names with minimal risk to either platform.
- Low integration cost: no bundler gymnastics or polyfills; each surface stays idiomatic (RN vs Web).
- Clear path forward: headless + tokens now; optional RNW POC later if it proves beneficial.

## Alternatives Considered

1) React Native Web in Tauri
   - Pros: maximum component reuse
   - Cons: bundling/aliasing complexity; API parity gaps; requires shims for Tauri APIs

2) Load Expo Web inside Tauri
   - Pros: reuse the full app (routing + components)
   - Cons: heavier pipeline, interop for desktop features, larger payloads

3) Keep duplication
   - Pros: simplest immediately
   - Cons: guaranteed drift; higher maintenance cost

## Consequences

- Pros
  - Single source of truth for tokens; consistent dark theme and typography names
  - Minimal runtime risk; easy rollback
  - Enables incremental migration of shared logic to a headless package
- Cons
  - Fonts still duplicated per app until licensing/packaging is finalized
  - Presentational components are still split (RN vs DOM) pending any RNW adoption

## Acceptance

- `packages/@openagents/theme` exists with `colors`, `typography`, and `web/theme.css`
- Expo imports `Colors`/`Typography` from the package; font loading remains via `expo-font`
- Tauri imports `web/theme.css` and uses Berkeley Mono everywhere via local `@font-face`
- PRs reference this ADR and Issue #1390 for future work

## References

- Issue #1390 — Shared theming package: packages/openagents-theme (Expo + Tauri)
- PR #1392 — feat(theme): shared @openagents/theme and migrate Expo/Tauri
- ADR 0006 — Component Organization
