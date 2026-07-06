# Aiur — the OpenAgents owner-only admin panel

Part of the Khala Code mobile-only MVP (epic #8467). Owner direction
2026-07-06: the first MVP build ships **without** RevenueCat/IAP — credits
are assigned manually by the owner through **Aiur**, served at
**aiur.openagents.com**, a fresh Cloudflare Worker running TanStack Start
(React + Tailwind, per the ONE-UI decision), fully separate from the main
`openagents.com` Worker but connected to the same Khala Sync engine.

Filed as #8499 (this scaffold), #8500 (credits console), #8501 (ops views).

## Scope (AIUR-1, #8499)

- New TanStack Start app, its own `wrangler.jsonc`, its own Worker
  (`openagents-aiur`), routed at the custom domain `aiur.openagents.com`.
- **Owner-only auth, fail-closed.** Sign-in reuses the shared
  `auth.openagents.com` OpenAuth issuer's GitHub provider (the same
  downstream client id, `openagents-web`, as the main web app — Aiur is a
  new allow-listed redirect hostname for that client, see
  `apps/openagents.com/workers/api/src/auth/mobile-session.ts`
  `authIssuerAllowsWebRedirectHostname`). After a verified session exists,
  every route re-checks a hard allowlist
  (`AIUR_OWNER_USER_IDS`, comma-separated verified OpenAuth user ids) via
  `src/auth/owner-gate.ts` — **an unset or empty allowlist denies
  everyone**, including a legitimately signed-in non-owner. There is no
  "no allowlist configured => allow" path.
- **Khala Sync**: `src/khala-sync-proxy.ts` is Aiur's own same-origin proxy
  to production Khala Sync (`https://openagents.com/api/sync/*`), gated by
  the SAME owner check on every request — the browser never sees a real
  bearer token; the proxy attaches the signed-in owner's own OpenAuth
  access token server-side (mirrors the mobile session bridge, #8469:
  `syncToken = the OpenAuth access token`). The dashboard's live proof view
  (`src/dashboard/`) subscribes to the PUBLIC `scope.public.tokens-served`
  counter via `@openagentsinc/khala-sync-client`'s
  `createHttpKhalaSyncTransport`, through that proxy — a real bootstrap +
  real live WebSocket tail against production Khala Sync, not a mock.
- **Theme**: Protoss-blue, no light mode — Tailwind `@theme` tokens sourced
  from `@openagentsinc/design-tokens/theme.css` (`src/styles.css`), same
  `--color-khala-*` naming convention as
  `apps/openagents.com/apps/start/src/styles.css` so the small
  `components/ui/*` primitives (copied from that app) resolve unchanged.

## How the auth boundary is laid out

- `src/auth/cookies.ts` — Aiur's own session cookie jar (`aiur_access` /
  `aiur_refresh` / `aiur_auth_state`), independent from the main site's
  `oa_access`/`oa_refresh` (different origin, must never be confused).
- `src/auth/subjects.ts` — the `user` subject schema, field-for-field
  compatible with the shared issuer's `UserSubject` (Aiur is a CLIENT of
  that issuer, not an issuer itself).
- `src/auth/session.ts` — `createClient(...).verify(...)` wrapper.
- `src/auth/owner-gate.ts` — the pure, fail-closed allowlist check.
- `src/auth/access.ts` — `resolveAiurAccess`, the single funnel every
  data-touching route must call: `signed_out | denied | owner`.
- `src/auth/routes.ts` — `/auth/github/start`, `/auth/callback`,
  `/auth/logout`.
- `src/auth/access-route.ts` — `GET /api/aiur/access`, the UI-status
  endpoint the dashboard shell reads to decide what to render. This is
  UX-only: it never gates real data by itself, since every data route
  (the sync proxy, and AIUR-2/3's future credit/ops routes) re-checks
  `resolveAiurAccess` independently.
- `src/server.ts` — wires all of the above ahead of the TanStack Start
  handler.

## Scope (AIUR-2, #8500) — the credits console

The MVP-critical surface: manual credit grants replace IAP (#8481 postponed)
for the first build. The D1 credit ledger is owned by the MAIN
`openagents.com` Worker, not Aiur:

- **Server (main Worker)**: `apps/openagents.com/workers/api/src/
  admin-credits-routes.ts` exposes `/api/admin/credits/{users,balance,
  history,recent-grants,grant,clawback}`, gated by a NEW composition
  (`requireAdminCreditsCaller` in that Worker's `index.ts`) of the existing
  mobile-bearer session boundary (`requireUserBearerSession`) plus the
  existing admin-email allowlist (`isOpenAgentsAdminEmail`) — never a new
  auth primitive, never a shared static token. The actual money movement
  reuses Pool B's exact primitives verbatim via
  `apps/openagents.com/workers/api/src/inference/admin-credit-grant.ts`:
  `usdCreditGrantStatements` (RL-3 `revenueAsset: 'free'`) for grants,
  `clawbackInferenceCredits` for clawbacks, both idempotent on a
  caller-supplied ref (`admin_credit_grants` D1 table, migration `0307`).
- **Aiur side**: `src/admin-credits-proxy.ts` is the same-origin,
  owner-gated forwarding proxy (identical shape to `khala-sync-proxy.ts`) —
  the browser only ever talks to Aiur's own origin; Aiur attaches the
  signed-in owner's OpenAuth access token as the bearer when forwarding to
  the main Worker, which independently re-verifies it. `src/credits/`
  holds the UI: `credits-api-client.ts` (typed fetch wrappers),
  `credits-action-state.ts` (the pure grant/clawback confirm-then-submit
  state machine, `useReducer`-driven), and `credits-console.tsx` (search a
  user by GitHub login or user id, see balance + merged admin/signup grant
  history, a grant form with a confirmation step, a clawback form, and a
  recent-grants ledger across all users). Reachable at `/credits`.

## Commands

```sh
bun install
bun run --cwd apps/aiur typecheck
bun run --cwd apps/aiur test
bun run --cwd apps/aiur dev      # local dev server
bun run --cwd apps/aiur deploy   # wrangler deploy (needs AIUR_OWNER_USER_IDS set)

# The main Worker owns the credit ledger routes/migration:
bun run --cwd apps/openagents.com/workers/api typecheck
bun run --cwd apps/openagents.com/workers/api test
bun run --cwd apps/openagents.com check:architecture
```

## Production

`wrangler.jsonc` deploys Worker `openagents-aiur` and attaches the custom
domain route `aiur.openagents.com`. Deploy runbook:
`docs/khala-code/2026-07-06-aiur-admin-deploy-runbook.md` (linked from
`docs/DEPLOYMENT.md`). The credit ledger migration
(`apps/openagents.com/workers/api/migrations/0307_admin_credit_grants.sql`)
ships with the main Worker's normal `deploy:safe` migration-apply step —
Aiur itself has no D1 of its own.

## Honest gaps (AIUR-2)

- `/api/admin/credits/history` merges receipted grant events (admin +
  signup) only; it does not yet include raw inference-charge activity for a
  user (the same unpaginated shape `agent-balance-routes.ts` already
  exposes for a user's own balance view). AIUR-3's ops view is the natural
  home for "what did they run and did it charge correctly" across users.
- The recent-signups search and recent-grants ledger are simple
  `LIMIT`-capped reads, not true cursor-paginated — acceptable at MVP scale,
  flagged for AIUR-3 if it needs more.

## Explicitly out of scope here

The ops views (AIUR-3, #8501) build on this scaffold in a follow-up change.
