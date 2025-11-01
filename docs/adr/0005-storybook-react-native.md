# ADR 0005 — Adopt Storybook (v10) for React Native Component Library

 - Date: 2025-11-01
 - Status: Accepted

## Context

We currently ship a custom, in‑app component library under `expo/app/library/*` that doubles as a demo surface for many UI elements (ACP renderers, JSONL cards, primitives). While useful, this approach has drawbacks:

- Discoverability/scale: Adding, organizing, and documenting variants is manual and scattered.
- Drift: Demos and production UI can diverge; there’s no single source of truth for states/knobs.
- Testing: Our E2E path (Maestro) benefits from stable, id‑driven surfaces; a proper component‑driven setup improves coverage and DX.

Storybook v10 for React Native provides a focused, component‑driven workflow, integrates with Expo/Metro, supports on‑device Storybook UI, and aligns with modern CSF (Component Story Format).

## Decision

Adopt Storybook v10 (React Native) as the single component library surface for the mobile app. Consolidate the existing custom library into Storybook stories and remove the legacy library screens after parity.

Provide a simple runtime toggle: if `USE_STORYBOOK` is true (boolean flag or env var), the app boots Storybook instead of the normal app shell; otherwise it boots the app.

## Rationale

- Component‑driven development (CDD) improves discoverability, state coverage, and docs.
- Storybook’s CSF aligns well with TypeScript and encourages well‑scoped, testable states.
- On‑device Storybook UI is ideal for React Native; we can keep Expo/Metro dev flow intact.
- Parity and eventual removal of the legacy in‑app library reduces maintenance.

## Implementation Plan

1) Bootstrap Storybook (v10, React Native)
   - Add `@storybook/react-native` and required deps (per Storybook RN v10 docs).
   - Create `storybook/` folder with:
     - `index.ts` (register stories, configure addons).
     - `stories/**/*/*.stories.tsx` for components.
   - Add `StorybookUIRoot` via `getStorybookUI`.

2) Startup toggle (`USE_STORYBOOK`)
   - In app entry (e.g., `expo/index.ts` or the root used by Expo Router), import either `StorybookUIRoot` or the normal app root based on a boolean:
     - Code constant (temporary) and/or env var (`process.env.USE_STORYBOOK === '1'`).
   - For DX, also add a dev menu item or a separate NPM/Bun script to launch Storybook mode.

3) Consolidation strategy
   - Migrate existing demo screens under `expo/app/library/*` into Storybook stories:
     - ACP components under `expo/components/acp/*` (message, thought, tool call, plan).
     - JSONL cards and primitives used by ACP renderers.
     - App atomics (colors, typography scales) as reference stories.
   - Use CSF (v3) with small, state‑rich stories (props and common permutations).
   - After parity (same visual states covered), remove the legacy `expo/app/library/*` routes.

4) Developer experience
   - Add scripts:
     - `bun run storybook` — start Metro in Storybook mode (or reuse Expo dev-client with `USE_STORYBOOK=1`).
     - `bun run storybook:ios` / `:android` — convenience scripts to open the platform simulator in Storybook mode.
   - Document local steps in `docs/app/` or a Storybook README.

5) Testing & CI
   - Maestro: optional flows can open Storybook mode (when `USE_STORYBOOK=1`) to assert critical component states (smoke checks, visual anchors with `testID`).
   - Keep Storybook out of the stable E2E lane by default (no regressions if Metro is not warmed for Storybook); provide an opt‑in job.

## Alternatives Considered

- Keep legacy, in‑app library: higher maintenance, less scalable, limited controls.
- Build custom showcase tooling: replicates Storybook features; not worth the complexity.

## Consequences

- Positive: Stronger CDD, better DX, single place for demo/variants; easier Maestro smoke checks in isolation; simpler onboarding.
- Neutral: Slightly more setup (storybook folder, scripts) and small runtime toggle logic.
- Deprecation: Legacy `expo/app/library/*` will be removed once parity is reached.

## Risks & Mitigations

- Metro/Expo integration quirks for RN Storybook: follow RN v10 setup; keep a minimal entry and avoid conflicting Metro configs.
- Binary size in prod: exclude Storybook from production builds (guard behind dev flag and build‑time env); ensure `USE_STORYBOOK` is false by default.
- E2E variability: ensure Storybook flows are optional and not blocking stable CI; keep them as additional smoke checks.

## Acceptance Criteria

- `USE_STORYBOOK=1` boots StorybookUIRoot; default boots normal app.
- Stories exist for ACP components and primary UI primitives.
- Initial coverage implemented: ACP Agent Message, Agent Thought, Tool Call (execute/edit), Plan, Available Commands, Current Mode, and Example Conversation.
- Legacy `expo/app/library/*` removed after parity.
- Optional Maestro flows can target Storybook controls/states when enabled.

## References

- Storybook React Native v10 docs: https://storybookjs.github.io/react-native/docs/intro/
- Intro to Storybook (React Native): https://storybook.js.org/tutorials/intro-to-storybook/react-native/en/get-started/
- Component‑Driven UIs: https://www.componentdriven.org/
