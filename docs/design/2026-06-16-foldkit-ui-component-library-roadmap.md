# Foldkit UI Component Library Roadmap — `@openagentsinc/ui`

Date: 2026-06-16
Status: Build roadmap for a coding agent. Scope: this repo (`openagents`).

## Goal

Turn the existing in-app Foldkit component folder into a shared, reusable,
extractable package — **`@openagentsinc/ui`** — and fill the one big gap (an
AI-native element family) so it can power the Blitz "prefilled workspace" /
accepted-outcome surfaces. This is **extraction + gap-fill, not a greenfield
build**.

Strategy context lives in the workspace repo at
`docs/blitz/design/2026-06-16-tailwind-foldkit-component-port-plan.md` and its
sibling Maud-port audit. This doc is the sequenced build plan.

## Current state (audited 2026-06-16)

- A Foldkit component library already exists at
  `apps/openagents.com/apps/web/src/ui/` — its `README.md` calls it the "Foldkit
  component-library port boundary" for the Tailwind UI v4 kits (`application-ui-v4`,
  `ecommerce-v4`, `marketing-v4`). Family files: `primitives.ts`, `shared.ts`,
  `forms.ts`, `layout.ts`, `navigation.ts`, `data-display.ts`, `feedback.ts`,
  `workroom.ts`, `public.ts`, `page-examples.ts`, `v4.ts`, plus app-specific
  `tenant-theme.ts`, `credits-panel.ts`; barrel `index.ts`; `coverage.test.ts`.
- It is imported as a **relative folder** (`import * as Ui from './ui'` etc.),
  not a package, and a **near-identical copy lives in the separate
  `autopilot-omega` repo** (lineage; out of scope here).
- `packages/autopilot-ui` (`@openagentsinc/autopilot-ui`) is a **separate**
  package of **domain feature views** (node-status, earnings, decisions) + a
  `tokens.ts`. It is *not* the design-system primitives layer.
- **Gap:** there is no AI-native element family (agent, prompt-input, message,
  code-block-with-run, task, sources, tool/agent status, confirmation, reasoning,
  web-preview). That catalog exists as reference in:
  - `autopilot3/src/components/ai-elements/*.tsx` (50 modules) — canonical
    React/Tailwind source (separate clone, reference only).
  - `autopilot4-deprecated/src/ui_components/ai_elements.rs` (48 modules / 250+
    primitives) + `tailwind_contracts.rs` + `base.rs` — the **curation map**
    (which modules, base contracts, exact class strings) (reference only).

## Where it lives

- **New package: `packages/ui`, name `@openagentsinc/ui`.** Top-level `packages/`
  (sibling to `autopilot-ui`) so any app can consume it — apps/openagents.com is
  the main consumer, but autopilot-desktop, pylon, forum, etc. may too ("there
  may be others"). Bun resolves root `packages/*` for the nested web app (already
  true for `@openagentsinc/autopilot-ui`).
- **Mirror the `@openagentsinc/autopilot-ui` conventions exactly:** source-level
  exports (no build step), `exports` map pointing at `./src/*.ts`,
  `"sideEffects": false`, scripts `{ test: "bun test", typecheck: "tsc -p
  tsconfig.json --noEmit" }`, deps `effect: "catalog:"`, `foldkit: "^0.102.1"`,
  `clsx` (used by `primitives.ts`).
- **Package relationship:**
  - `@openagentsinc/ui` = design-system primitives + Tailwind-kit families +
    (new) `ai-elements` family. The port boundary.
  - `@openagentsinc/autopilot-ui` = domain feature views; **make it depend on
    `@openagentsinc/ui`** and stop duplicating primitives.
  - **Tokens:** consolidate to one source of truth in `@openagentsinc/ui`
    (`./tokens`); have `autopilot-ui/tokens` re-export or migrate. Resolve during
    Phase 1.
- Do **not** revive `autopilot4-deprecated` or build in `autopilot-omega` as part
  of this roadmap. Reference-only.

## Phase 0 — Extract (mechanical, zero behavior change)

1. Create `packages/ui/` with `package.json` (`@openagentsinc/ui`, exports map,
   scripts, deps as above) and `tsconfig.json` copied from
   `packages/autopilot-ui/tsconfig.json`.
2. Move the **kit families** from `apps/openagents.com/apps/web/src/ui/` into
   `packages/ui/src/`: `primitives.ts`, `shared.ts`, `forms.ts`, `layout.ts`,
   `navigation.ts`, `data-display.ts`, `feedback.ts`, `workroom.ts`, `public.ts`,
   `page-examples.ts`, `v4.ts`, and the barrel `index.ts`. Move `coverage.test.ts`
   into `packages/ui/test/`.
3. **Keep app-specific files in the app for now:** `tenant-theme.ts`,
   `credits-panel.ts` (and their tests) are product-specific — leave them under
   `apps/openagents.com/apps/web/src/ui/` and have them import shared bits from
   `@openagentsinc/ui`. Re-export from the app barrel so existing call sites keep
   working.
4. Add `"@openagentsinc/ui": "workspace:*"` to
   `apps/openagents.com/apps/web/package.json` (the web app's package.json).
5. Migrate imports: replace relative `./ui` / `../ui` / `../../../ui` imports of
   the moved families with `@openagentsinc/ui`. Simplest safe path: keep
   `apps/openagents.com/apps/web/src/ui/index.ts` as a **thin re-export shim**
   (`export * from '@openagentsinc/ui'` + the two app-local modules) so most call
   sites are untouched; migrate call sites later if desired.
6. Run `bun test` + `tsc --noEmit` for the package and the web app. **No visual
   or behavioral change.**

**DoD:** `@openagentsinc/ui` is importable as a workspace package; web app builds,
typechecks, and all tests pass; `coverage.test.ts` runs inside the package; no
rendered-output change.

## Phase 1 — Tokens + trusted-selection scaffolding

1. **Tokens source of truth:** move/define design tokens in
   `@openagentsinc/ui` (`./tokens`); reconcile with
   `@openagentsinc/autopilot-ui/tokens` (re-export or migrate) so there is one
   token contract. Make `autopilot-ui` depend on `@openagentsinc/ui`.
2. **Base-contract metadata:** add a convention (mirroring
   `autopilot4-deprecated/src/ui_components/base.rs`) where every component
   records the design-system primitive it derives from (e.g. a `data-ui-base`
   attribute / a registry map like `shad-ui:button/Button`,
   `ai-elements:task/Task`) or is a documented exception.
3. **Typed, fail-closed selection:** add an Effect `Schema` union of the
   component types program/schema output may select, with a renderer that **fails
   closed on unknown types** (mirroring
   `autopilot4-deprecated/src/ui_descriptors.rs`). This is the key safety property:
   programs select trusted component type + trusted props, never raw `Html`.

**DoD:** single token source; every exported component carries a base tag or
documented exception; a `renderDescriptor`-style typed selector exists with a
fail-closed test.

## Phase 2 — AI Elements family (the gap; Blitz-critical)

Add `packages/ui/src/ai-elements/` (and a barrel). Port from the autopilot3 TSX
**markup/Tailwind**, using the Maud `ai_elements.rs` as the **curation map**
(module list, coverage counts, base contracts) and `tailwind_contracts.rs` to
cross-check class strings. Do **not** transliterate Rust; do not vendor TSX.

Priority order (what prefilled workspaces / accepted-outcome demos need first):

1. `prompt-input` (composer + submit + attachments)
2. `message` (user/assistant turns, roles, timestamps)
3. `code-block` (syntax surface + run/test result panel)
4. `task` / task list (steps, status)
5. `sources` (citations / provenance list — ties to receipts)
6. `tool` / agent status (tool calls, agent cards)
7. `confirmation` / approval controls (human-in-the-loop gate)
8. `reasoning` (collapsible thinking)
9. `web-preview` (rendered artifact preview)

Each component: typed props via Effect Schema, returns `Html` from `foldkit/html`,
classes from named constants (no ad-hoc/model-authored classes), a base-contract
tag, and a coverage test. Add an `ai-elements` coverage test mirroring the Maud
catalog counts as the spec.

**DoD:** `ai-elements` family exported from `@openagentsinc/ui`; coverage test
asserts the catalog; the priority-1..7 components render and are usable in a
workspace demo.

## Phase 3 — Components workbench route

Add an internal `/components` gallery (mirroring
`autopilot4-deprecated/src/component_workbench.rs`) that renders each family with
owner/purpose/use-avoid/a11y/tokens notes. Render it with Foldkit in the real app
shell (not Storybook). Gate it to internal/non-public. Optional: isolated Figma
parity screens (kept out of the live product theme).

**DoD:** `/components` lists every family + the new `ai-elements`, with contracts
documented per family.

## Phase 4 — Wire into Blitz prefilled workspaces

Assemble the accepted-outcome / prefilled-workspace surfaces from
`@openagentsinc/ui` `ai-elements`: agent card + prompt input + task panel +
sources + confirmation + receipt summary. (Product work; see the workspace-repo
blitz docs for what each partner's workspace contains.)

## Conventions & guardrails

- **Source exports, no build** — same as `@openagentsinc/autopilot-ui`. Consumed
  via `workspace:*`. If/when published externally, follow
  `apps/pylon/docs/npm-publishing-runbook.md` (scope `@openagentsinc/`, never
  `@openagents/`).
- **Trusted UI only:** typed components, classes from constants, no model-authored
  HTML/`Html`; JS only toggles trusted server-emitted markup. Honor the dark-only
  / pure-black / compact-mono design contract in the existing `ui/README.md`.
- **Reference, don't vendor:** `autopilot3` (React source) and
  `autopilot4-deprecated` (Maud port) are separate clones used as
  source/curation maps; do not copy their code wholesale into this repo.
- Keep Git operations scoped to this repo; work on `main`; neutral
  (non-personal) commit metadata per repo policy.

## Quick task checklist (ordered)

- [ ] P0: scaffold `packages/ui` (package.json, tsconfig) mirroring `autopilot-ui`.
- [ ] P0: move kit families + `index.ts` + `coverage.test.ts` into `packages/ui`.
- [ ] P0: add `@openagentsinc/ui` dep to the web app; add re-export shim; migrate imports.
- [ ] P0: `bun test` + `tsc --noEmit` green; confirm no visual diff.
- [ ] P1: consolidate tokens; make `autopilot-ui` depend on `@openagentsinc/ui`.
- [ ] P1: add base-contract tags + typed fail-closed descriptor + test.
- [ ] P2: build `ai-elements` family (priority order) + coverage test.
- [ ] P3: add `/components` workbench route.
- [ ] P4: assemble prefilled-workspace surfaces from `@openagentsinc/ui`.
