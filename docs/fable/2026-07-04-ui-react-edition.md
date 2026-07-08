# TS-9 UI React Edition Receipt

**STATUS (2026-07-08): SUPERSEDED by `docs/fable/MASTER_ROADMAP.md`
§EN (rev 6) — the Effect Native full-conversion mandate.** Kept as
the historical record of the earlier decision; do not implement
from this document.


Issue: #8342

TS-9 adds the React entrypoint for `@openagentsinc/ui` while preserving the
existing Foldkit package surface. It follows the TanStack component convention
of small React components plus merged Tailwind class strings, but swaps their
light/dark palette machinery for the existing OpenAgents StarCraft-blue
`@openagentsinc/design-tokens` variables.

Shipped scope:

- `@openagentsinc/ui/react` exports buttons, panels, top navigation with a
  mobile disclosure menu, cards, text/textarea fields, and code blocks.
- `@openagentsinc/ui/react.css` imports Tailwind 4 and projects `--oa-*` token
  variables through `@theme inline`.
- `openAgentsNativeWindTokens` exports the same palette as literal values for
  the TS-8 NativeWind lane.
- `ReactEditionSmokeFixture` is the Storybook-less visual fixture for package
  tests and future screenshot lanes.

Guardrails:

- Dark-only. No theme toggle and no `dark:`/`light:` Tailwind variants.
- The canonical token values stay in `@openagentsinc/design-tokens`.
- The Foldkit exports remain in place for delete-as-you-go migrations.

Verify:

```sh
bun run --cwd packages/ui test
bun run --cwd packages/ui visual-smoke
bun run --cwd packages/ui typecheck
```
