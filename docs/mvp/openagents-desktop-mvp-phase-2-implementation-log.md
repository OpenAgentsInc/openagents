# OpenAgents Desktop MVP phase 2 implementation log

- ProductSpec: `openagents-desktop-mvp-phase-2-react-codex-workbench.product-spec.md`
- Parent issue: [#8817](https://github.com/OpenAgentsInc/openagents/issues/8817)
- Started: 2026-07-14
- Rule: packets land sequentially on `main`; this log records the shipped
  boundary, verification, and remaining compatibility scope after each packet.

## MVP-02A — React projection boundary (#8818)

Status: implemented and verified.

Canonical Effect Native revision:
`086378e03b2546d39a85b6b74ac1269e8587b23b`.

Delivered:

- one React 19 root and one Scope-owned Effect stream subscription;
- stable synchronous snapshots consumed through `useSyncExternalStore`;
- explicit, mutually exclusive `react` and `compatibility` surface backends;
- ordinary semantic React lowerings for Stack, Text, Button, Card, Spacer, and
  Divider, preserving keys, bounded a11y, typed style tokens, and existing
  `IntentReporter` identities;
- public loading, failed, and incompatible states with React error recovery;
- shared canonical token/component stylesheet for both DOM backends;
- exact React/React DOM pins in canonical Effect Native and app-owned dedupe in
  Desktop;
- Tailwind default theme namespaces disabled; its semantic aliases derive only
  from canonical `--en-*` variables;
- an explicit Desktop compatibility selection until later packets cover the
  complete workbench subset; and
- invariant, vendor, import-boundary, and backend-selection guards.

Verification:

- `bunx tsc -b packages/render-dom --pretty false`
- `bun test packages/render-dom/test` — 113 passed
- vendored render-dom typecheck and focused React/vendor/Desktop boundary tests
- Desktop typecheck and complete suite — 133 files passed, 1,312 tests passed,
  39 skipped
- production build — `boot.js` 819.22 kB and `app.css` 11.68 kB
- built Electron smoke and reload path — all checks passed; lifecycle teardown
  reported zero active owners

Remaining compatibility boundary:

- Desktop intentionally selects `backend: "compatibility"` for its full
  catalog. MVP-02B through MVP-02E expand declared React lowerings by product
  slice before the integrated MVP-02F proof can select React for the complete
  retained workbench.

## MVP-02B — React workbench shell and session rail (#8819)

Status: implemented; the scoped React path is available with
`?renderer=react-shell`. The complete app stays on the compatibility default
until MVP-02F because the timeline, composer, decisions, and review surfaces
are intentionally owned by their following packets.

Canonical Effect Native revision:
`ec04d1a066d6f3ed0c67735ba451cfc90a343aa8`.

Delivered:

- a generic value parameter on the existing Scope-owned React external store,
  preserving the `View` default while allowing Desktop to consume the
  authoritative `DesktopShellState` without a React-owned domain store;
- ordinary React `WorkbenchShell`, `SessionRail`, and `ConversationHeader`
  components with one React root, Strict Mode, shared failure containment, and
  one Effect subscription;
- metadata-first globally ordered local and Codex session rows, deterministic
  deduplication, selection, search, load-more, and honest scanning/empty states;
- existing intent identities for new, select, resume/open, archive, two-step
  delete, recovery, and catalog pagination;
- lifecycle and bounded repository context in the conversation header, without
  provider/account/model controls or absolute paths;
- a below-980-pixel overlay rail with Escape close, focus restoration, visible
  focus treatment, keyboard row traversal, reduced-motion compatibility, and a
  760-by-520 supported minimum; and
- renderer-private CSS derived only from canonical Effect Native variables.

Runtime clarification:

- Phase 2 does not use or plan to add the Vercel AI SDK. React owns
  presentation only; the existing Codex Runtime Gateway, compatible app-server,
  Effect services, and typed intent registry retain streaming, tools, sessions,
  and command authority.
- The pinned T3 Code reference likewise declares no `ai` or `@ai-sdk/*`
  dependency. Its coding-provider path uses Effect client runtime/contracts and
  a generated `effect-codex-app-server` integration (plus other provider
  adapters), so React adoption does not imply AI SDK adoption.

Verification:

- canonical React store focused tests — 5 passed;
- canonical Effect Native TypeScript build — passed;
- canonical full suite — 639 passed, with two unrelated committed visual
  baseline mismatches (`counter-phone` and `counter-desktop`) retained as a
  transparent repository gate caveat;
- Desktop typecheck — passed;
- Desktop complete suite against the rebuilt artifact — 134 files passed,
  1,319 tests passed, 39 skipped;
- focused typography and release-preflight gates — 19 passed, including the
  production-bundle local-path rejection;
- production build — `boot.js` 888.28 kB and `app.css` 167.72 kB (the CSS now
  contains the checked-in shadcn utilities and locally bundled variable font
  faces);
- wide, narrow, and narrow-open-overlay headed visual proofs confirmed the
  scoped React shell, responsive rail, scrim, and initial search focus; and
- ProductSpec validation passed under both OpenAgents and upstream profiles
  (the repository ProductSpec suite passed 102 tests), and the built Electron
  smoke completed every compatibility-path check with zero active lifecycle
  owners after teardown.

## Owner-directed shadcn component extension (#8824)

Status: integrated into the React shell packet.

Preset:

- code `b3Zg9L0M8A`;
- `base-vega` style with zinc base, blue theme, cyan chart intent;
- Oxanium variable body font and Geist variable heading font;
- small radius, Lucide icons, subtle menu accent, default-translucent menu;
- pointer cursor enabled.

The exact Vite initializer initially rejected the custom Electron package
because it lacked a conventional `vite.config.ts`. Desktop now exposes one
conventional config shared by its production build and component tooling; the
same exact initializer then completed successfully. Modern package-import
aliases (`#components`, `#lib`, `#hooks`) keep generated source resolvable by
Vite, tests, and TypeScript without the deprecated `baseUrl` option.

The preset is implemented as a Khala extension, not a second theme:

- generated Button, Input, ScrollArea, and Separator source lives under
  `apps/openagents-desktop/src/components/ui`;
- the React shell prefers those components for its controls, search, scrolling,
  and separation;
- `shadcn-khala.css` retains preset fonts, shape, menu behavior, animation, and
  component utilities but maps background, foreground, surface, primary,
  secondary, muted, accent, destructive, border, focus, chart, radius, and
  sidebar semantics onto canonical `--en-*` roles; and
- a conformance oracle rejects independent `oklch(...)` or hex palette values
  in that extension.

The Vercel AI SDK remains absent. shadcn changes the React component source
layer only; Codex Runtime Gateway/app-server and Effect authorities are
unchanged.

The typography assurance contract was extended in the same packet: Oxanium is
the approved body/UI family, Geist is the approved heading family, system
families remain resilient body fallbacks, and generic monospace remains the
code fallback. Its recursive source oracle rejects any other family or font
shorthand. The shadcn palette remains a semantic alias layer over Khala's
canonical Effect Native variables rather than an independent zinc/blue theme.

## MVP-02C — Typed React conversation timeline (#8820)

Status: implemented on the scoped `?renderer=react-shell` path. The complete
app remains on the compatibility default until MVP-02F.

Delivered:

- ordinary React timeline and item components over the existing bounded
  `CodexHistoryItem` and local `DesktopNoteEntry` projections, with no provider
  event parser or React-owned transcript array;
- stable authoritative item-ref keys, sequence ordering, duplicate collapse,
  and assistant segments that remain separated by display-bearing non-text
  records;
- in-place tool invocation/result correlation at the invocation key;
- distinct text, reasoning, plan, tool, approval, usage, metadata, context,
  error, gap, redaction, lifecycle, terminal, and local-question treatments;
- exactly one newest authoritative terminal disposition when a recovered
  prefix contains superseded terminal lifecycle records;
- the existing bounded Markdown parser lowered to safe React text, headings,
  emphasis, lists, quotes, and code, with links remaining inert visible text
  and no HTML injection surface;
- per-item error containment that reports an unavailable presentation item
  without fabricating completion or dropping sibling rows;
- a pre-mutation `getSnapshotBeforeUpdate` anchor receipt plus pre-paint
  correction for variable-height prepends and offscreen height changes;
- manual-reader position preservation, bounded live-edge following, a shadcn
  new-activity affordance, top/bottom typed pagination intents, atomic session
  replacement, and bounded live-region summaries; and
- a 500-item maximum-page projection/render/update/teardown corpus. It passed
  without a virtualizer, retained exactly 500 keyed rows through an in-place
  stream update, and removed every row on unmount, so no virtualization
  dependency was added.

The React module receives only the same bounded/redacted typed projection the
compatibility renderer already receives. Effect services, Runtime Gateway,
history completeness, persistence, terminal truth, and intent identities are
unchanged; Vercel AI SDK remains absent.

Verification:

- Desktop typecheck — passed;
- focused timeline, shell, renderer-boundary, design-conformance, and release
  preflight suites — 74 tests passed;
- production build — `boot.js` 898.23 kB and `app.css` 171.84 kB;
- full Desktop suite — 136 files passed, 1,333 tests passed, 39 skipped; and
- ProductSpec validation passed under OpenAgents and upstream profiles, its
  repository suite passed 102 tests, and built Electron compatibility smoke
  passed with zero active lifecycle owners after teardown.
