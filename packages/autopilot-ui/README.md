# @openagentsinc/autopilot-ui

Foldkit-rendered Autopilot domain UI shared by `apps/openagents.com` and the
Autopilot desktop webview.

Token/class migration notes:

- `src/domain-styles.ts` owns the shared Autopilot domain class vocabulary:
  panels, rows, metadata, action buttons, progress bars, and status chips. Its
  matching CSS references the central `--oa-*` tokens from
  `@openagentsinc/design-tokens`.
- `src/view.ts` still keeps the public Foldkit API surface. Its shared
  `statusChip`, `SessionList` / `SessionRow`, `DecisionCard`, and
  `EventTimeline` now use the neutral Foldkit class helper.
- The first standalone domain modules migrated to token-backed classes are node status,
  cloud quota, earnings, decisions, session actions, assignments, artifacts,
  receipts, and verify status.
- `src/tokens.ts` remains a compatibility facade over
  `@openagentsinc/design-tokens` until all downstream imports move to the
  neutral token package.
- Public activity, account controls, steering controls, diff review, and larger
  app-composite surfaces are intentionally still coexistence work.
