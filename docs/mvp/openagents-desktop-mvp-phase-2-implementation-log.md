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
