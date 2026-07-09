# CX-2 stale Codex-account cleanup (issue #8546)

Operator runbook for tidying accumulated dead `provider_accounts` rows for the
`chatgpt_codex` provider, filed from the CX-2 exit-receipt audit (#8546).

## Is this required?

**No — it is optional DB hygiene.** The projection fix already hides all
stale/dead residue from the phone: the mobile list
(`GET /api/mobile/codex-accounts`) now shows only *live* accounts (connected,
or an in-progress non-expired device login), and disconnect now soft-deletes
the row (`deleted_at`) so it leaves every `deleted_at IS NULL` projection
immediately. This script only removes the now-hidden rows from D1 so the table
is clean; skipping it changes nothing the user sees.

It is **idempotent** (already-`deleted_at` rows are skipped) and **list-first**
(run the dry-run counts before any write).

## Store

Prod D1 database: `openagents-autopilot`
(binding `OPENAGENTS_DB`, id `9644ea09-…`). Staging:
`openagents-autopilot-staging`. Run from
`apps/openagents.com/workers/api`.

## Step 1 — DRY RUN (read-only, always run first)

```sh
cd apps/openagents.com/workers/api
bunx wrangler d1 execute openagents-autopilot --remote --command "
  SELECT status, (deleted_at IS NULL) AS live, COUNT(*) AS n
  FROM provider_accounts
  WHERE provider = 'chatgpt_codex'
  GROUP BY status, live
  ORDER BY n DESC;"
```

Per-user magnitude (identify who has accumulation; no token material is read):

```sh
bunx wrangler d1 execute openagents-autopilot --remote --command "
  SELECT user_id, COUNT(*) AS n
  FROM provider_accounts
  WHERE provider = 'chatgpt_codex' AND deleted_at IS NULL
  GROUP BY user_id ORDER BY n DESC LIMIT 10;"
```

Snapshot at time of writing (2026-07-09), live `chatgpt_codex` rows:
`unhealthy=16, connected=12, pending=8, denied=5` across all users; the owner's
own account held 26 live rows (16 unhealthy, 5 pending whose device codes had
all expired, 5 connected). After the projection fix the owner's phone shows
only the 5 connected rows.

## Step 2 — APPLY (soft-delete unambiguously dead rows)

Targets only terminal-dead residue, and never a `connected` row:

- `status = 'denied'` — the user declined the device login.
- `status = 'expired'` — the account's own status is expired.
- `status = 'pending'` **and every** connection attempt has already expired —
  an abandoned device login that was never completed.

It intentionally does **not** touch `unhealthy` rows: those can recover via a
reauth and are already hidden from the phone by the projection filter. If, after
eyeballing the dry-run, an operator wants to also retire specific `unhealthy`
rows, add `'unhealthy'` to the `IN (...)` list deliberately.

```sh
# Soft-delete denied/expired terminal rows.
bunx wrangler d1 execute openagents-autopilot --remote --command "
  UPDATE provider_accounts
  SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
      updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
  WHERE provider = 'chatgpt_codex'
    AND deleted_at IS NULL
    AND status IN ('denied', 'expired');"

# Soft-delete abandoned pending logins (no non-expired attempt remains).
bunx wrangler d1 execute openagents-autopilot --remote --command "
  UPDATE provider_accounts
  SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
      updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
  WHERE provider = 'chatgpt_codex'
    AND deleted_at IS NULL
    AND status = 'pending'
    AND NOT EXISTS (
      SELECT 1 FROM provider_account_connection_attempts att
      WHERE att.provider_account_id = provider_accounts.id
        AND att.status = 'pending'
        AND att.expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')
    );"
```

Re-run Step 1 to confirm the live counts dropped and nothing `connected` was
touched. Both statements are safe to re-run (already-`deleted_at` rows are
excluded by `deleted_at IS NULL`).

## Safety notes

- Only ever soft-deletes (`deleted_at`); no hard `DELETE`, no token material.
- Never targets `connected` rows, so a real linked account is never removed.
- Staging first: swap `openagents-autopilot` → `openagents-autopilot-staging`.
