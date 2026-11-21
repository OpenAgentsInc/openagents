# Plan 003: The API accepts only tips and prices that the wallet is able to pay out

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report. Do not improvise. When done, update the status row for this plan
> in `plans/README.md`, unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 76084e17a9..HEAD -- coax/web/api/makers.c coax/web/api/spark/wallet.cjs coax/web/api/test_makers_tips.py coax/web/api/test_upgrades.py docs/metaverse/makers.md`
> If any of these changed, compare the "Current state" excerpts against the
> live code. On a mismatch, STOP.

## Status

- **Priority**: P3 (maker tips and upgrades are not a live feature and are not part of Everglade; do this before that feature launches)
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `76084e17a9`, 2026-11-09

## Why this matters

A player can tip a realm's maker up to 100,000 sats, and a maker can price
an upgrade up to 100,000 sats. The wallet service that pays the maker
refuses any single payout above 20,000 sats. So a tip between 20,001 and
100,000 sats is taken from the player and settled, and then the payout to
the maker fails on every attempt. After 30 attempts the payout is never
tried again. The player has paid and the maker receives nothing.

The first tip to a maker also carries a 1,000-sat bonus, so a first tip of
19,001 to 20,000 sats fails the same way.

This plan lowers the default limits so the API refuses, up front, an amount
the wallet cannot pay out.

## Current state

Files:

- `coax/web/api/makers.c`: the tip and upgrade routes and the payout runner.
- `coax/web/api/spark/wallet.cjs`: the wallet service. Read it; do not
  change it.
- `coax/web/api/migrations/0073_maker_tips.sql`: `makers.tip_terms` and
  `makers.payout_next`.
- `coax/web/api/migrations/0077_fairy_upgrades.sql`: `makers.listing_new`.
- `docs/metaverse/makers.md`: the product description, which names these
  settings.

The limits the API accepts, `coax/web/api/makers.c:216` and `:479`:

```c
  const char *v[4] = {slug, body, env_or("OA_TIP_MIN_SATS", "10"), env_or("OA_TIP_MAX_SATS", "100000")};
```

```c
    const char *v[4] = {who, body, env_or("OA_UPGRADE_MIN_SATS", "10"), env_or("OA_UPGRADE_MAX_SATS", "100000")};
```

The bonus, `coax/web/api/makers.c:252`:

```c
    const char *sv[5] = {hash, state, *amount ? amount : NULL, pre, env_or("OA_MAKER_FIRST_TIP_BONUS_SATS", "1000")};
```

The wallet's limit, `coax/web/api/spark/wallet.cjs:31` and `:100`:

```js
const MAX_PAY = Number(process.env.SPARK_MAX_PAY_SATS || 20000), DAY_PAY = Number(process.env.SPARK_DAY_PAY_SATS || 60000);
```

```js
  if (!Number.isSafeInteger(q.sats) || q.sats < 1 || q.sats > MAX_PAY) return [400, { error: "sats" }];
```

What a payout sends, from `makers.queue_payout` in `0073_maker_tips.sql`:
`sats` is the tip plus the bonus. That is the number the wallet's `/quote`
checks against `MAX_PAY`.

The retry limit, `0073_maker_tips.sql:152`: `payout_next` selects only rows
with `attempts < 30`.

The header comment that documents the settings, `coax/web/api/makers.c:24-28`:

```c
// gets the tip less the routing fee; the first player tip to each maker also carries
// OA_MAKER_FIRST_TIP_BONUS_SATS (1,000; 0 ends the launch bonus).
//   OA_TIP_MIN_SATS, OA_TIP_MAX_SATS   a tip's bounds (10, 100,000)
//   OA_MAKER_DAY_PAYOUT_SATS           what payouts may send in a Chicago day (50,000)
```

None of `OA_TIP_MAX_SATS`, `OA_UPGRADE_MAX_SATS`, `SPARK_MAX_PAY_SATS` or
`OA_MAKER_FIRST_TIP_BONUS_SATS` is set by `coax/web/api/deploy.sh` (its
environment line is `deploy.sh:78`), so production runs on these defaults.

The error text a player sees for an out-of-range tip, from
`makers.tip_terms`: `a tip is %s to %s sats`. For a price, from
`makers.listing_new`: `a price is %s to %s sats`. Both take the limits as
arguments, so changing the defaults in C changes the messages too.

The decision this plan implements: lower the two defaults. A tip is at most
19,000 sats (20,000 less the 1,000 bonus). A price is at most 20,000 sats (a
purchase carries no bonus: `makers.buy_settle` passes a bonus of 0).

## Commands you will need

| Purpose | Command | Expected on success |
| --- | --- | --- |
| The API test suite | `coax/web/api/test.sh` | last line `api: checks passed`, exit 0 |

`test.sh` runs only on a Mac with Homebrew `postgresql@16`, `secp256k1` and
`openssl@3`. It takes about 11 minutes. Do not set `API_TESTS_ONLY`; a
subset fails with a `NameError`.

## Scope

**In scope**:

- `coax/web/api/makers.c` (two default strings and the header comment)
- `coax/web/api/test_makers_tips.py` (append checks at the end)
- `coax/web/api/test_upgrades.py` (append one check at the end)
- `docs/metaverse/makers.md` (the stated limits, if it states them)
- `plans/README.md` (status row only)

**Out of scope**:

- `coax/web/api/spark/wallet.cjs`. Raising the wallet's limit is a risk
  decision for the owner, not part of this plan.
- Any migration. No SQL changes are needed.
- `coax/web/api/deploy.sh`.
- The day cap (`OA_MAKER_DAY_PAYOUT_SATS`) and the retry limit.

## Git workflow

- Work in a worktree: `git worktree add ../openagents-plan-003 origin/main`.
  This repository forbids branches other than `main` and edits in the main
  checkout.
- One commit, for example:
  `Makers: a tip is at most 19,000 sats and a price 20,000, what the wallet pays out`.
- Do not push unless the operator tells you to.

## Steps

### Step 1: Write the failing checks

Append to the end of `coax/web/api/test_makers_tips.py`. The file has
`tw_post(path, obj, keyfile=None)`, which returns `(status, json)`, and
`check(name, ok, detail)`. An existing call to model on, at line 60:
`s, b = tw_post("/api/playtest/makers/belt-2200/tip", {"sats": 100})`.

Add two checks:

- a tip of `19001` sats to `belt-2200` returns status 400 and an `error`
  that contains `19000`;
- a tip of `19000` sats returns status 200.

Name them, for example, `"tips: a tip above what the wallet pays out is
refused"` and `"tips: the largest tip is accepted"`.

The accepted 19,000-sat tip leaves an unpaid invoice row. That is the same
state as other unpaid tips the suite creates, and needs no cleanup.

Append one check to the end of `coax/web/api/test_upgrades.py`. Near the top
of that file, `fairy` is a listing dict with `"sats": 500`, and line 35
posts a variant with
`up_call("POST", "/api/playtest/upgrades/fairy", dict(fairy, look="999999999"), keyfile=up_maker_key)`.
Post `dict(fairy, sats=20001)` the same way and assert status 400 with an
`error` that contains `20000`. A signed request is unique for one second
and its body; if the post before yours used the same key in the same second
with the same body, add `time.sleep(1.1)` before it, as line 127 does.

**Verify**: run `coax/web/api/test.sh`. It must FAIL with exactly these two
new `FAIL` lines: the 19,001-sat tip check and the 20,001-sat price check.
The 19,000-sat check passes already.

### Step 2: Lower the defaults

In `coax/web/api/makers.c`:

- line 216: change `env_or("OA_TIP_MAX_SATS", "100000")` to
  `env_or("OA_TIP_MAX_SATS", "19000")`;
- line 479: change `env_or("OA_UPGRADE_MAX_SATS", "100000")` to
  `env_or("OA_UPGRADE_MAX_SATS", "20000")`;
- in the header comment, change `a tip's bounds (10, 100,000)` to
  `a tip's bounds (10, 19,000: the wallet pays out at most 20,000, and a first tip carries the bonus)`,
  and add a line for the price bounds:
  `//   OA_UPGRADE_MIN_SATS, OA_UPGRADE_MAX_SATS   a price's bounds (10, 20,000)`.

**Verify**: `grep -c '"100000"' coax/web/api/makers.c` prints `0`.

### Step 3: Update the product doc

Run `grep -n '100,000\|100000' docs/metaverse/makers.md`. For each line that
states the tip or price maximum, change it to the new number. If the command
prints nothing, make no change to that file.

**Verify**: `grep -n '100,000\|100000' docs/metaverse/makers.md` prints nothing.

### Step 4: Run the suite

**Verify**: `coax/web/api/test.sh` ends with `api: checks passed` and exits
0.

## Test plan

- Three new checks, described in step 1, modelled on
  `test_makers_tips.py:60` and `test_upgrades.py:35`.
- The existing tip and upgrade checks use 50 to 1,100 sats and must still
  pass.

## Done criteria

All must hold:

- [ ] `coax/web/api/test.sh` exits 0 and prints `api: checks passed`
- [ ] The output contains `ok    tips: a tip above what the wallet pays out is refused`
- [ ] `grep -c '"100000"' coax/web/api/makers.c` prints `0`
- [ ] `git diff --name-only origin/main` lists only files from the in-scope
      list
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back if:

- `coax/web/api/spark/wallet.cjs:31` no longer has a default of 20000 for
  `SPARK_MAX_PAY_SATS`. The numbers in this plan depend on it.
- `coax/web/api/makers.c:252` no longer has a default bonus of `"1000"`.
- An existing test tips or prices something above the new limits and fails.
  Report the check name; do not change that test's amounts without asking.
- `test.sh` fails in a file you did not touch, twice in a row with the same
  failure.

## Maintenance notes

- **Payouts that are already stuck.** This plan stops new ones. Someone with
  production database access should run
  `select tip, maker, sats, state, attempts, error from makers.payouts where state <> 'sent' and sats > 20000`
  and decide how to pay each maker, for example by an admin gift in parts.
- The three numbers (tip maximum, bonus, wallet maximum) live in two
  services and are tied only by this plan's arithmetic. If the owner changes
  `SPARK_MAX_PAY_SATS` or the bonus, the tip maximum must follow. A startup
  check that reads the wallet's limit from its `/health` answer would make
  that automatic; it is not in this plan.
- The wallet's error text is dropped on a refused quote
  (`coax/web/api/inference.c:159` frees the body on any non-200 answer), so
  a payout row records a generic reason. Keeping that text would make a
  future mismatch visible in the admin's payout list.
- Related, not fixed here: `makers.payout_next` counts a re-poll of a pending
  payment as an attempt, and a payout that reaches 30 attempts is never
  shown as abandoned.
