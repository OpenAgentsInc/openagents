# Aiur admin panel — deploy runbook

Date: 2026-07-06
Owns: `apps/aiur/` — the owner-only OpenAgents admin panel for the Khala
Code mobile-only MVP (epic #8467). AIUR-1 (#8499) scaffold + owner-only
auth + Khala Sync connection; AIUR-2 (#8500) credits console; AIUR-3
(#8501) ops views.

## What this is

A fresh, standalone Cloudflare Worker (`openagents-aiur`) running TanStack
Start (React + Tailwind), deployed at the custom domain
`aiur.openagents.com`. It is NOT a route inside the main `openagents.com`
Worker — separate `wrangler.jsonc`, separate deploy, separate session
cookies (`aiur_access`/`aiur_refresh`, never confused with the main site's
`oa_access`/`oa_refresh`).

It reuses the shared `auth.openagents.com` OpenAuth issuer's GitHub
provider (same downstream client id, `openagents-web`, as the main web
app — Aiur is simply a newly allow-listed redirect hostname for that
client; see
`apps/openagents.com/workers/api/src/auth/mobile-session.ts`
`authIssuerAllowsWebRedirectHostname`). After a session is verified, Aiur
applies its OWN hard allowlist gate
(`AIUR_OWNER_USER_IDS`, `apps/aiur/src/auth/owner-gate.ts`) — fail-closed:
unset or empty denies every request, including a legitimately signed-in
non-owner.

## Prerequisites

- `AIUR_OWNER_USER_IDS` set to the comma-separated OpenAuth user id(s) of
  the actual OpenAgents owner (the verified `userId` a signed-in session
  carries — NOT a GitHub login/handle). This is genuinely owner-gated: see
  `NEEDS_OWNER.md` for how to look it up and set it. Until this is set,
  Aiur denies everyone, including the owner — a deliberate fail-closed
  default, not a bug.
- DNS/route: `aiur.openagents.com` must be a proxied (orange-cloud) CNAME
  or A record on the same Cloudflare zone as `openagents.com`, and the
  zone must have this Worker's custom-domain route
  (`wrangler.jsonc` → `routes: [{ pattern: "aiur.openagents.com",
  custom_domain: true }]`) provisioned — `wrangler deploy` normally
  attaches this automatically the first time it runs against an
  authenticated Cloudflare account with the zone, but if the domain does
  not resolve after a deploy, check the Cloudflare dashboard's Workers
  Routes / Custom Domains page for the zone (owner-gated dashboard step —
  see `NEEDS_OWNER.md`).

## Deploy

```sh
bun install
bun run --cwd apps/aiur typecheck
bun run --cwd apps/aiur test
bun run --cwd apps/aiur build
bun run --cwd apps/aiur deploy    # = wrangler deploy
```

Set/update the owner allowlist and any secrets via `wrangler secret put` or
the `vars` block in `apps/aiur/wrangler.jsonc` (the allowlist and OpenAuth
client id are NOT secret — they are public-safe config — but the
allowlist should still be set per-environment rather than committed with a
real owner user id).

## Live smoke

```sh
curl -fsSI https://aiur.openagents.com/
```

Expect `200` (or a redirect into the sign-in shell) with the standard
security headers (`X-Frame-Options: DENY`, etc. — see
`apps/aiur/src/server.ts` `SECURITY_HEADERS`). Then, as the owner:

1. Visit `https://aiur.openagents.com/` and click "Sign in with GitHub".
2. Confirm the OpenAuth flow completes and lands back on Aiur signed in
   (not denied) — this proves the redirect-hostname allowlist widen and
   the owner allowlist are both configured correctly.
3. Confirm the "Khala Tokens Served" panel renders a real, live number
   (not stuck on "Connecting...") — this is the live Khala Sync proof
   (`scope.public.tokens-served`), proxied through `apps/aiur/src/
   khala-sync-proxy.ts` with the owner's own OpenAuth access token as the
   Khala Sync bearer (mirrors the mobile session bridge, #8469).
4. From a second, non-owner GitHub account (or by clearing
   `AIUR_OWNER_USER_IDS` and redeploying), confirm sign-in succeeds at the
   OpenAuth layer but Aiur renders "Access denied" and no dashboard data
   loads — the fail-closed gate.

## AIUR-2: the credits console also needs the main Worker deployed

The credit ledger, its migration, and the `/api/admin/credits/*` routes
live in the MAIN `openagents.com` Worker
(`apps/openagents.com/workers/api/src/admin-credits-routes.ts` +
`inference/admin-credit-grant.ts` + migration
`migrations/0308_admin_credit_grants.sql`), not in Aiur. Deploying Aiur
alone is not sufficient for the credits console to work — the main
Worker's normal deploy path (`docs/DEPLOYMENT.md`'s `openagents.com
Worker` row, `bun run --cwd apps/openagents.com/workers/api deploy:safe`)
must have shipped the migration and the new routes first. Live smoke for
that side:

```sh
# Expect 401 (unauthenticated) from a plain curl — this route requires a
# real owner bearer, which only Aiur's proxy supplies:
curl -fsS https://openagents.com/api/admin/credits/balance?userId=test
```

Then, as the owner, from `https://aiur.openagents.com/credits`: search a
real user, grant a small amount with a reason, confirm the balance and
history update, and claw it back — see `apps/aiur/README.md`'s AIUR-2
section for the exact surfaces involved.

## Rollback

`wrangler rollback` against the `openagents-aiur` Worker, same as any
other Cloudflare Worker in this repo. Aiur has no D1/KV of its own — all
data reads/writes go through the owner-gated Khala Sync proxy to the
production Khala Sync API and (AIUR-2 onward) the same-origin admin
credits proxy to the main `openagents.com` Worker's `/api/admin/credits/*`
routes — so a rollback here never risks data loss, only the admin UI's
availability. Rolling back the credit ledger itself is a main-Worker
concern (see that Worker's own deploy/rollback runbook).
