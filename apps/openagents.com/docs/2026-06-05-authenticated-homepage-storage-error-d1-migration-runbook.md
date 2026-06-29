# Authenticated Homepage Storage Error D1 Migration Runbook

Date: 2026-06-05

## Summary

Authenticated visits to `https://openagents.com/` could surface
`{"error":"storage_error"}` after the browser app loaded. The public app shell
itself was healthy; the error came from an authenticated startup API path that
read the active customer order.

Two separate storage failures hit the same route. First, the deployed Worker
expected production D1 tables from migrations newer than the database had
applied. The immediate missing table was `site_projects`, used by
`workers/api/src/customer-orders.ts` when reading
`/api/customer-orders/active`.

After the migrations were applied, the same route still failed because the
joined active-order query selected unqualified `software_orders` columns. Once
`site_projects` and `site_deployments` existed, D1 correctly rejected the
overlapping names as ambiguous. The final code fix qualifies every selected
order column in the joined reads.

## Impact

- Logged-out homepage traffic was not affected.
- Authenticated users routed through the order/onboarding startup path could
  see `storage_error`.
- After the route fallback was deployed, the user-facing raw `storage_error`
  stopped, but the order payload could temporarily return `order: null` until
  the SQL was qualified.
- API/session cookies were not the root cause.
- The database schema was behind deployed code by migrations `0031` through
  `0038`.

## Evidence

Deployed Worker:

```sh
bunx wrangler deployments list --name openagents-autopilot
```

Relevant deployed version during diagnosis:

```text
d1f92da9-d5db-402c-b5a5-305cfa72a5df
```

Pending migrations before the fix:

```text
0031_stripe_billing.sql
0032_autopilot_sites.sql
0033_adjutant_project_metadata.sql
0034_adjutant_assignments.sql
0035_adjutant_assignment_events.sql
0036_team_chat_adjutant_intent.sql
0037_adjutant_adjustment_requests.sql
0038_adjutant_notification_email_refs.sql
```

The production D1 query shape used by `readActiveOrderRow` failed with:

```text
no such table: site_projects: SQLITE_ERROR
```

After the schema was current, Cloudflare tail on the authenticated order fetch
showed:

```text
event: customer_order_active_storage_fallback
operation: customerOrders.active.read
errorMessage: D1_ERROR: ambiguous column name: id at offset 7: SQLITE_ERROR
```

That D1 error is wrapped as `CustomerOrderStorageError` and mapped to:

```json
{"error":"storage_error"}
```

## Why Diagnosis Took Longer Than Expected

The visible symptom was generic. `storage_error` is intentionally redacted and
can be emitted by more than one route family, including onboarding and operator
site routes. The homepage request itself returned `200` because it only served
the app shell; the failure happened later in authenticated browser API calls.

Unauthenticated probes were clean:

```sh
curl -i https://openagents.com/
curl -i https://openagents.com/api/auth/session
curl -i https://openagents.com/api/onboarding
```

Those checks showed normal public behavior and did not reproduce the
authenticated order-path failure. Cloudflare live tail also mixed unrelated
background traffic, pylon calls, scheduled events, and websocket traffic, so the
logs did not immediately isolate the failing path. The decisive first signal
came from comparing the deployed code's startup reads against the remote D1
migration ledger and then executing the customer-order query shape directly
against D1.

The second failure took another pass because the route intentionally returned a
generic `storage_error` body and did not include the storage operation in the
live tail output. A narrow route log using `logWorkerRouteError` exposed the
exact operation and SQLite error.

## Production Actions Taken

Applied remote migrations:

```sh
cd workers/api
bunx wrangler d1 migrations apply openagents-autopilot --remote
```

Wrangler applied:

```text
0031_stripe_billing.sql
0032_autopilot_sites.sql
0033_adjutant_project_metadata.sql
0034_adjutant_assignments.sql
0035_adjutant_assignment_events.sql
```

The normal migration run then stopped at `0036_team_chat_adjutant_intent.sql`
with:

```text
FOREIGN KEY constraint failed: SQLITE_CONSTRAINT_FOREIGNKEY
```

Root cause for that migration blocker:

- `0036` rebuilds `team_chat_messages` to expand its `kind` CHECK constraint
  from `message | autopilot_intent | system` to include `adjutant_intent`.
- Existing `thread_file_message_refs.message_id` references
  `team_chat_messages(id)`.
- The rebuild's `ALTER TABLE ... RENAME TO ...` / `DROP TABLE ..._old` sequence
  trips the child-table foreign key in D1.

Controlled manual recovery for `0036`:

1. Backed up the 7 `thread_file_message_refs` rows.
2. Dropped the child table.
3. Rebuilt `team_chat_messages` with the intended `adjutant_intent` CHECK.
4. Restored `thread_file_message_refs` and its indexes.
5. Inserted `0036_team_chat_adjutant_intent.sql` into `d1_migrations`.
6. Ran `PRAGMA foreign_key_check`.

Then applied the remaining migrations normally:

```sh
bunx wrangler d1 migrations apply openagents-autopilot --remote
```

Wrangler applied:

```text
0037_adjutant_adjustment_requests.sql
0038_adjutant_notification_email_refs.sql
```

Additional repair after the second failure:

1. Backfilled `software_orders` for completed-onboarding users that had no
   active order. This wrote 4 rows in production.
2. Added and applied
   `0039_completed_onboarding_order_backfill.sql` so the repair is tracked and
   idempotent in the repo.
3. Added redacted storage-operation logging for onboarding/customer-order route
   storage failures.
4. Added a defensive active-order fallback that returns `{ "order": null }`
   instead of exposing raw `storage_error` if this read path has another storage
   failure.
5. Qualified every selected `software_orders` column in both active and detail
   customer-order reads.

## Verification

Migration ledger:

```sh
bunx wrangler d1 migrations list openagents-autopilot --remote
```

Result:

```text
No migrations to apply
```

Foreign key verification:

```sql
PRAGMA foreign_key_check;
```

Result: no rows.

The previously failing customer-order query shape now returns successfully and
can read the active order with nullable site/adjutant fields:

```text
software_order_57593c2c60c54d25a140588633e3b318
site_status: null
site_active_url: null
latest_adjustment_status: null
```

The final production Worker version deployed for the SQL fix was:

```text
92532b9e-fa15-4fc2-8880-6a43b43ed02a
```

The focused customer-order route test passed:

```sh
bun run --cwd workers/api test -- src/customer-order-routes.test.ts
```

The canonical production deploy path also passed:

```sh
bun run --cwd workers/api deploy
```

Unauthenticated API behavior remained normal:

```text
/api/customer-orders/active -> 401 unauthorized
/api/auth/session -> 200 {"authenticated":false}
```

## Consequences

- Production D1 now includes Stripe, Sites, Adjutant assignment, Adjutant event,
  team-chat adjutant intent, adjustment request, and notification email ref
  schema through `0038`.
- The manual `0036` recovery was schema-equivalent to the intended migration
  plus child-reference preservation. It should not be repeated blindly; future
  table rebuild migrations need to account for inbound foreign keys.
- The customer-order active read can now join `site_projects`,
  `site_deployments`, and `adjutant_adjustment_requests` without storage errors.
- Completed-onboarding users now have corresponding active `software_orders`
  rows; future environments will get the same idempotent backfill through
  migration `0039`.
- The active-order endpoint now has a defensive fallback for storage failures.
  This avoids showing raw `storage_error` on the homepage, but it also means a
  future storage regression may appear as a missing order unless operators check
  the redacted `customer_order_active_storage_fallback` log.
- Any code deployed before migrations are applied can produce redacted storage
  errors even when the app shell and session endpoint look healthy.

## Future Notes

- Treat `storage_error` on authenticated startup as a schema-ledger check first:

```sh
bunx wrangler d1 migrations list openagents-autopilot --remote
```

- If deployed code references a new table, verify it directly:

```sh
bunx wrangler d1 execute openagents-autopilot --remote \
  --command "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
```

- Before deploying Worker code that depends on new D1 tables, apply migrations
  or make the new reads tolerate absent tables until migrations are complete.
- When a query starts joining tables, qualify overlapping column names in the
  SELECT list. In particular, prefer `software_orders.id AS id`,
  `software_orders.status AS status`, and other explicit table-qualified order
  columns in joined customer-order queries.
- Table rebuild migrations must search for inbound foreign keys first:

```sql
SELECT name, sql
FROM sqlite_master
WHERE sql LIKE '%team_chat_messages%';
```

- If a migration needs to rebuild a parent table with inbound references, either
  rebuild dependent child tables in the same migration or design the migration
  as additive so it avoids `ALTER TABLE ... RENAME` / `DROP TABLE old`.
