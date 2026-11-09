# Plan 004: The payout runner's low-balance, day-cap, expired-invoice and pending branches have tests

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report. Do not improvise. When done, update the status row for this plan
> in `plans/README.md`, unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 76084e17a9..HEAD -- coax/web/api/makers.c coax/web/api/test_inference.py coax/web/api/test_makers_tips.py coax/web/api/migrations/0073_maker_tips.sql`
> Plans 002 and 003 change `test_upgrades.py`, `test_makers_tips.py` (appended
> checks) and two default strings in `makers.c`. Those changes are expected.
> If `run_payouts` in `makers.c` or `makers.payout_next` changed in another
> way, compare the "Current state" excerpts against the live code. On a
> mismatch, STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW (tests only)
- **Depends on**: plans/002-serialize-credit-purchases-and-the-first-tip-bonus.md, plans/003-accept-only-tips-the-wallet-can-pay-out.md (they append to the same test files; land them first to avoid conflicts)
- **Category**: tests
- **Planned at**: commit `76084e17a9`, 2026-11-09

## Why this matters

The payout runner sends real Bitcoin from the company wallet to realm
makers. Its tests cover the main path: a tip is paid out once, a failed send
is retried, and a lost answer does not cause a second send. The branches
that decide what happens when the wallet is short of funds, when the day's
limit is reached, when a maker's invoice has expired and when a payment is
still pending have no test. Today those branches are checked only in
production. This plan adds tests for them. It changes no product code.

## Current state

Files:

- `coax/web/api/makers.c`: `run_payouts` (lines 127-195) is the runner.
- `coax/web/api/migrations/0073_maker_tips.sql`: `makers.payout_next`
  (day cap and lease), `makers.defer_bonus`, the `makers.tips` and
  `makers.payouts` tables.
- `coax/web/api/test_inference.py`: defines the fake wallet server that the
  API talks to in tests.
- `coax/web/api/test_makers_tips.py`: the payout tests and their fake wallet
  handlers.

The runner asks the wallet for its balance before quoting,
`coax/web/api/makers.c:151-169`:

```c
      long st0 = api_spark("/health", NULL, 20, &w);
      char bal[24] = "";
      if (st0 == 200) jget(db, w, "balance_sat", bal, sizeof bal);
      free(w); w = NULL;
      long need = atol(sats) + atol(sats) / 50 + 10;
      if (st0 == 200 && *bal && atol(bal) < need) {
        const char *dv[1] = {tip};
        PGresult *dr = PQexecParams(db, "SELECT makers.defer_bonus($1)", 1, NULL, dv, NULL, NULL, 0);
        ...
        need = atol(sats) + atol(sats) / 50 + 10;
        if (atol(bal) < need) {
          snprintf(err, sizeof err, "our wallet holds %s sats; this payout needs %ld", bal, need);
          outcome(db, tip, "failed", NULL, err, false);
          continue;
        }
```

So a payout of `S` sats needs a balance of `S + S/50 + 10`. When the balance
is short and the payout carries a first-tip bonus, `makers.defer_bonus`
splits the bonus into its own later payout whose `tip` is `'bonus:' ||
md5(tip)`, and the runner retries with the tip alone.

The fake wallet answers `/health` with an error today,
`coax/web/api/test_inference.py:117-121`:

```python
    def do_GET(self):
        wallet["asked"].append((self.path, self.headers.get("Authorization")))
        if self.path == "/rate" and wallet["rate"]:
            return self.reply(200, {"usd_per_btc": wallet["rate"]})
        self.reply(502, {"error": "the wallet failed"})
```

Because `/health` is never 200, the balance branch never runs in tests.

An earlier invoice is asked about first, `coax/web/api/makers.c:142-150`:

```c
    if (inv[0]) {   // an invoice kept before: what became of it?
      snprintf(body, sizeof body, "{\"invoice\":\"%s\"}", inv);
      long st = api_spark("/sent", body, 40, &w);
      ...
      if (!strcmp(state, "completed")) { outcome(db, tip, "sent", fee, NULL, false); continue; }
      if (!strcmp(state, "pending")) { outcome(db, tip, "pending", NULL, NULL, false); continue; }
      if (!strcmp(state, "failed") || expired) inv[0] = 0;   // a new invoice: this one can no longer be paid
    }
```

`expired` is true when the row's `invoice_expires` is more than 10 minutes
in the past (`makers.payout_next` computes it).

The day cap, `coax/web/api/migrations/0073_maker_tips.sql:155-159`:

```sql
  SELECT coalesce(sum(sats), 0) INTO spent FROM makers.payouts
   WHERE tip <> p.tip AND (state IN ('sending', 'quoted') OR (state = 'sent' AND sent >= date_trunc('day', now() AT TIME ZONE 'America/Chicago') AT TIME ZONE 'America/Chicago'));
  IF spent + p.sats > day_cap THEN
    UPDATE makers.payouts SET next_try = now() + interval '1 hour', error = 'the day''s payouts are used up' WHERE tip = p.tip;
    RETURN NULL;
  END IF;
```

The cap defaults to 50,000 sats (`makers.c:129`). The test server does not
set `OA_MAKER_DAY_PAYOUT_SATS`.

The test helpers in `coax/web/api/test_makers_tips.py`:

```python
tw = {"quotes": [], "sends": [], "paid": {}, "send_mode": ["completed"]}   # send_mode: what each /send does, in turn
...
wallet["extra"] = {"/quote": tw_quote, "/send": tw_send, "/sent": tw_sent}
...
def tw_wait(cond, seconds=8):
...
def tw_payout(tip):
    return sql(f"select state || '|' || sats || '|' || bonus_sats from makers.payouts where tip = '{tip}'")
```

- `tw_send` reads the next mode from `tw["send_mode"]`. Modes in use:
  `completed`, `pending`, `failed` and `lost`.
- `tw_sent` answers with the state recorded in `tw["paid"]` for that
  invoice, or `none`.
- The runner is started with
  `tw_post("/api/admin/makers/payouts/run", {}, keyfile=os.environ["ADMIN_KEY"])`
  (see `test_makers_tips.py:103`). It runs in a child process, so a test
  waits for the result with `tw_wait`.
- The file's last lines remove its handlers from `wallet["extra"]`. Your new
  tests must run before that removal: insert them just above it.

The table columns you need for direct inserts
(`0073_maker_tips.sql:13-48`): `makers.tips (payment_hash, realm, maker,
lightning, sats, expires, paid)` and `makers.payouts (tip, realm, maker,
lightning, sats, bonus_sats, state, invoice, invoice_expires, next_try,
sent)`. `payouts.tip` references `tips.payment_hash`.

Conventions:

- Add no new `.py` file. Extend the two existing files.
- The test driver `exec`s every `test_*.py` into one namespace in name
  order. `test_inference.py` runs before `test_makers_tips.py`.
- Do not use `time.sleep` to wait for a result. Use `tw_wait(lambda: ...)`.

## Commands you will need

| Purpose | Command | Expected on success |
| --- | --- | --- |
| The API test suite | `coax/web/api/test.sh` | last line `api: checks passed`, exit 0 |

`test.sh` runs only on a Mac with Homebrew `postgresql@16`, `secp256k1` and
`openssl@3`. It takes about 11 minutes. Do not set `API_TESTS_ONLY`; a
subset fails with a `NameError`.

## Scope

**In scope**:

- `coax/web/api/test_inference.py` (the fake wallet's `do_GET` only)
- `coax/web/api/test_makers_tips.py` (new tests above the final cleanup)
- `plans/README.md` (status row only)

**Out of scope**:

- `coax/web/api/makers.c`, every migration and
  `coax/web/api/spark/wallet.cjs`. This plan changes no product code. If a
  new test shows a bug, that is a STOP condition.
- The other `test_*.py` files.

## Git workflow

- Work in a worktree: `git worktree add ../openagents-plan-004 origin/main`.
  This repository forbids branches other than `main` and edits in the main
  checkout.
- One commit, for example:
  `Makers: tests for a short wallet, the day's cap, an expired invoice and a pending payout`.
- Do not push unless the operator tells you to.

## Steps

### Step 1: Let the fake wallet answer /health

In `coax/web/api/test_inference.py`, in `FakeWallet.do_GET`, add a branch
before the final 502 reply:

```python
        if self.path == "/health" and wallet.get("balance_sat") is not None:
            return self.reply(200, {"balance_sat": wallet["balance_sat"]})
```

`wallet["balance_sat"]` is unset by default, so every existing test still
gets the 502 and behaves as before.

**Verify**: `coax/web/api/test.sh` ends with `api: checks passed`. Nothing
else has changed yet, so this confirms the new branch is inert.

### Step 2: Add a helper that queues one payout directly

In `coax/web/api/test_makers_tips.py`, above the file's final cleanup, add a
helper that inserts one paid tip and its queued payout with plain SQL and
returns the tip id:

```python
def tw_queue(tip, sats, bonus=0, maker="c0" * 32, **cols):
    sql(f"insert into makers.tips (payment_hash, realm, maker, lightning, sats, expires, paid) "
        f"values ('{tip}', 'belt-2200', '{maker}', 'payee@example.com', {sats - bonus}, now(), now())")
    extra = "".join(f", {k}" for k in cols), "".join(f", {v}" for v in cols.values())
    sql(f"insert into makers.payouts (tip, realm, maker, lightning, sats, bonus_sats{extra[0]}) "
        f"values ('{tip}', 'belt-2200', '{maker}', 'payee@example.com', {sats}, {bonus}{extra[1]})")
    return tip
```

Values in `cols` are SQL text, for example `state="'quoted'"`.

Also add `tw_run()`, a one-line wrapper around the existing
`tw_post("/api/admin/makers/payouts/run", {}, keyfile=os.environ["ADMIN_KEY"])`.
A signed request is unique for one second and its body. If two runs follow
within a second, the second is refused as a replay. Look at how lines
103-118 space their two runs and do the same.

**Verify**: none yet. The helper is used in step 3.

### Step 3: Add the five cases

Add each case as a short block with `check(...)` calls. Give each its own
tip ids so the cases cannot affect each other. After each case, set
`wallet["balance_sat"] = None` and `tw["send_mode"] = ["completed"]` again.

**Case 1, a short wallet defers the bonus.** Queue `short-1` with
`sats=1100, bonus=1000`. Set `wallet["balance_sat"] = 500`. Run. Expect:

- `tw_wait(lambda: tw_payout("short-1").startswith("sent|100|0"))`;
- a second payout row exists whose `tip` starts with `bonus:`, for the same
  maker, with `sats = 1000`, not sent, and `next_try` in the future:
  `select count(*) from makers.payouts where tip like 'bonus:%' and maker = '<maker>' and state <> 'sent' and next_try > now()`
  is `1`.

Then mark that bonus row sent so it never runs later:
`update makers.payouts set state = 'sent', sent = now() - interval '2 days' where tip like 'bonus:%' and maker = '<maker>'`.

**Case 2, a wallet too short for the tip itself.** Queue `short-2` with
`sats=400`. Set the balance to `50`. Run. Expect the row to reach state
`failed` with an `error` that starts with `our wallet holds 50 sats`, and
the number of quotes (`len(tw["quotes"])`) to be unchanged from before the
run. Then mark the row sent two days ago, as above.

**Case 3, the day's cap.** Compute the current spend with the same
expression the function uses:

```python
spent = int(sql("select coalesce(sum(sats), 0) from makers.payouts where state in ('sending', 'quoted') "
                "or (state = 'sent' and sent >= date_trunc('day', now() at time zone 'America/Chicago') at time zone 'America/Chicago')"))
```

Queue a filler `cap-fill` with `sats = 50000 - spent - 50`, `state="'sent'"`
and `sent="now()"`. Queue `cap-1` with `sats=100`. Run. Expect, within
`tw_wait`, that `cap-1` has `error = 'the day''s payouts are used up'`,
is still not `sent`, and has `next_try > now() + interval '50 minutes'`.
Expect the number of quotes to be unchanged.

Clean up, or every later payout in the suite is blocked: delete `cap-fill`
from `makers.payouts` and then from `makers.tips`, and mark `cap-1` sent two
days ago.

**Case 4, an expired invoice gets a new quote.** Queue `old-1` with
`sats=300`, `state="'quoted'"`, `invoice="'lnbc-old-1'"` and
`invoice_expires="now() - interval '20 minutes'"`. Note
`len(tw["quotes"])`. Run. `tw_sent` answers `none` for an invoice it never
saw, the row is expired, so the runner must ask for a new quote. Expect
`tw_wait(lambda: tw_payout("old-1").startswith("sent|300|0"))`, exactly one
more quote, and `'lnbc-old-1' not in tw["sends"]`.

**Case 5, a pending payment is asked about and never sent twice.** Queue
`pend-1` with `sats=200`. Set `tw["send_mode"] = ["pending"]`. Note
`len(tw["sends"])`. Run. Expect the row to have an invoice and not be
`sent`: wait for
`sql("select invoice is not null and state <> 'sent' from makers.payouts where tip = 'pend-1'") == "t"`.
Read that invoice from the row. Set `tw["paid"][invoice] = "completed"`.
Make the row due again:
`update makers.payouts set next_try = now(), lease = null where tip = 'pend-1'`.
Run again. Expect `tw_wait(lambda: tw_payout("pend-1").startswith("sent"))`
and `len(tw["sends"])` to be exactly one more than the note.

Name the checks as rules, for example:

- `tips: a wallet short of a first tip's bonus pays the tip and holds the bonus`
- `tips: a wallet short of the tip itself sends nothing`
- `tips: past the day's cap a payout waits an hour`
- `tips: an expired invoice is quoted again, and the old one is never paid`
- `tips: a pending payout is asked about, not sent again`

**Verify**: `coax/web/api/test.sh` ends with `api: checks passed`, and the
five new check names appear with `ok`.

### Step 4: Confirm the cleanup

Run the suite a second time.

**Verify**: `coax/web/api/test.sh` again ends with `api: checks passed`.
Each run uses a fresh database, so this mainly confirms that the tests are
not timing-dependent. If a check that passed in step 3 fails now, treat it
as a STOP condition.

## Test plan

This plan is the test plan. It adds at least five checks to
`test_makers_tips.py`, modelled on the existing retry checks at
`test_makers_tips.py:100-125`.

## Done criteria

All must hold:

- [ ] `coax/web/api/test.sh` exits 0 twice in a row
- [ ] The output contains all five new check names with `ok`
- [ ] `git diff --name-only origin/main` lists only
      `coax/web/api/test_inference.py`, `coax/web/api/test_makers_tips.py`
      and `plans/README.md`
- [ ] `git diff origin/main -- coax/web/api/makers.c coax/web/api/migrations`
      is empty
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back if:

- A new case fails because the product behaves differently from this plan's
  description. For example: in case 5 the runner sends the invoice a second
  time, or in case 3 a quote is requested. That is a real bug. Report the
  case, the expected and actual values, and do not change `makers.c` or the
  expectation to make it pass.
- The file's final cleanup of `wallet["extra"]` is not where this plan says,
  or `tw_send`, `tw_sent` or `tw_quote` have different signatures.
- A later test file fails after your tests are added. Your cleanup is
  probably incomplete; if you cannot find the leftover row in two attempts,
  report it.
- A case passes only with a fixed `time.sleep` longer than the 1.1 seconds
  used for signed-request spacing.

## Maintenance notes

- The lease is 150 seconds (`makers.c:132`), while the wallet timeouts in
  one iteration add up to 160 seconds (40 for `/sent`, 20 for `/health`, 40
  for `/quote`, 60 for `/send`). A second runner can therefore pick up a
  payout that the first is still sending. The wallet refuses to pay one
  invoice twice, which is what prevents a double payment. A test with two
  overlapping runs and a slow fake wallet would pin that; it was left out
  because the fake wallet may be single-threaded. Worth adding later.
- `makers.payout_next` adds one to `attempts` on every lease, including
  re-polls of a pending payment, and rows with 30 attempts are never picked
  up again. Case 5 will keep working if that is changed.
- When the runner's branches change, these cases are the place to update.
