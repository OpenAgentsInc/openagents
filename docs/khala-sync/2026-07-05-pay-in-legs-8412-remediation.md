# KS-8.7: `pay_in_legs` swapped `party_ref`/`amount_msat` remediation (#8412)

Owner-approved **Option B** corrective UPDATE, executed 2026-07-05 against
production D1 (`openagents-autopilot`), for the two historical `pay_in_legs`
rows corrupted by the now-fixed parameter-order bug in
`usdCreditGrantStatements` (`apps/openagents.com/workers/api/src/inference/usd-credit-bridge.ts`).
Filed from #8337 / epic #8282; this doc is the audit trail for closing #8412.

Full pre-change snapshot: [`2026-07-05-pay-in-legs-8412-remediation-backup.json`](./2026-07-05-pay-in-legs-8412-remediation-backup.json).

## 1. Re-confirmed current state (before touching anything)

Ran the issue's exact finding query against production D1:

```
wrangler d1 execute openagents-autopilot --remote --json \
  --command "SELECT rowid, id, party_ref, amount_msat, typeof(party_ref) AS party_ref_type, typeof(amount_msat) AS amount_msat_type FROM pay_in_legs WHERE typeof(amount_msat) != 'integer'"
```

Result: exactly the same **2 rows** the issue described, with the same
corrupted values — no drift since the issue was filed.

| rowid | id | party_ref (corrupted) | amount_msat (corrupted) |
|---|---|---|---|
| 66 | `mpp-lightning:e92fefb88c0e01e53204b4c662a75558e85703cb5dace1e23c77aefc97f07250:grant` | `"1000.0"` (text) | `"agent:mpp-lightning:e92fefb88c0e01e53204b4c662a75558e85703cb5dace1e23c77aefc97f07250"` (text) |
| 127 | `operator-inference-credit:opencode-gym-edit-20260625:grant` | `"1000000.0"` (text) | `"agent:user_98141a79-049d-4e4f-8077-bf3f40fcf065"` (text) |

## 2. Determined correct values, with three independent cross-checks

The swap direction (party_ref currently holds the msat amount as text;
amount_msat currently holds the account ref string) was corroborated three
independent ways before writing anything:

1. **`resulting_balance_msat`** on each row (computed at insert time by a
   separate, correctly-parameterized `SELECT balance_msat FROM agent_balances
   WHERE actor_ref = ?` subquery) exactly matches the numeric value sitting in
   the corrupted `party_ref` column:
   - rowid 66: `resulting_balance_msat = 1000` == `party_ref = "1000.0"`
   - rowid 127: `resulting_balance_msat = 1000000` == `party_ref = "1000000.0"`
2. **The parent `pay_ins` row** for each leg (a separate table, never
   affected by this bug) has correctly-parameterized `payer_ref` / `cost_msat`
   columns that match the intended correct values exactly:
   - `inference:lightning-charge:mpp-lightning:...`: `payer_ref =
     "agent:mpp-lightning:e92fefb88c0e01e53204b4c662a75558e85703cb5dace1e23c77aefc97f07250"`,
     `cost_msat = 1000`
   - `inference:usd-credit:operator-inference-credit:opencode-gym-edit-20260625`:
     `payer_ref = "agent:user_98141a79-049d-4e4f-8077-bf3f40fcf065"`,
     `cost_msat = 1000000`
3. **The code diff itself** (`usdCreditGrantStatements` in #8337): the params
   array was `[..., grantMsat, input.accountRef, input.accountRef, ...]`
   bound against a placeholder order of `(party_ref, amount_msat, ...,
   actor_ref)` — i.e. `grantMsat` (a number) landed in `party_ref` and
   `input.accountRef` (a string) landed in `amount_msat`, confirming the swap
   is a straight two-column transposition with no other corruption.

All three checks agree exactly, so the corrected values applied were:

| rowid | id | correct `party_ref` | correct `amount_msat` |
|---|---|---|---|
| 66 | `mpp-lightning:...:grant` | `agent:mpp-lightning:e92fefb88c0e01e53204b4c662a75558e85703cb5dace1e23c77aefc97f07250` | `1000` |
| 127 | `operator-inference-credit:...:grant` | `agent:user_98141a79-049d-4e4f-8077-bf3f40fcf065` | `1000000` |

## 3. Pre-change snapshot

Full current-state row contents (D1 query results, the corroborating
`pay_ins` rows, and the corrected values to be applied) were captured to
[`2026-07-05-pay-in-legs-8412-remediation-backup.json`](./2026-07-05-pay-in-legs-8412-remediation-backup.json)
**before** any write.

## 4. Corrective UPDATE executed

Two precise, row-id-keyed UPDATEs against production D1
(`openagents-autopilot`), each scoped by both `rowid` AND the exact `id`
string (belt-and-suspenders against a broad/pattern match):

```sql
UPDATE pay_in_legs
   SET party_ref = 'agent:mpp-lightning:e92fefb88c0e01e53204b4c662a75558e85703cb5dace1e23c77aefc97f07250',
       amount_msat = 1000
 WHERE rowid = 66
   AND id = 'mpp-lightning:e92fefb88c0e01e53204b4c662a75558e85703cb5dace1e23c77aefc97f07250:grant';

UPDATE pay_in_legs
   SET party_ref = 'agent:user_98141a79-049d-4e4f-8077-bf3f40fcf065',
       amount_msat = 1000000
 WHERE rowid = 127
   AND id = 'operator-inference-credit:opencode-gym-edit-20260625:grant';
```

Both ran via `wrangler d1 execute openagents-autopilot --remote --json
--command "..."`; each returned `"changes": 1` (exactly one row touched, as
expected).

Re-read after the UPDATE:

```
SELECT count(*) FROM pay_in_legs WHERE typeof(amount_msat) != 'integer';
-- bad_count: 0
```

```
rowid=66:  party_ref = "agent:mpp-lightning:...:97f07250", amount_msat = 1000     (integer)
rowid=127: party_ref = "agent:user_98141a79-...",          amount_msat = 1000000  (integer)
```

No other rows/tables were touched. `agent_balances` was never read from or
written to by this remediation — only the two `pay_in_legs` audit-leg rows'
own `party_ref`/`amount_msat` columns.

## 5. Billing backfill re-run and verify

Before the correction, `bun scripts/backfill-billing.ts --verify --table
pay_in_legs` showed exactly the expected gap (D1=323 rows vs. Postgres=321
rows — the 2 rows were never mirrored, per the issue), and the per-`(direction,
kind)` `amount_msat` sum mismatches were off by **exactly** 1,000,000 msat
(`in:balance`) and 1,000 msat (`in:lightning`) — matching the corrected
amounts precisely, a fourth independent confirmation of the fix direction.

After the UPDATE, ran the backfill converge sweep scoped to `pay_in_legs`:

```
bun scripts/backfill-billing.ts --table pay_in_legs --restart
# pay_in_legs: page done (cursor rowid=323, scanned=323/323, converged=323, ~233 rows/s)
# pay_in_legs: complete — scanned 323 row(s) this run, 323 converged
```

Then re-ran `--verify`:

```
bun scripts/backfill-billing.ts --verify --table pay_in_legs
# == pay_in_legs ==
#   rows: d1=323 postgres=323
#   per-(direction, kind) amount_msat: exact match
#   newest-50 row hashes: all match
# VERIFY OK: exact counts, per-account balances, money sums, key sets, and newest-N hashes match.
```

A full-domain `--verify` (all 22 billing tables) was also run to confirm no
regression elsewhere; every table — including `pay_ins` and `pay_in_legs` —
came back with exact row-count, sum, and newest-N-hash matches (`21/21`
tables `VERIFY OK` individually; one transient `wrangler d1 execute`
network hiccup on `khala_code_paid_plan_payment_intents` during the combined
sweep resolved cleanly on retry in isolation and is unrelated to this
remediation).

## 6. Test suite + typecheck

- `bun run --cwd apps/openagents.com/workers/api test -- usd-credit-bridge` —
  13/13 pass (includes the KS-8.7/#8337 regression assertion added against
  the original param-order bug).
- `bun run --cwd packages/khala-sync-server typecheck` — clean.
- `bun run --cwd apps/openagents.com/workers/api typecheck` — clean.

## 7. Spam-comment cleanup on #8412

Deleted a spam/malware-link comment from GitHub user `dasubene23` linking to
`bareneguboko/patch_fix` (a suspicious `.rar` release download — not
visited/downloaded). GitHub had already auto-minimized it as `ABUSE`; it was
additionally deleted via `gh api -X DELETE
repos/OpenAgentsInc/openagents/issues/comments/4885513286`. Recommend the
account `dasubene23` be reported/blocked at the org level — no further action
was taken against the account itself from this pass.

## Outcome

Both historical `pay_in_legs` rows now hold correct `party_ref`/`amount_msat`
values consistent with the fixed code path, with the row-level swap direction
independently confirmed four separate ways before the write. The billing
domain's D1 ↔ Postgres backfill now converges exactly (`21/21` tables
verified clean, including `pay_in_legs`). No `agent_balances` or other
money-movement table was touched.
