# Plan 002: A credit purchase and a first-tip bonus each happen once, even with concurrent requests

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report. Do not improvise. When done, update the status row for this plan
> in `plans/README.md`, unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 76084e17a9..HEAD -- coax/web/api/migrations coax/web/api/makers.c coax/web/api/test_upgrades.py coax/web/api/test_makers_tips.py`
> New migration files from other work are expected. If
> `0073_maker_tips.sql`, `0077_fairy_upgrades.sql` or a later migration that
> redefines `makers.buy_credit` or `makers.queue_payout` changed, compare the
> "Current state" excerpts against the live code. On a mismatch, STOP.

## Status

- **Priority**: P3 (maker tips and upgrades are not a live feature and are not part of Everglade; do this before that feature launches)
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `76084e17a9`, 2026-11-09

## Why this matters

Two SQL functions that move money check a condition and then act on it
without holding a lock that covers both. The API runs as up to four
instances, so two requests can run these functions at the same moment.

- `makers.buy_credit` can sell the same fairy to the same player twice. The
  player's credit is taken twice, and the maker is paid twice from our
  wallet.
- `makers.queue_payout` can grant the first-tip bonus (1,000 sats from our
  wallet) to the same maker more than once.

Each fix is one advisory lock inside the function.

## Current state

Files:

- `coax/web/api/migrations/0077_fairy_upgrades.sql`: defines
  `makers.buy_terms`, `makers.owns` and `makers.buy_credit`.
- `coax/web/api/migrations/0073_maker_tips.sql`: defines the `makers.tips`
  and `makers.payouts` tables and `makers.queue_payout`.
- `coax/web/api/test_upgrades.py`: the tests for buying upgrades.
- `coax/web/api/test.py`: the test driver. It `exec`s every `test_*.py` file
  into one namespace in sorted name order, so a later file can use an earlier
  file's helpers.

The ownership check, `0077_fairy_upgrades.sql:57-59` and `:134-143`:

```sql
CREATE FUNCTION makers.owns(pl text, l bigint) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT pl IS NOT NULL AND EXISTS (SELECT 1 FROM makers.tips t WHERE t.listing = l AND t.buyer = pl AND t.paid IS NOT NULL)
$$;
```

```sql
CREATE FUNCTION makers.buy_terms(l bigint, pl text) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
...
  IF makers.owns(pl, l) THEN RETURN '{"status":409,"error":"it is yours already"}'; END IF;
```

The purchase, `0077_fairy_upgrades.sql:199-216`:

```sql
CREATE FUNCTION makers.buy_credit(l bigint, pl text, pk text, is_team boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  terms jsonb; x makers.listings; c inference.credit; t makers.tips;
BEGIN
  terms := makers.buy_terms(l, pl);
  IF (terms->>'status')::int <> 200 THEN RETURN terms; END IF;
  IF pk IS NULL OR pk !~ '^[0-9a-f]{64}$' THEN RETURN '{"status":401,"error":"log in to pay with credit"}'; END IF;
  SELECT * INTO x FROM makers.listings WHERE id = l;
  SELECT * INTO c FROM inference.credit WHERE pubkey = pk FOR UPDATE;
  IF NOT FOUND OR c.msat < x.sats * 1000 THEN
    RETURN jsonb_build_object('status', 402, 'error', 'not enough credit', 'credit_msat', coalesce(c.msat, 0));
  END IF;
  UPDATE inference.credit SET msat = msat - x.sats * 1000, updated = now() WHERE pubkey = pk RETURNING * INTO c;
  INSERT INTO makers.tips (payment_hash, realm, maker, lightning, sats, team, expires, paid, listing, buyer, buyer_key, paid_with)
  VALUES ('credit:' || md5(random()::text || clock_timestamp()::text || pk), 'fairy-forest', x.maker, x.lightning, x.sats,
          coalesce(is_team, false), now(), now(), l, pl, pk, 'credit') RETURNING * INTO t;
  PERFORM makers.queue_payout(t, 0);
  PERFORM makers.bought(t);
  RETURN jsonb_build_object('status', 200, 'state', 'paid', 'id', l, 'look', x.look, 'credit_msat', c.msat);
END $$;
```

Why it can run twice: `buy_terms` checks ownership before any lock is held.
Request B passes that check while request A has not yet committed. B then
waits on the `FOR UPDATE` row lock. When A commits, B reads the new, lower
balance, and if it still covers the price, B takes the credit again and
inserts a second paid row. The index on `(listing, buyer)` is not unique
(`0077_fairy_upgrades.sql:36`), so nothing stops the second row.

The bonus, `0073_maker_tips.sql:95-104`:

```sql
CREATE FUNCTION makers.queue_payout(t makers.tips, bonus bigint) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE
  b bigint := 0;
BEGIN
  IF NOT t.gift AND coalesce(bonus, 0) > 0 AND NOT EXISTS (
       SELECT 1 FROM makers.payouts p WHERE p.maker = t.maker AND p.bonus_sats > 0) THEN
    b := bonus;
  END IF;
  INSERT INTO makers.payouts (tip, realm, maker, lightning, sats, bonus_sats) VALUES (t.payment_hash, t.realm, t.maker, t.lightning, t.sats + b, b);
  RETURN b;
END $$;
```

Two first tips to one maker, settled at the same moment in two transactions,
each see no earlier bonus row and both get the bonus.

Conventions:

- **Never edit an existing migration.** Add a new file
  `coax/web/api/migrations/NNNN_name.sql` with the next free number. Run
  `ls coax/web/api/migrations | tail -3` to find it. At planning time the
  last was `0137_analytics_team_links.sql`. Other agents add migrations
  often, so check again just before you commit.
- Change a function with `CREATE OR REPLACE FUNCTION`, keeping its
  signature. An existing example is
  `coax/web/api/migrations/0084_maker_unclaimed.sql`, which begins with a
  comment that says what the migration does and then redefines `makers.of`.
- `test.py` asserts that every file in `migrations/` is applied once, so a
  new migration is exercised by every test run.
- Add no new `.py` file. Extend `test_upgrades.py`.

## Commands you will need

| Purpose | Command | Expected on success |
| --- | --- | --- |
| The API test suite | `coax/web/api/test.sh` | last line `api: checks passed`, exit 0 |
| Next migration number | `ls coax/web/api/migrations \| tail -3` | the highest existing number |

`test.sh` runs only on a Mac with Homebrew `postgresql@16`, `secp256k1` and
`openssl@3`. It starts a throwaway Postgres and takes about 11 minutes. Do
not set `API_TESTS_ONLY`: the test files depend on each other and a subset
fails with a `NameError`.

## Scope

**In scope**:

- `coax/web/api/migrations/NNNN_makers_once.sql` (create; `NNNN` is the next
  free number)
- `coax/web/api/test_upgrades.py` (append tests at the end)
- `plans/README.md` (status row only)

**Out of scope**:

- `0073_maker_tips.sql`, `0077_fairy_upgrades.sql` and every other existing
  migration.
- `coax/web/api/makers.c`. The C code calls these functions and needs no
  change.
- A unique index on `makers.tips (listing, buyer)`. Existing duplicate rows
  in production would make the migration fail at startup and stop the API.
  See "Maintenance notes".
- The payout runner, the day cap and the retry count. Plan 004 covers their
  tests.

## Git workflow

- Work in a worktree: `git worktree add ../openagents-plan-002 origin/main`.
  This repository forbids branches other than `main` and edits in the main
  checkout.
- One commit, for example:
  `Makers: a credit purchase and the first-tip bonus hold a lock, so each happens once`.
- Do not push unless the operator tells you to.

## Steps

### Step 1: Write the failing tests

Append to the end of `coax/web/api/test_upgrades.py`. The file already has
`up_id` (an approved listing with a price of 500 sats), `up_mint()` (makes a
player and returns a dict with `id` and `key`), `sql(q)` and `check(name,
ok, detail)`. `PSQL` is a list: the `psql` binary, the database URL and
`-Atc`. `psql -c` runs several statements separated by `;` in one
transaction, which is how the tests hold a transaction open.

Add `import threading` at the top of your appended block.

**Test A, a credit purchase.** Create a new player with `up_mint()`. Insert a
credit row for a new 64-hex public key with `2000000` msat, enough for four
purchases. Then:

1. Start a thread that runs one `psql` call with two statements:
   `select makers.buy_credit(<up_id>, '<player id>', '<pk>', false); select pg_sleep(1.5)`.
2. Sleep 0.4 seconds in the main thread.
3. In the main thread, run `select makers.buy_credit(<up_id>, '<player id>', '<pk>', false)->>'status'`
   and keep the result.
4. Join the thread.

Assert with `check`:

- the main thread's status is `409`;
- `select count(*) from makers.tips where listing = <up_id> and buyer = '<player id>' and paid is not null` is `1`;
- `select msat from inference.credit where pubkey = '<pk>'` is `1500000`.

**Test B, the bonus.** Use a maker public key that has no payouts: any new
64-hex string, for example `'b0' * 32`. Insert two paid tip rows for it
directly:

```sql
insert into makers.tips (payment_hash, realm, maker, lightning, sats, expires, paid)
values ('race-a', 'belt-2200', '<maker>', 'race@example.com', 100, now(), now()),
       ('race-b', 'belt-2200', '<maker>', 'race@example.com', 100, now(), now())
```

Then, with the same thread pattern:

1. Thread: `select makers.queue_payout(t, 1000) from makers.tips t where payment_hash = 'race-a'; select pg_sleep(1.5)`.
2. Sleep 0.4 seconds.
3. Main thread: `select makers.queue_payout(t, 1000) from makers.tips t where payment_hash = 'race-b'`.
4. Join.

Assert that
`select count(*) from makers.payouts where maker = '<maker>' and bonus_sats > 0`
is `1`.

After test B, stop the two payouts from being sent by the background runner,
so they cannot disturb later test files:
`update makers.payouts set state = 'sent', sent = now() where tip in ('race-a', 'race-b')`.
Do the same for the tip row that test A created
(`... where tip in (select payment_hash from makers.tips where buyer = '<player id>')`).

Name the checks so they read as rules, for example
`"upgrades: two credit purchases at once sell it once"` and
`"tips: two first tips at once carry one bonus"`.

**Verify**: run `coax/web/api/test.sh`. It must FAIL, and the output must
contain `FAIL  upgrades: two credit purchases at once sell it once` and
`FAIL  tips: two first tips at once carry one bonus`. Every other check must
still print `ok`. This proves the tests detect the bug.

### Step 2: Add the migration

Create `coax/web/api/migrations/NNNN_makers_once.sql`. Begin with a comment
of two or three lines that says what it does. Then redefine both functions
with `CREATE OR REPLACE FUNCTION`, copying each body exactly from "Current
state" and adding one line to each:

In `makers.buy_credit`, as the first statement after `BEGIN`:

```sql
  PERFORM pg_advisory_xact_lock(hashtext('makers.buy:' || coalesce(pl, '') || ':' || l::text));
```

In `makers.queue_payout`, as the first statement after `BEGIN`:

```sql
  IF NOT t.gift AND coalesce(bonus, 0) > 0 THEN
    PERFORM pg_advisory_xact_lock(hashtext('makers.bonus:' || t.maker));
  END IF;
```

`pg_advisory_xact_lock` is released when the transaction ends. With it, the
second request waits until the first commits and then sees the first one's
row.

**Verify**: `grep -c 'pg_advisory_xact_lock' coax/web/api/migrations/NNNN_makers_once.sql`
prints `2`.

### Step 3: Run the suite

**Verify**: `coax/web/api/test.sh` ends with `api: checks passed` and exits
0. The two new checks print `ok`.

## Test plan

- Two new checks in `test_upgrades.py`, described in step 1. They are
  modelled on the existing credit checks at `test_upgrades.py:119-133`.
- The existing checks `upgrades: bought with credit, the price comes out of
  it` and `tips: the first tip carries the first-paid bonus` must still pass.
  They cover the single-request path.

## Done criteria

All must hold:

- [ ] `coax/web/api/test.sh` exits 0 and prints `api: checks passed`
- [ ] The output contains `ok    upgrades: two credit purchases at once sell it once`
- [ ] The output contains `ok    tips: two first tips at once carry one bonus`
- [ ] `git diff --name-only origin/main` lists exactly the new migration,
      `coax/web/api/test_upgrades.py` and `plans/README.md`
- [ ] `git diff origin/main -- coax/web/api/migrations/0073_maker_tips.sql coax/web/api/migrations/0077_fairy_upgrades.sql`
      is empty
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back if:

- A migration later than `0077` already redefines `makers.buy_credit` or
  `makers.queue_payout`
  (`grep -ln 'FUNCTION makers.buy_credit\|FUNCTION makers.queue_payout' coax/web/api/migrations/*.sql`
  lists more than `0073_maker_tips.sql` and `0077_fairy_upgrades.sql`). Copy
  the body from the newest definition instead only if you are certain it is
  the newest; otherwise stop.
- In step 1 the new tests pass before the migration exists. The tests are
  then not reproducing the bug; do not continue.
- `test.sh` fails in a test file you did not touch, twice in a row with the
  same failure. Report the failing check names.
- The machine has no Homebrew `postgresql@16`. `test.sh` cannot run there.

## Maintenance notes

- Purchases paid in sats go through `makers.buy_settle` and
  `makers.tip_settle`, which lock the tip row and use a replay ledger. They
  are not changed here. A player could still pay two separate invoices for
  the same fairy; that is a different case with two real payments.
- A unique index on `makers.tips (listing, buyer) WHERE paid IS NOT NULL AND
  paid_with = 'credit'` would make the database enforce the rule. Before
  adding it, someone with production access must check for existing
  duplicates:
  `select listing, buyer, count(*) from makers.tips where paid is not null and paid_with = 'credit' group by 1, 2 having count(*) > 1`.
- The same query answers whether any player was already charged twice and
  is owed a refund.
- A reviewer should confirm that the two function bodies in the migration
  differ from the originals only by the added lock lines.
