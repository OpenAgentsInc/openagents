# ADR 0006 — Component Organization for Primitives, Domain Renderers, and App Shell

 - Date: 2025-11-01
 - Status: Proposed — Structure defined; no moves yet

## Context

We need a predictable, testable, and story‑friendly component organization that works hand‑in‑hand with:
- ADR‑0004 (Maestro E2E) — stable `testID`/accessibility anchors and clear boundaries for flow tests.
- ADR‑0005 (Storybook RN) — a single, discoverable place to author stories for reusable UI.

The current codebase mixes reusable UI, domain renderers, and app shell helpers under `expo/components/`. Without a clear structure, stories get scattered and E2E selectors are inconsistent. We will standardize folder roles, naming, and conventions to improve reuse, testing, and documentation.

Existing folders and roles today:
- `expo/components/acp/*` — typed renderers for ACP session updates (domain‑specific).
- `expo/components/jsonl/*` — JSONL/Tinyvex cards consumed by ACP and Library (domain‑specific).
- `expo/components/*` — app shell pieces such as `app-header.tsx`, `composer.tsx`, `code-block.tsx`, `inline-toast.tsx`, `toast-overlay.tsx`.
- `expo/components/ui/collapsible.tsx` — an initial primitive.

## Decision

Define a three‑layer component organization and conventions that support Storybook and Maestro:

1) UI Primitives — reusable, app‑agnostic components
   - Location: `expo/components/ui/`
   - Contents: buttons, text, inputs, cards, images, list items, toggles, layout helpers (including existing `collapsible.tsx`).
   - Group related items in subfolders (e.g., `toggle/{checkbox,radio,switch}.tsx`).
   - Filenames: kebab‑case; exports: named PascalCase components.
   - Theming/typography: use `expo/constants/theme.ts` and `expo/constants/typography.ts`; no separate theme provider.
   - Testing: accept and pass through `testID`/accessibility props (ADR‑0004).
   - Storybook: each primitive has a story (ADR‑0005).

2) Domain Renderers — data‑typed views for ACP/Tinyvex
   - Locations: `expo/components/acp/*`, `expo/components/jsonl/*` (unchanged).
   - Purpose: translate typed events/rows into UI; compose primitives from `ui/` where appropriate.
   - Filenames: keep existing; prefer kebab‑case for any new files.
   - Testing: expose stable anchors for Maestro where flows need to assert domain UI.
   - Storybook: add stories for representative states, colocated under Storybook folder per ADR‑0005.

3) App Shell — navigation/header/composer and feature shells
   - Locations: `expo/components/app-header.tsx`, `expo/components/composer.tsx`, `expo/components/drawer/*`, `expo/components/projects/*` (unchanged).
   - Purpose: wire navigation, header chrome, and screen‑level composition.
   - Testing: include durable `testID`s required by Maestro flows (`header-menu-button`, `header-connection-indicator`, etc.).
   - Storybook: optional stories if useful; not required for all shell pieces.

Barrels (optional):
- `expo/components/ui/index.ts` may re‑export primitives for ergonomics. Avoid deep barrels in domain/app layers to keep import paths explicit.

## Rationale

- Testability (ADR‑0004): clear separation makes it easy to attach stable selectors to shells and verify domain output without coupling to primitive internals.
- Storybook (ADR‑0005): a single `ui/` home for primitives provides a natural catalog, while domain renderers can showcase data‑typed states separately.
- Consistency: kebab‑case files, strict TypeScript, centralized tokens for dark theme and typography; no ad‑hoc theming.
- Maintainability: contributors quickly find primitives vs. domain vs. shell; encourages reuse and reduces duplication.

## Alternatives Considered

1) Flat `expo/components/` without sub‑domains
   - Con: primitives, domain, and shell mix together; stories and tests become hard to navigate.

2) Vendor‑specific folders (e.g., `ignite/*`) for primitives
   - Con: leaks external nomenclature and discourages integration with our tokens and ADR‑driven process.

3) Adopt a heavy UI kit for all primitives
   - Con: increases runtime/theming complexity and may not align with Expo SDK versions; our needs are modest and custom.

## Consequences

- Positive: predictable imports and discoverability; Storybook catalogs primitives cleanly; Maestro flows target stable anchors.
- Neutral: some legacy files are PascalCase; we will adopt kebab‑case for new files and normalize opportunistically (no mass renames).
- Operational: no REST or bridge changes; this is an app‑level organization decision.

## Migration Plan (No moves yet)

Phase 0 — Adopt structure (this ADR)
- Use `expo/components/ui/` for any new reusable primitives.

Phase 1 — Fill gaps incrementally
- As we add primitives (e.g., button, text, text‑field, card, list‑item, icon, auto‑image, toggles, screen), place them in `ui/` and author Storybook stories.
- Ensure primitives accept `testID` and accessibility props; keep them theme‑token based.

Phase 2 — Opportunistic convergence
- When modifying domain or shell code, prefer composing from `ui/` primitives.
- Normalize filenames to kebab‑case when touching files; avoid broad refactors.

## Acceptance Criteria

- New reusable UI components live under `expo/components/ui/` with kebab‑case filenames and named exports.
- Domain renderers remain under `acp/` and `jsonl/`; shells remain in their current locations.
- Primitives use `Colors` and `Typography` tokens; no hardcoded colors.
- Primitives and key shell components surface stable `testID`s for Maestro.
- Stories exist (or are added) for primitives per ADR‑0005.

## References

- ADR‑0004 — Maestro E2E Testing
- ADR‑0005 — Storybook React Native
- Theme/typography tokens: `expo/constants/theme.ts`, `expo/constants/typography.ts`
- Existing domains: `expo/components/acp/*`, `expo/components/jsonl/*`
