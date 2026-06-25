# After-Action Report — Khala gateway-wide 500 on `/api/v1/chat/completions`

- **Date:** 2026-06-25
- **Severity:** SEV-1 (gateway-wide; the public Khala inference product was down)
- **Duration of customer impact:** ~10+ minutes (manual detection latency)
- **Status:** Resolved + prevention layer shipped (items 1–3 below)

## Summary

Every `POST /api/v1/chat/completions` returned **500 `internal_server_error`**
for **all** API keys — a gateway-wide outage of the public `openagents/khala`
inference product. `GET /api/v1/models` stayed `200` (so the worker was up), but
the public tokens-served counter froze for ~10+ minutes. Agent registration and
**every bearer-auth credential lookup** also 500'd.

The cause was **worker code shipped ahead of its database schema**: a deploy that
bypassed the sanctioned path also skipped the D1 migration the new code depended
on. The fix was to apply the one pending migration. The deeper fixes are a
deploy-migration safety guard (so this class of failure cannot recur), a tight
500 red-alert canary (so the next outage is detected in ~90s, not ~10 min by a
human), and an env-overridable free-key mint cap (so a responder is never keyless
mid-incident).

## Timeline (UTC)

- **~18:08** — Commit `dda6eae8` ("Route Khala coding workflows through Pylons",
  issue #6273) is merged. It introduces worker code that reads/writes
  `agent_credentials.openauth_user_id`, a column added by migration
  `0234_pylon_openauth_links.sql`.
- **~shortly after** — A deploy ships the new worker to prod using a **shortcut**
  (`build:web && bunx wrangler deploy --assets …`) taken to dodge the flaky
  `verse-launch-smoke` in `check:deploy` (#6234). That shortcut **also skipped
  `wrangler d1 migrations apply`**, so migration `0234` was never applied to
  remote D1. The worker is now running against a schema that lacks
  `openauth_user_id`.
- **T+0** — Every credential lookup hits the missing column and throws; the chat
  route's generic catch returns `500 internal_server_error`. `/api/v1/models`
  (no credential lookup on that path) stays `200`. The public tokens-served
  counter stops advancing.
- **T+~10 min** — The **owner notices manually** (no automated alert fired).
- **Detection→resolution** — Root-caused to the missing migration. Ran
  `wrangler d1 migrations apply openagents-autopilot --remote`, applying the one
  pending migration (`0234`).
- **Verified** — Registration `201`, completions `200`, the tokens-served counter
  moving again. Confirmed resolved.

## Root cause

`dda6eae8` (#6273) shipped worker code that depends on the
`agent_credentials.openauth_user_id` column (migration `0234_pylon_openauth_links.sql`),
but the deploy that uploaded that worker **did not apply migration 0234 to remote
D1**. Every credential lookup then hit a missing column and threw, and the chat
route's generic catch turned that into a `500`. Because credential lookup is on
the hot path for *every* bearer-authenticated request, the failure was
gateway-wide across all keys.

**Why the migration was skipped (the real systemic cause).** The sanctioned
deploy (`bun run deploy`) runs `wrangler d1 migrations apply` **and** `check:deploy`.
But recent deploys used a shortcut — `build:web && bunx wrangler deploy --assets`
— specifically to **bypass the flaky `verse-launch-smoke`** that had been hanging
/ OOM-killing `check:deploy` in headless/CI (#6234). That shortcut bypassed the
*entire* sanctioned path, so it **also skipped migrations** — and nothing failed
the deploy when the worker shipped ahead of its schema. The flaky smoke created a
standing incentive to bypass the guard, and the bypass had no migration safety
net.

## Impact

- Public Khala inference (`POST /api/v1/chat/completions`) returned `500` for all
  keys for ~10+ minutes.
- Agent registration and all bearer-auth credential lookups 500'd.
- The public tokens-served counter (the North-Star liveness signal) froze.
- No data loss; no incorrect billing (the path failed closed before metering).

## Detection gap

The outage was **not auto-detected**. The existing 15-minute liveness heartbeat
(`scripts/khala-heartbeat.sh`) is too coarse (and too heavy) to be a fast outage
detector, and there was no edge-triggered 500 alert. A human caught it after
~10 minutes.

## Resolution

Applied the single pending migration to remote D1:

```sh
cd apps/openagents.com/workers/api
wrangler d1 migrations apply openagents-autopilot --remote
```

Then verified registration `201`, completions `200`, and a moving counter.

## Prevention ("never again")

### 1. Deploy-migration safety guard (the keystone — prevents code-ahead-of-schema)

- **`apps/openagents.com/scripts/check-pending-migrations.mjs`** — runs
  `wrangler d1 migrations list openagents-autopilot --remote` and **exits
  non-zero if ANY migration is pending**, naming the pending files and the
  remediation. Pure parser (`parseMigrationsList` / `decidePendingMigrations`) is
  unit-tested (`scripts/check-pending-migrations.test.ts`, wired into
  `check:deploy` via `test:pending-migrations-guard`). Wired as
  `bun run check:pending-migrations`.
- **`deploy:safe`** (`apps/openagents.com/workers/api/package.json`) — the
  sanctioned fast deploy path, which always runs, IN ORDER:
  `check:deploy-from-main` (local==origin/main) → `check:deploy` (typecheck:web +
  typecheck:api + the real web/worker test suites + guards) → **`wrangler d1
  migrations apply --remote`** → **`check:pending-migrations`** (must show zero
  pending) → `build:web` → `wrangler deploy`. The old `deploy` now aliases
  `deploy:safe`. Crucially, `check:deploy` has **no dependency on the flaky
  `verse-launch-smoke`** (#6234), so there is no longer any reason to bypass the
  guard with a raw deploy. **This closes the exact gap that caused the outage:**
  migrations are applied *before* the worker is uploaded, and the deploy fails
  loud if any remain pending.
- **`docs/DEPLOYMENT.md`** + **`apps/openagents.com/AGENTS.md`** updated: the ONLY
  sanctioned Worker deploy is `deploy:safe`; **raw `bunx wrangler deploy` is
  forbidden** because it skips migrations; migrations are always applied before
  the worker is uploaded.

### 2. 500 RED-ALERT synthetic canary (fast detection)

- **`scripts/khala-canary.sh`** — fires ONE real `openagents/khala` completion
  every ~90s (LaunchAgent `StartInterval 90`). On a 500 / non-200 / counter-not-
  moving it declares **DOWN** and, on a healthy→down **transition** (edge-
  triggered, no spam), fires a RED ALERT: a prominent
  `~/work/.khala-heartbeat/RED-ALERT.log` block (with the first-investigation
  steps, `check:pending-migrations` first) + one dated `RED-ALERT:` line in
  `~/work/NEEDS_OWNER.md` + a non-zero exit so a watcher reacts. A 402/429 is
  DEGRADED (quota), not an outage. Secret-safe (keys from the gitignored secrets
  file, never printed). Documented in
  `docs/inference/2026-06-25-khala-heartbeat-runbook.md` (Canary section), with
  the LaunchAgent plist and the agent-investigation procedure.

### 3. Mint-key fix (so a responder is never keyless)

- **`FREE_KEY_MAX_MINTS_PER_IP_PER_DAY`** is now **env-overridable**
  (`resolveFreeKeyMintCap`, read in the `POST /api/keys/free` route via the
  existing `parsePositiveIntEnv` pattern) and the compiled default was raised
  **25 → 200**. During the incident the responder hit
  `free_key_mint_rate_limited` (25/IP/day, not env-overridable) trying to mint a
  fresh key to test the recovering gateway. Tests updated
  (`free-key-mint-routes.test.ts`, `inference/inference-free-tier-key.test.ts`).
- **Ops free-key pool convention** documented in the runbook: keep a small stable
  pool of pre-provisioned free-tier keys at
  `~/work/.secrets/khala-ops-keys.env` (`KHALA_CANARY_KEYS`) so ops/canaries
  reuse keys instead of minting. (Secrets are never committed.)

## Tracking

- **Epic:** #6289 — Never again — prevent code-ahead-of-schema + add 500 red-alert.
- **#6290** — Deploy-migration safety guard (item 1, the keystone).
- **#6291** — 500 RED-ALERT synthetic canary (item 2).
- **#6292** — Free-key mint cap env-overridable + raised default (item 3).

Honest scope: `deploy:safe` cannot be fully exercised in CI (it needs owner OAuth
to reach remote D1) — the pure logic of `check:pending-migrations` is unit-tested
in CI (and was verified against real remote D1, reporting 0 pending now that
`0234` is applied), and each component script is verifiable independently.

## Lessons

- A flaky gate creates a standing incentive to bypass the gate — and the bypass
  took out the *migration* safety net along with the flaky smoke. Fix flaky
  gates by **removing them from the critical path** (done: #6234) AND by making
  the sanctioned path the only one that can reach prod, not by tolerating raw
  bypasses.
- Schema and the code that depends on it must ship as one atomic, ordered unit:
  **migrate first, then upload the worker**, and **fail the deploy** if anything
  is pending.
- A coarse liveness heartbeat is not an outage detector. A tight, edge-triggered
  canary on the real customer path is.
