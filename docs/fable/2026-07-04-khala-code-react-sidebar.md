# TS-7 Phase 1 React Sidebar Receipt

**STATUS (2026-07-08): SUPERSEDED by `docs/fable/MASTER_ROADMAP.md`
§EN (rev 6) — the Effect Native full-conversion mandate.** Kept as
the historical record of the earlier decision; do not implement
from this document.


Date: 2026-07-04
Issue: #8349
Epic: #8339

## Scope

TS-7 phase 1 ports the Khala Code desktop chat/thread sidebar to a React +
Tailwind island while leaving Electrobun, the Codex app-server RPC contracts,
fleet machinery, composer, transcript, and remaining panels unchanged.

The stable desktop mount API remains
`clients/khala-code-desktop/src/ui/codex-thread-sidebar.ts`. That module now
re-exports the React implementation in
`clients/khala-code-desktop/src/ui/codex-thread-sidebar-react.tsx`, so existing
callers and behavior-contract tests continue to exercise the same public
surface.

## Behavior Boundary

The port preserves the existing sidebar behavior contracts:

- active row background and no visible "Current chat" copy;
- streaming spinner only in the timestamp slot for actually streaming threads;
- recent-thread hotkey hints and selection cycling;
- stored-only Codex catalog rows stay visible but disabled;
- optimistic new-thread and rename updates appear before RPC catch-up;
- raw internal Codex resume errors remain mapped to friendly sidebar text;
- harness badges stay removed from row chrome.

The vanilla DOM renderer was deleted in the same change. Context menus still
use the shared Basecoat DOM menu helper because the rest of the desktop shell
has not yet moved to React.

## Verification

- `bun test tests/codex-thread-sidebar.test.ts tests/ux-contracts.test.ts tests/app-shell.test.ts`
- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop scan:architecture`
- `bun run --cwd clients/khala-code-desktop build:ui`
- `bun run --cwd clients/khala-code-desktop verify`
- `OA_FORCE_KHALA_VISUAL_SMOKE_GATE=1 bun scripts/qa-visual-smoke-gate.ts`
