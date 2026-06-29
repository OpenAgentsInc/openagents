# Tailwind UI Foldkit Coverage

Updated: 2026-06-20

The shared Tailwind UI/Foldkit component registry lives in `packages/ui`, not in
`apps/openagents.com/apps/web/src/ui`. The app-local `src/ui/index.ts` path is a
shim that re-exports `@openagentsinc/ui` plus a few app-local helpers.

Detailed coverage note:

- `apps/openagents.com/docs/2026-06-03-tailwind-ui-foldkit-port-coverage.md`

Related launch audit:

- `docs/launch/2026-06-20-tailwind-ui-foldkit-business-landing-audit.md`

## Current Standard

Coverage is family-level. We track the Tailwind UI v4 family taxonomy and
verify local HTML variants map to those families when the proprietary local
downloads are present. We do not claim a one-to-one Foldkit export for every
individual Tailwind UI example variant.

Current local inventory:

- Application UI v4: `364` HTML variants
- Ecommerce UI v4: `114` HTML variants
- Marketing UI v4: `179` HTML variants

## Guards

- `packages/ui/test/coverage.test.ts` protects the family arrays and local
  download inventory.
- `apps/openagents.com/apps/web/src/business-route.test.ts` asserts the
  recomposed `/business` route emits expected shared `data-ui-family` markers.
- `apps/openagents.com/apps/web/src/components-route.test.ts` asserts the live
  component workbench renders the Business and Public theme families.
- `apps/openagents.com/scripts/check-zero-debt-architecture.mjs` runs in
  pre-push `check:deploy` and guards `/business` against drifting back into a
  class-only landing page.
