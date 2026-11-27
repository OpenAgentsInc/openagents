# 32. Run a live world

> **Status: partly implemented.** Thirty-day service levels, seven automated
> alerts, and deploy-free runtime flags live in
> [`coax/web/api/service-policy.coax`](../../coax/web/api/service-policy.coax)
> and [`coax/web/api/service.c`](../../coax/web/api/service.c)
> ([`docs/deployment/service-levels.md`](../../docs/deployment/service-levels.md)),
> fail-closed live moderation lives in
> [`metaverse-engine/session/moderation.coax`](../../metaverse-engine/session/moderation.coax)
> and [`coax/web/room/room-mod.coax`](../../coax/web/room/room-mod.coax),
> symbolicated crash bucketing lives in
> [`coax/web/room/room-crash.coax`](../../coax/web/room/room-crash.coax)
> ([`docs/deployment/crash-reports.md`](../../docs/deployment/crash-reports.md)),
> and the nightly two-hour full-fleet soak lives in
> [`coax/web/room/soak-policy.coax`](../../coax/web/room/soak-policy.coax) and
> [`coax/web/room/soak.coax`](../../coax/web/room/soak.coax), exercised by
> [`everglade/examples/live-world-ops.coax`](../examples/live-world-ops.coax).
> Automated postmortem timelines, multi-region failover drills, and
> self-healing shard autoscaling remain to be built.

This chapter answers the operator's question: "How do I know Everglade is
healthy right now, intervene while players stay logged in, and catch memory
leaks or crashes before players do?"

Running a live MMORPG means measuring player-visible health continuously,
keeping low-risk controls (maintenance mode, shard draining, moderation)
available without redeploying binaries, and turning every crash or soak
regression into a tracked fix.

## What this chapter teaches

- How three 30-day service-level objectives (tick time, join time, and
  checkpoint age) plus the 300 ms seat-ticket target define "healthy".
- How the per-minute `service-watch` job raises and resolves shard, database,
  and crash alerts with concrete runbook actions—without paging on planned
  drains.
- How deploy-free flags (`maintenance`, `closed-sK`) and outbox-first
  moderation (`look`, `move`, `kick`, `mute`, `ban`) let operators intervene
  live with an append-only audit trail.
- How the nightly two-hour soak and its seeded-fault self-check catch slow
  ticks, memory leaks, and astray creatures before a release goes out.

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| Service levels & alerts (`#authority`) | 3 SLOs (tick p99 ≤ 10 ms 99.9%, join ≤ 5 s 95%, checkpoint ≤ 15 s 99.9%), 7 alerts with runbooks, `maintenance` / `closed-sK` flags | [`coax/web/api/service-policy.coax`](../../coax/web/api/service-policy.coax), [`coax/web/api/service.c`](../../coax/web/api/service.c) |
| Live room telemetry | 10-second signed `/api/playtest/room-stats` reports folded into per-minute rollups (`verse.room_minutes`) | [`coax/web/room/room-observe.coax`](../../coax/web/room/room-observe.coax), [`coax/web/room/room-stats.coax`](../../coax/web/room/room-stats.coax) |
| Live moderation (`#authority`) | 8 moderator verbs (`look`..`unban`) gated by `ACCT 2`, outbox-logged before execution, refusing self-sanctions | [`metaverse-engine/session/moderation.coax`](../../metaverse-engine/session/moderation.coax), [`coax/web/room/room-mod.coax`](../../coax/web/room/room-mod.coax) |
| Crash reports & bucketing | Async-signal-safe room dump + `addr2line` Coax frames, Wasm `.syms` page crashes, 1-hour alert window, auto-filed GitHub issues | [`coax/web/room/room-crash.coax`](../../coax/web/room/room-crash.coax), [`docs/deployment/crash-reports.md`](../../docs/deployment/crash-reports.md) |
| Nightly soak & self-check | 9-bit failure verdict (`empty`..`astray`), 2-hour staging fleet run with seeded slow-tick & leak self-check | [`coax/web/room/soak-policy.coax`](../../coax/web/room/soak-policy.coax), [`coax/web/room/soak.coax`](../../coax/web/room/soak.coax) |

**What remains to ship:** Per-minute SLO rollups, scheduled alert delivery,
deploy-free runtime flags, outbox-logged moderation, symbolicated crash
bucketing, and nightly 2-hour staging soaks are all built and wired in
`coax/web/api/schedules.conf` (`service-watch * * * * *`) and
`coax/remote/after.sh`. Automated incident postmortem generation, multi-region
room failover drills, and autonomous shard autoscaling remain to be built.

## The method

### Service levels, alerts, and deploy-free flags

Every 10 seconds each shard posts its worst tick p99, resident memory, and
oldest unsaved checkpoint (`rm_st_unsaved_ms`). Every minute `POST
/api/jobs/service-watch` evaluates `service-policy.coax`:

- **Alerts fire only on actionable failure.** A VM shard silent for 45 s raises
  `silent:N`, unless it reported `draining` (a planned deploy) or has been
  silent for 24 h (retired). Two slow reports (> 10 ms tick p95) in 2 minutes
  raise `tick:N`; any checkpoint failure in 5 minutes raises
  `checkpoint-failures:N`; 3 fenced writes in 10 minutes raise `fenced:N`;
  30 s of unsaved checkpoint age raises `unsaved:N`; and 80% Postgres
  connection usage raises `db-connections`.
- **Flags intervene without a deploy.** `POST /api/admin/flags` sets bounded
  flags (`maintenance`, `closed-s3`) that rooms pick up on their 2-second
  control poll (`room-control.coax`). A draining flag pulls a sick shard out of
  new-player rotation immediately while existing players finish safely.

### Outbox-first moderation

When a moderator (ticket `ACCT 2`) sends `X mute p-a 60 spam in commons`,
`room-mod.coax` checks `moderation_judge` (refusing non-moderators, self
actions, or missing reasons) and writes the audit line into its outbox
(`rm_mod_offer`) **before** executing the action. If the outbox is full, the
command is refused—nothing a moderator does ever goes unlogged.

### Nightly soak with a check of the check

Every green head runs a two-hour protocol-bot soak on staging. Before trusting
the soak, `soak.coax self-check` runs a local room three times: clean (must
pass), seeded slow tick (`OA_ROOM_FAULT_TICK_US`, must fail `slow-tick`), and
seeded memory leak (`OA_ROOM_FAULT_LEAK_KB`, must fail `leak`). A test harness
that cannot fail is never allowed to declare a healthy night.

## Worked trace: SLOs, alerts, moderation, and soak verdict

```text
1. SLOs:      tick 999/1000 (>=99.9%) -> met (1); 998/1000 -> missed (0)
              ticket p95 240 ms <= 300 ms -> ok; 301 ms -> missed
2. Alerts:    addressed shard silent 45,000 ms -> SV_A_SILENT (1)
              draining shard silent 60,000 ms  -> 0 (planned deploy)
              2 slow ticks + 1 failure + 30,000 ms unsaved -> 2 + 4 + 16 = 22
              DB 80/100 connections -> alert; crash 20 min old -> alert (gone at 60 min)
3. Moderation: moderator mute 60 min -> MOD_OK, until = now + 3,600,000 ms
               non-mod kick -> MOD_NOT_MODERATOR; self unban -> MOD_SELF
               ban with reason "-" -> MOD_BAD
4. Soak:      96 bots, flat RSS, 0 astray -> verdict 0 (pass)
              2 slow windows + 65,536 KB RSS growth + 1 astray -> "slow-tick,leak,astray"
```

## Runnable example

[`everglade/examples/live-world-ops.coax`](../examples/live-world-ops.coax)
runs the 30-day SLO math, shard/DB/crash alert evaluation, runtime flag check,
fail-closed moderation judge, and nightly soak verdict directly against
[`coax/web/api/service-policy.coax`](../../coax/web/api/service-policy.coax),
[`metaverse-engine/session/moderation.coax`](../../metaverse-engine/session/moderation.coax),
and [`coax/web/room/soak-policy.coax`](../../coax/web/room/soak-policy.coax)
(committed transcript
[`everglade/examples/live-world-ops.out`](../examples/live-world-ops.out)):

<!-- source: everglade/examples/live-world-ops.coax live-world-ops -->
```jai
ops_run :: () -> bool {
  // 1. Service levels: tick p99 <= 10 ms (99.9%), join <= 5 s (95%), checkpoint <= 15 s (99.9%), ticket p95 <= 300 ms.
  slo_pass := sv_met(SV_TICK, 999, 1000) == 1 && sv_met(SV_JOIN, 95, 100) == 1 && sv_met(SV_CHECKPOINT, 1000, 1000) == 1 && sv_ticket_ok(240);
  slo_miss := sv_met(SV_TICK, 998, 1000) == 0 && !sv_ticket_ok(301);
  print("slo_pass=% slo_miss_caught=%\n", slo_pass, slo_miss);

  // 2. Live shard/DB/crash alerts and deploy-free runtime flags (maintenance, closed-s3).
  a_silent := sv_shard_alerts(true, false, 45000, 0, 0, 0, 0);
  a_drain := sv_shard_alerts(true, true, 60000, 0, 0, 0, 0);
  a_sick := sv_shard_alerts(true, false, 0, 2, 1, 0, 30000);
  db_hi := sv_db_alert(80, 100);
  crash_live := sv_crash_alert(1200000) && !sv_crash_alert(3600000);
  maint_on := sv_flag_name_ok("maintenance") && sv_flag_name_ok("closed-s3") && sv_maintenance(1, 0, 1000);
  print("alert_silent=% drain_quiet=% sick_bits=% db_alert=% crash_1h=% maint_on=%\n",
    a_silent == SV_A_SILENT, a_drain == 0, a_sick, db_hi, crash_live, maint_on);

  // 3. Live moderation: requires moderator bit, non-self target, and explicit reason.
  m_ok := moderation_judge(MOD_MUTE, true, false, 60, "spam in commons");
  m_nonmod := moderation_judge(MOD_KICK, false, false, 0, "griefing");
  m_self := moderation_judge(MOD_UNBAN, true, true, 0, "-");
  m_noreason := moderation_judge(MOD_BAN, true, false, 0, "-");
  until_ms := moderation_until(MOD_MUTE, 60, 1000000);
  stopped_speak := moderation_speak(false, true) == MOD_KIND_MUTE;
  stopped_ticket := moderation_ticket(true, false) == MOD_KIND_BAN;
  print("mod_ok=% refuse_nonmod=% refuse_self=% refuse_noreason=% mute_ms=% speak_muted=% ticket_banned=%\n",
    m_ok == MOD_OK, m_nonmod == MOD_NOT_MODERATOR, m_self == MOD_SELF, m_noreason == MOD_BAD,
    until_ms - 1000000, stopped_speak, stopped_ticket);

  // 4. Nightly full-fleet soak verdict: healthy 96-bot fleet passes; slow ticks + 64 MiB leak + astray creature fail.
  ok_soak := SkFacts.{seated_most = 96, seated_end = 96, memory_early_kb = 700000, memory_late_kb = 700100, ticket_p95_ms = 40, windows_read = 700, windows_due = 720, astray = 0};
  bad_soak := ok_soak; bad_soak.slow_windows = 2; bad_soak.memory_late_kb = 700000 + SK_LEAK_KB; bad_soak.astray = 1;
  wb: [96] u8;
  soak_pass := sk_verdict(ok_soak) == 0;
  soak_why := sk_words(sk_verdict(bad_soak), wb[..]);
  print("soak_pass=% soak_bad_why=%\n", soak_pass, soak_why);

  return slo_pass && slo_miss && a_silent == SV_A_SILENT && a_drain == 0 &&
         a_sick == SV_A_TICK + SV_A_FAILURES + SV_A_UNSAVED && db_hi && crash_live && maint_on &&
         m_ok == MOD_OK && m_nonmod == MOD_NOT_MODERATOR && m_self == MOD_SELF && m_noreason == MOD_BAD &&
         until_ms == 4600000 && stopped_speak && stopped_ticket &&
         soak_pass && soak_why == "slow-tick,leak,astray";
}
```

Output (`coax/bin/coax run everglade/examples/live-world-ops.coax`, identical in `--release`):

```text
slo_pass=true slo_miss_caught=true
alert_silent=true drain_quiet=true sick_bits=22 db_alert=true crash_1h=true maint_on=true
mod_ok=true refuse_nonmod=true refuse_self=true refuse_noreason=true mute_ms=3600000 speak_muted=true ticket_banned=true
soak_pass=true soak_bad_why=slow-tick,leak,astray
ok=true
```

## The checks

```shell
coax/bin/coax test --library coax/web/api/service-policy.coax
coax/bin/coax test --library metaverse-engine/session/moderation.coax
coax/bin/coax test --library coax/web/room/soak-policy.coax
coax/bin/coax run everglade/examples/live-world-ops.coax
```

## Mistakes

- **Measuring capture age instead of unsaved age (`#13297`).** Using
  `checkpoint capture age ms most=` for the checkpoint SLO triggered false
  alarms whenever a player disconnected cleanly, because their slot kept its
  last saved capture timestamp. `rm_st_unsaved_ms` measures only the oldest
  change that no API receipt covers yet.
- **Unchecked moderator actions or self-unbans.** Letting a room apply a kick
  or mute before recording it risked invisible sanctions, and allowing a
  moderator to act on their own ID let compromised accounts lift their own
  bans. `moderation_judge` refuses `self && verb != MOD_LOOK`, and `room-mod.coax`
  refuses the command if the audit outbox is full.

## What comes next

With the world built and running live, Part X turns to beasts and mounts.
[Chapter 33](ch033-lets-build-a-dragon.md) models a dragon from a script, flies
it on SRD speeds, breathes a 60-foot fire cone, and mounts it in a room.
