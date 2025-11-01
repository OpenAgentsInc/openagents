# ADR 0006 — Component Organization for UI Primitives and Domain Renderers

 - Date: 2025-11-01
 - Status: Proposed — Structure defined; no moves yet

## Context

We are adopting the common UI components from the Ignite boilerplate (AutoImage, Button, Card, Checkbox/Radio/Switch, EmptyState, Header, Icon, Screen, Text, TextField) as a lightweight design system to complement our existing domain renderers.

Today our components live under several folders:
- `expo/components/acp/*` — typed ACP renderers for streaming session updates
- `expo/components/jsonl/*` — JSONL/Tinyvex cards used by ACP renderers and Library
- `expo/components/*` — app scaffolding (e.g., `app-header.tsx`, `composer.tsx`, `code-block.tsx`, `inline-toast.tsx`, `toast-overlay.tsx`)
- `expo/components/drawer/*`, `expo/components/projects/*` — feature/domain‑specific
- `expo/components/ui/collapsible.tsx` — an initial primitive

Ignite organizes its reusable primitives under `app/components/` with related items grouped in subfolders (e.g., `Toggle/{Checkbox,Switch,Radio}.tsx`) and provides a generator template (`PizzaApp3/ignite/templates/component/NAME.tsx.ejs`). Our repo policy prefers kebab‑case filenames and TypeScript strict mode, and we already have dark‑theme tokens in `expo/constants/theme.ts` and typography defaults in `expo/constants/typography.ts`.

We need a clear place to add the Ignite components without colliding with domain renderers and while following our naming/theming conventions. We will not move existing components yet; this ADR defines structure, conventions, and a migration path.

## Decision

Establish `expo/components/ui/` as the single home for reusable UI primitives (Ignite‑style components), with related items grouped in subfolders. Keep domain‑specific renderers under their existing folders. Adopt kebab‑case filenames and named exports for new UI primitives.

Proposed folder layout (additive; no moves yet):
- `expo/components/ui/`
  - Primitives (files, kebab‑case):
    - `auto-image.tsx`
    - `button.tsx`
    - `card.tsx`
    - `empty-state.tsx`
    - `icon.tsx`
    - `list-item.tsx` (Ignite has `ListItem` used by Card; we include it as a primitive)
    - `screen.tsx` (safe area + keyboard avoiding wrapper)
    - `text.tsx`
    - `text-field.tsx`
    - `collapsible.tsx` (already present)
  - Grouped primitives:
    - `toggle/checkbox.tsx`
    - `toggle/radio.tsx`
    - `toggle/switch.tsx`
  - Barrels (optional, for ergonomics):
    - `index.ts` re‑exports public primitives

Domain and app‑specific components remain in place:
- `expo/components/acp/*` — unchanged (typed ACP renderers)
- `expo/components/jsonl/*` — unchanged (cards/primitives used by ACP)
- `expo/components/drawer/*`, `expo/components/projects/*` — unchanged
- `expo/components/app-header.tsx`, `expo/components/composer.tsx`, etc. — app shell components (remain where they are for now)

API & conventions for new UI primitives:
- Filenames: kebab‑case; exports: named PascalCase components (e.g., `export function Button()` in `button.tsx`).
- Theming: use `Colors` (tokens) and `Typography` from `expo/constants` instead of introducing a theme provider. Do not hardcode colors; add tokens when needed.
- E2E: accept and pass through `testID` and accessibility props (aligns with ADR‑0004 Maestro testing).
- i18n: initially support plain `text` props; we may add Ignite‑style `tx`/`txOptions` later. Do not block adoption on i18n wiring.
- RN collisions: avoid using React Native’s built‑in `Button`. Our `Button` should be imported from `@/components/ui/button` (or barrel). Prefer `Pressable` internally.
- Storybook: add stories for new primitives per ADR‑0005.

## Rationale

- Clear separation of concerns:
  - `ui/` is the reusable, app‑agnostic design layer (Ignite primitives).
  - `acp/` and `jsonl/` are domain renderers bound to Tinyvex/ACP types.
  - App shell components (header, composer) stay distinct from both.
- Consistency with our repo conventions (kebab‑case files, strict TS, dark theme tokens) while retaining the essence of Ignite’s component set.
- Easier discoverability and reuse; future contributors know where primitives live.
- Smooth path to Storybook (ADR‑0005) with a single place to author stories.

## Alternatives Considered

1) Keep Ignite components at the top level of `expo/components/`
   - Con: muddles primitives with domain renderers and app shell; harder to navigate.

2) Introduce `expo/components/ignite/*`
   - Con: leans on external naming and encourages copy‑pasting rather than integrating with our tokens/conventions.

3) Adopt a third‑party UI kit instead of Ignite primitives
   - Con: heavier theming/runtime cost and potential Expo SDK mismatches; Ignite’s primitives are simpler and align with our custom design.

## Consequences

- Positive: predictable, scalable component organization; reusable primitives in one place; easier onboarding and Storybook coverage.
- Neutral: minor overhead to adapt Ignite examples to our tokens and file naming; `Header` remains app‑specific (`app-header.tsx`) for now.
- Risk: file‑name style divergence (some legacy PascalCase files under `jsonl/`). Mitigation: enforce kebab‑case for new `ui/` files and normalize incrementally when touching legacy files (no mass renames).

## Migration Plan (No moves yet)

Phase 0 — This ADR
- Create `expo/components/ui/` (exists) and use it for all new primitives. Do not move existing files.

Phase 1 — Add Ignite primitives (as needed)
- Implement primitives in `ui/` using `Colors`/`Typography` and Expo‑compatible APIs:
  - `auto-image`, `button`, `card`, `empty-state`, `icon`, `list-item`, `screen`, `text`, `text-field`, `toggle/{checkbox,radio,switch}`.
- Add Storybook stories for each (ADR‑0005), wire `testID` passthrough for E2E.

Phase 2 — Optional convergence
- As we touch app surfaces, opportunistically migrate to `ui/` primitives.
- Evaluate whether to keep `AppHeader` as the sole header or introduce a generic `ui/header` for non‑app contexts. Avoid breaking changes in app routes.
- Normalize legacy PascalCase filenames during nearby changes (no broad refactors).

## Acceptance Criteria

- New reusable UI components live under `expo/components/ui/` using kebab‑case filenames and named exports.
- Domain renderers remain in `acp/` and `jsonl/` with no structural changes now.
- New primitives use `Colors` and `Typography`; no hardcoded colors.
- Storybook stories are added for new primitives as they are introduced.
- E2E selectors (`testID`) are supported on primitives.

## References

- Ignite components overview: `https://github.com/infinitered/ignite/blob/master/docs/boilerplate/app/components/Components.md`
- Ignite example app components: `/Users/christopherdavid/code/PizzaApp3/app/components/`
- Ignite component template: `/Users/christopherdavid/code/PizzaApp3/ignite/templates/component/NAME.tsx.ejs`
- Our theme/typography: `expo/constants/theme.ts`, `expo/constants/typography.ts`
- Existing domains: `expo/components/acp/*`, `expo/components/jsonl/*`
- ADR 0004 — Maestro E2E Testing; ADR 0005 — Storybook React Native

