# 31. Ship it

> **Status: partly implemented.** The single release-set orchestrator and its
> `#authority` policy live in
> [`coax/web/release-set.coax`](../../coax/web/release-set.coax) and
> [`coax/web/release-set-policy.coax`](../../coax/web/release-set-policy.coax)
> ([`docs/deployment/release-set.md`](../../docs/deployment/release-set.md)),
> backed by [`coax/web/api/deploy.sh`](../../coax/web/api/deploy.sh),
> [`coax/web/room/vm-deploy.sh`](../../coax/web/room/vm-deploy.sh),
> [`coax/web/release.sh`](../../coax/web/release.sh), and pre-release fault
> qualification in
> [`coax/web/room/mmo-fault-policy.coax`](../../coax/web/room/mmo-fault-policy.coax),
> exercised by [`everglade/examples/release-set-gates.coax`](../examples/release-set-gates.coax).
> Unattended zero-human production promotion and automated schema roll-downs
> remain out of scope (production promotion is owner-authorized and database
> migrations stay forward-compatible).

This chapter answers the operator's question: "How do I ship a change to the
API, six room shards, and the browser page in one command—and pull it back in
seconds if the canary slows down?"

An MMORPG has three moving parts: the HTTP API, the authoritative room servers,
and the WebAssembly page. Shipping them out of order—especially pushing a new
page before every room shard speaks its wire version and `(engine, rules,
content)` pins—strands every player in a reload loop (`#13163`, `#13387`).

## What this chapter teaches

- Why every release moves in one fixed order (**API → rooms → page**) and rolls
  back in exact reverse (**page → rooms → API**).
- How wire ranges and dual-build pin windows (`own` + `before`) keep live
  players connected across room promotions.
- How staging locks and pinned `--release NAME` promotions prevent promoting a
  build staging no longer runs.
- How the canary compares one upgraded shard against its peers so only
  regressions caused by the release stop the rollout.

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| Release-set policy (`#authority`) | Enforces deploy/rollback order, wire & 3-pin gates, staging lock, pinned promotion, and 7-bit canary verdict | [`coax/web/release-set-policy.coax`](../../coax/web/release-set-policy.coax) |
| Release-set CLI | `coax/web/tool.sh release-set ship\|rollback\|status\|page-wire` (`--staging` or production) | [`coax/web/release-set.coax`](../../coax/web/release-set.coax), [`docs/deployment/release-set.md`](../../docs/deployment/release-set.md) |
| Part deploy scripts | Deploys API (`deploy.sh`), room VM canary/fleet/rollback (`vm-deploy.sh`), and web pages (`release.sh`) | [`coax/web/api/deploy.sh`](../../coax/web/api/deploy.sh), [`coax/web/room/vm-deploy.sh`](../../coax/web/room/vm-deploy.sh), [`coax/web/release.sh`](../../coax/web/release.sh) |
| Fault qualification gate | Four bounded `mmo-fault-v2` slices (`durable`, `overlap`, `startup`, `isolation`); fails closed on timeout or truncation | [`coax/web/room/mmo-fault-policy.coax`](../../coax/web/room/mmo-fault-policy.coax) |

**What remains to ship:** Staging deploys, staging bot canaries, pin/wire gates,
staging locks, production single-shard canaries, and single-command rollbacks
all run today through `release-set`. Production promotion still requires an
explicit owner-authorized invocation, and database migrations are intentionally
forward-only (each API release keeps schema changes readable by the previous
revision so API rollback never drops data).

## The method

### One order forward, exact reverse backward

`release-set-policy.coax` tracks moved parts in a 3-bit mask (`1` API, `2`
rooms, `4` page). `rs_next` insists on `RS_API` (0) → `RS_ROOMS` (1) → `RS_PAGE`
(2) → `RS_DONE` (3); `rs_may` refuses skipping ahead. `rs_back` unwinds in exact
reverse—pulling the page back first, then the rooms, then the API—so players
never load a page that speaks a wire version the rooms no longer accept.

### Pins beside the live page and staging locks

Every room speaks a range `wire_lo..wire_hi` and seats two `(engine, rules,
content)` pin triples: its own build and the release immediately before it
(`rs_pins_beside`, `#13408`). Before a page ships, `rs_pins_ship` and
`rs_wire_ship` verify that every live shard speaks its wire version and matches
its pins; a single lagging shard blocks the page.

To prevent two concurrent deploys from stepping on each other on staging,
`release-set ship` holds `/opt/oa-room/staging.lock` (`rs_lock`). Another
release is refused (`RS_L_HELD`), while an automated nightly soak automatically
yields its lock to a release (`rs_lock_yields`, `#13384`). Promotion then
passes `--release NAME` explicitly: `rs_promote` refuses any mismatch against
what staging currently runs (`RS_P_MOVED`) and refuses seeded fault builds
(`RS_P_SEEDED`).

### Peer-relative canary verdicts

`rs_canary(canary, peers)` evaluates seven failure bits: `crashed`, `unhealthy`,
`slower-tick` (p50 or p95 over twice peers + 300 µs margin, or under 90% tick
rate), `more-refusals` (over twice peers + 1.0/s), `fewer-seats` (under 90% of
peers), `slower-saves` (checkpoint unsaved > 15 s when peers are not), and
`refuses-the-build`. Comparing against peer shards ensures ambient database
slowness never triggers a false rollback, while a seeded slow tick stops the
canary immediately.

## Worked trace: release order, pins, staging lock, and canary stop

```text
1. Order:       moved=0 -> api -> moved=1 -> rooms -> moved=3 -> page -> done
                skip page at moved=1 -> refused; rollback from 7 -> page, rooms, api
2. Wire & pins: page (4,9,3, wire 6) vs 6 shards (4,9,3, wire 5..6) -> ship OK
                5 of 6 shards matching -> refused
                rooms seat own (4,9,3) or prev (3,9,2), never hybrid (3,9,3)
3. Staging:     pinned == staging -> RS_P_OK; moved -> RS_P_MOVED; -seeded -> RS_P_SEEDED
                lock held by "soak ..." -> yields to release; held by release -> blocks
4. Canary:      peers p95=600 us, good p95=640 us -> 0 (hold)
                bad: crash=1, p95=20600 us, version_refused=1
                  -> bits=RS_C_CRASH+RS_C_TICK+RS_C_VERSION ("crashed,slower-tick,refuses-the-build")
```

## Runnable example

[`everglade/examples/release-set-gates.coax`](../examples/release-set-gates.coax)
runs the forward/reverse order rules, wire & three-pin gates, staging promotion
and lock arbitration, canary regression verdict, and fail-closed fault
qualification check directly against
[`coax/web/release-set-policy.coax`](../../coax/web/release-set-policy.coax)
and [`coax/web/room/mmo-fault-policy.coax`](../../coax/web/room/mmo-fault-policy.coax)
(committed transcript
[`everglade/examples/release-set-gates.out`](../examples/release-set-gates.out)):

<!-- source: everglade/examples/release-set-gates.coax release-set-gates -->
```jai
ship_run :: () -> bool {
  // 1. Strict deploy order: API -> rooms -> page; rollback reverses: page -> rooms -> API.
  order_ok := rs_next(0) == RS_API && rs_next(1) == RS_ROOMS && rs_next(3) == RS_PAGE && rs_next(7) == RS_DONE;
  skip_refused := !rs_may(RS_PAGE, 1) && !rs_may(RS_ROOMS, 0);
  back_ok := rs_back(7) == RS_PAGE && rs_back(3) == RS_ROOMS && rs_back(1) == RS_API;
  print("order_ok=% skip_refused=% rollback_order_ok=%\n", order_ok, skip_refused, back_ok);

  // 2. Wire and three-pin gates: a page never ships ahead of any live shard.
  own := RsPins.{engine = 4, rules = 9, content = 3, wire_lo = 5, wire_hi = 6};
  prev := RsPins.{engine = 3, rules = 9, content = 2, wire_lo = 5, wire_hi = 5};
  page := RsPins.{engine = 4, rules = 9, content = 3, wire_lo = 6, wire_hi = 6};
  pins_all := rs_pins_match(page, own) && rs_pins_ship(6, 6) && rs_wire_ship(6, 6, 6, 5);
  lag_refused := !rs_pins_ship(5, 6) && !rs_wire_ship(5, 6, 6, 5);
  beside_ok := rs_pins_beside(prev, own, prev) && !rs_pins_beside(RsPins.{engine = 3, rules = 9, content = 3}, own, prev);
  print("pins_all=% lag_refused=% prev_pins_beside=%\n", pins_all, lag_refused, beside_ok);

  // 3. Staging promotion pin and staging lock (soak yields to release; another release blocks).
  p_ok := rs_promote("01bdb405f0-20261113", "01bdb405f0-20261113") == RS_P_OK;
  p_moved := rs_promote("01bdb405f0-20261113", "de2783f887-20261113") == RS_P_MOVED;
  p_seeded := rs_promote("01bdb405f0-seeded", "01bdb405f0-seeded") == RS_P_SEEDED;
  lk_held := rs_lock(true, "tok-a", "tok-b", 50, 100, true, true);
  soak_yield := rs_lock_yields(lk_held, "soak 8da7b1d5e7", "release-set 1.2");
  rel_block := !rs_lock_yields(lk_held, "release-set 1.1", "release-set 1.2");
  print("promote_ok=% refuse_moved=% refuse_seeded=% soak_yields=% release_blocks=%\n",
    p_ok, p_moved, p_seeded, soak_yield, rel_block);

  // 4. Canary verdict vs peer shards + pre-release fault qualification fail-closed check.
  peers := RsShard.{tick_p50_us = 450, tick_p95_us = 600, ticks_ds = 300, refused_ds = 4, seated = 32, unsaved_ms = 5000, windows = 30};
  good := RsShard.{tick_p50_us = 480, tick_p95_us = 640, ticks_ds = 300, refused_ds = 5, seated = 32, unsaved_ms = 5200, windows = 30};
  bad := good; bad.crashes = 1; bad.tick_p95_us = 20600; bad.version_refused = 1;
  words: [120] u8;
  v_good := rs_canary(good, peers);
  v_bad := rs_canary(bad, peers);
  why := rs_c_words(v_bad, words[..]);
  qual_ok := mq_slice("durable") == 0 && mq_pass(0, 0, false, false, true) && !mq_pass(0, 0, true, false, true);
  print("canary_good=% canary_bad_why=% qual_fail_closed=%\n", v_good == 0, why, qual_ok);

  return order_ok && skip_refused && back_ok && pins_all && lag_refused && beside_ok &&
         p_ok && p_moved && p_seeded && soak_yield && rel_block &&
         v_good == 0 && v_bad == RS_C_CRASH + RS_C_TICK + RS_C_VERSION && qual_ok;
}
```

Output (`coax/bin/coax run everglade/examples/release-set-gates.coax`, identical in `--release`):

```text
order_ok=true skip_refused=true rollback_order_ok=true
pins_all=true lag_refused=true prev_pins_beside=true
promote_ok=true refuse_moved=true refuse_seeded=true soak_yields=true release_blocks=true
canary_good=true canary_bad_why=crashed,slower-tick,refuses-the-build qual_fail_closed=true
ok=true
```

## The checks

```shell
coax/bin/coax test --library coax/web/release-set-policy.coax
coax/bin/coax test --library coax/web/room/mmo-fault-policy.coax
coax/bin/coax run everglade/examples/release-set-gates.coax
```

## Mistakes

- **Promoting a staging build that moved mid-release (`#13397`, `#13398`).** On
  2026-11-12, another deploy landed on staging between a release's canary and
  its promotion, sending the untested build to all six production shards.
  `rs_promote` now requires `--release NAME` to match staging's running unit,
  and `rs_lock` holds `/opt/oa-room/staging.lock` from first step to last.
- **Shipping a page ahead of its rooms (`#13163`, `#13387`).** Pushing a page
  with a bumped wire version before rooms upgraded caused every room to refuse
  `hello` and loop the browser on reload. `release.sh` now calls
  `release-set page-wire` and refuses to ship unless every live shard matches.

## What comes next

Shipping safely is half of running an MMORPG; the other half is watching it run
at 3 a.m. and handling live incidents without taking the world down.
[Chapter 32](ch032-run-a-live-world.md) covers service levels, alerts, runtime
flags, live moderation, crash reports, and the nightly full-fleet soak.
