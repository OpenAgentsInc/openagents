# @openagentsinc/autopilot-ui

Foldkit-rendered Autopilot domain UI shared by `apps/openagents.com` and the
Autopilot desktop webview.

StyleX migration notes:

- `src/domain-styles.ts` owns the first shared Autopilot domain StyleX
  vocabulary: panels, rows, metadata, action buttons, progress bars, and status
  chips. It types the CSS custom-property contract against
  `@openagentsinc/ui/tokens` so raw Bun tests avoid runtime `defineVars` while
  compiled app builds still receive StyleX component styles.
- `src/view.ts` still keeps the public Foldkit API surface. Its shared
  `statusChip` is StyleX-backed, and `SessionList` / `SessionRow`,
  `DecisionCard`, and `EventTimeline` now use the StyleX/Foldkit adapter.
- The first standalone domain modules migrated to StyleX are node status,
  cloud quota, earnings, decisions, session actions, assignments, artifacts,
  receipts, and verify status.
- `src/tokens.ts` remains a compatibility facade over
  `@openagentsinc/design-tokens` until all downstream imports move to the
  neutral token package.
- Public activity, account controls, steering controls, diff review, and larger
  app-composite surfaces are intentionally still coexistence work.
