# 30. Typed clocks and multirate ticks

> **Status: partly implemented.** This is roadmap R13 (spec stage 10).
> The library profile in `coax/lib/clock.coax` runs in `coax0`: instants
> typed by clock, conversions that name their source, sample-and-hold
> connections with a maximum age, tick schedules in a fixed order, the UTC,
> TAI, TT and TDB scales over a pinned leap-second table, and time warp that
> stops at every event. The `task @rate` syntax, compiler-enforced clock-read
> effects (R11) and adoption by the robot and the OpenAgents Metaverse loops are planned.

A simulation reads several clocks. Simulation time can pause, scale or jump.
Steady time only moves forward. Civil time follows a calendar and can be
corrected. A program that keeps all three as plain integers can subtract one
from another and get a number that means nothing. In this chapter, you give
each clock its own instant type, connect tasks that run at different rates,
convert between astronomical time scales, and run a warped clock that still
lands on every scheduled event.

## Instants carry their clock (runs today)

A clock is an empty tag struct. An instant is a nanosecond count in an `s64`
with the tag as a type parameter, so `Instant(Sim)` and `Instant(Steady)` are
different types. The library declares `Sim`, `Steady`, `Civil`, `UTC`, `TAI`,
`TT` and `TDB`, and a constructor for each, such as `sim_at(ns)`. A literal
that names its clock, such as `Instant(Sim).{ns = 0}`, works too:

```jai
Instant :: struct($C: copyable) { ns: s64; }

// An instant on one clock, from its nanosecond count.
sim_at :: (ns: s64) -> Instant(Sim) { r: Instant(Sim); r.ns = ns; return r; }
steady_at :: (ns: s64) -> Instant(Steady) { r: Instant(Steady); r.ns = ns; return r; }
civil_at :: (ns: s64) -> Instant(Civil) { r: Instant(Civil); r.ns = ns; return r; }
utc_at :: (ns: s64) -> Instant(UTC) { r: Instant(UTC); r.ns = ns; return r; }
tai_at :: (ns: s64) -> Instant(TAI) { r: Instant(TAI); r.ns = ns; return r; }
tt_at :: (ns: s64) -> Instant(TT) { r: Instant(TT); r.ns = ns; return r; }
tdb_at :: (ns: s64) -> Instant(TDB) { r: Instant(TDB); r.ns = ns; return r; }

elapsed :: (later: Instant($C: copyable), earlier: Instant($C)) -> s64 {
  return later.ns - earlier.ns;
}
```

`elapsed` binds `$C` from its first argument, and its second argument must
have the same clock. Integer nanoseconds keep every sum exact, so a replay
gets the same instants. When physics code needs seconds, `seconds(ns)` turns a
span into chapter 29's `Quantity(Second, f64)`.

Pass instants from two clocks, and the compiler refuses the call. The program
is [`examples/ch30_clocks_refused.coax`](examples/ch30_clocks_refused.coax):

<!-- checked-source: examples/ch30_clocks_refused.coax -->
```jai
#import "Basic";
#load "../../lib/clock.coax" as clock;

main :: () {
  sim := clock.sim_at(5000);
  wall := clock.steady_at(9000);
  print("%\n", clock.elapsed(wall, sim));   // refused: two clocks
}
```

```shell
$ coax run examples/ch30_clocks_refused.coax
examples/ch30_clocks_refused.coax:7:36: error[E0348]: '$C' is Steady from another argument of 'elapsed', but argument 2 is Sim
    print("%\n", clock.elapsed(wall, sim));   // refused: two clocks
                                     ^
  fix: pass the same type; a struct or array does not convert with cast
1 error
```

A clock tag is a struct with no value, so nothing converts to it with a
cast. To compare a steady instant with a simulation instant, convert one of
them explicitly, as the next section shows.

## Conversions name their source

`Clock_Map(From, To)` holds a pinned offset and a `source`: a number that
identifies where the offset came from, such as a recorded mapping or a
configuration. `map_clock(m, at)` returns a `Mapped(To)` with the converted
instant and that source, so a converted instant says how it was converted.
There is no conversion without a map.

## Sample-and-hold with a maximum age

Tasks that run at different rates share values through a connection.
`Hold(T, C)` keeps the latest value, when it was written and how old a reader
may see it. `hold_get` returns a `Held(T)`: the value and its age, or an
error.

```jai
hold_get :: (h: Hold($T: copyable, $C: copyable), now: Instant($C)) -> Held($T) {
  result: Held($T);
  if !h.written { result.error = Hold_Error.empty; return result; }
  age := now.ns - h.at.ns;
  result.age_ns = age;
  if age < 0 { result.error = Hold_Error.future; return result; }
  if age > h.max_age_ns { result.error = Hold_Error.stale; return result; }
  result.value = h.value;
  return result;
}
```

A read past the maximum age is `Hold_Error.stale`, not a silently stale value.
The error is a value the caller must look at ([chapter 7](ch07-errors.md)).
A stale or failed read returns a zero value, never the old one.

## Tick schedules run in a fixed order

A `Rate_Task` runs at its phase and then every period. `schedule_due(tasks,
now, out)` writes the indices of the tasks due at `now` into `out`, in their
declared order. Given the rates and phases, the order is the same on every
run. The program
[`examples/ch30_ticks.coax`](examples/ch30_ticks.coax) runs physics at 1 kHz,
control at 500 Hz and telemetry at 100 Hz from 1 ms. Control reads the joint
angle that physics wrote, at most 4 ms old. After four ticks the simulation
pauses: physics stops writing, and control's next read is stale.

<!-- checked-source: examples/ch30_ticks.coax -->
```jai
#import "Basic";
#load "../../lib/clock.coax" as clock;

// Physics at 1 kHz, control at 500 Hz, telemetry at 100 Hz starting 1 ms in.
NAMES :: [3] string.["physics", "control", "telemetry"];

main :: () {
  tasks := [3] clock.Rate_Task.[
    clock.Rate_Task.{period_ns = 1000000, phase_ns = 0},
    clock.Rate_Task.{period_ns = 2000000, phase_ns = 0},
    clock.Rate_Task.{period_ns = 10000000, phase_ns = 1000000}];
  // Control reads the joint angle physics wrote, at most 4 ms old.
  angle: clock.Hold(f64, clock.Sim) = clock.Hold.{max_age_ns = 4000000};
  due: [3] s64;
  for tick: 0..<4 {
    now := clock.sim_at(tick * 1000000);
    count := clock.schedule_due(tasks[..], now, due[..]);
    print("t=% ms:", tick);
    for i: 0..<count { print(" %", NAMES[due[i]]); }
    print("\n");
    for i: 0..<count {
      if due[i] == 0 { angle = clock.hold_put(angle, 0.1 * cast(f64) tick, now); }
      if due[i] == 1 {
        r := clock.hold_get(angle, now);
        print("  control reads % (age % ns, %)\n", r.value, r.age_ns, r.error);
      }
    }
  }
  // Pause the simulation: physics stops writing, and control's read goes stale.
  late := clock.hold_get(angle, clock.sim_at(8000000));
  print("after a pause, control at 8 ms reads % (age % ns, %)\n", late.value, late.age_ns, late.error);
}
```

```shell
$ coax run examples/ch30_ticks.coax
t=0 ms: physics control
  control reads 0.0 (age 0 ns, none)
t=1 ms: physics telemetry
t=2 ms: physics control
  control reads 0.2 (age 0 ns, none)
t=3 ms: physics
after a pause, control at 8 ms reads 0.0 (age 5000000 ns, stale)
```

The library's own check compares 60 ticks of a three-task schedule with an
independent reference trace: it expands every task's run times, sorts them by
time and declaration order, and requires the same 110 runs in the same order.

## Astronomical time scales

UTC follows the Earth's rotation, so it gains a leap second from time to time.
TAI counts SI seconds without leap seconds. TT is TAI plus exactly 32.184 s.
TDB, the time argument of planetary ephemerides, differs from TT by a periodic
term under 2 ms.

- `utc_to_tai` adds TAI − UTC from a pinned table: 10 s from 1972-01-01 and
  the 27 leap seconds through 2017-01-01. A UTC instant counts POSIX
  nanoseconds, so the leap second itself (23:59:60) has no UTC instant here.
- The table's identity, `LEAP_TABLE_ID`, is an FNV-1a hash of its rows and its
  end. Each converted instant carries it in `Scaled(TAI).table`. A different
  table is a different identity.
- The table covers 1972-01-01 up to, but not including, 2026-06-28, the expiry
  of the IERS leap-second list it was checked against. An instant outside that
  coverage is refused with `before_table` or `after_table`. A newer list
  extends the coverage under a new identity.
- `tt_to_tdb` adds TDB − TT from the series of USNO Circular 179,
  equation 2.6, which the circular gives as good to about 10 µs from 1600 to
  2200.

The program [`examples/ch30_scales.coax`](examples/ch30_scales.coax) converts
five UTC instants:

```shell
$ coax run examples/ch30_scales.coax
leap-second table 7334693354795583479
2016-12-31T23:59:59Z: TAI-UTC 36000000000 ns, TT-UTC 68184000000 ns, TDB-TT -45649 ns
2017-01-01T00:00:00Z: TAI-UTC 37000000000 ns, TT-UTC 69184000000 ns, TDB-TT -45648 ns
2026-01-01T00:00:00Z: TAI-UTC 37000000000 ns, TT-UTC 69184000000 ns, TDB-TT -80496 ns
1971-12-31T00:00:00Z: refused, before_table
2026-07-01T00:00:00Z: refused, after_table
```

TT − UTC is 69.184 s after 2017-01-01, the value IERS publishes. The 2026-07-01
instant is past the table's coverage, so it's refused rather than converted
with a guess. The library checks TAI − UTC on both sides of the 1972-07-01 and
2017-01-01 leap seconds, TT − UTC against the published value, and
`days_from_civil` against day-by-day counting from year −803 to 2103. The TDB
values are checked against the series itself and its bound; a comparison with
a full ephemeris time series is not part of this profile.

## Time warp stops at every event

A warp factor scales simulation time against steady time. `warp_step(now,
steady_ns, warp, next_event)` returns the simulation step for one steady step,
capped at the next event:

```jai
warp_step :: (now: Instant(Sim), steady_ns: s64, warp: s64, next_event: Instant(Sim)) -> s64 {
  room := next_event.ns - now.ns;
  if steady_ns < 1 || warp < 1 || room < 1 { return 0; }
  if warp > room / steady_ns { return room; }
  return steady_ns * warp;
}
```

The program [`examples/ch30_warp.coax`](examples/ch30_warp.coax) steps 1 ms
host frames through four events at warp 1 and at warp 10⁶:

```shell
$ coax run examples/ch30_warp.coax
warp 1: event 0 at 3500000 ns, event 1 at 10000000 ns, event 2 at 10000001 ns, event 3 at 2000000000 ns, 2002 steps
warp 1000000: event 0 at 3500000 ns, event 1 at 10000000 ns, event 2 at 10000001 ns, event 3 at 2000000000 ns, 4 steps
```

Warp 10⁶ takes 4 steps instead of 2,002, and lands on the same events at the
same instants in the same order, including two events 1 ns apart.
[Chapter 32](ch32-strict-math-and-integrators.md) adds event location for
events that a solver has to find.

## Reading a clock

No procedure in `coax/lib/clock.coax` reads a clock. The host reads its
clocks and passes instants in, so the code that computes with time stays
free of clock reads. `coax deps FILE --effects` reports no foreign call
(bit 2) for any of the library's procedures. Making a clock read an effect
that the compiler tracks and refuses in pure code is R11 work.

## What remains planned

- The task syntax: `control :: task @rate(hz=500) (q: Hold(JointState,
  max_age=4ms))`. Today a schedule is a `Rate_Task` array and a connection is
  a `Hold` value.
- Clock reads as compiler-checked effects (R11).
- Adoption: the simulated robot's physics and control steps and the Metaverse's
  fixed-step update driven from declared schedules
  ([Metaverse Engine chapter 1](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch001-a-frame-has-a-fixed-step.md)),
  and recorded clock boundaries for replay
  ([chapter 33](ch33-replay-and-snapshots.md)).

## Dependencies

- R12 durations ([chapter 29](ch29-units-and-frames.md)).
- Typed errors ([chapter 7](ch07-errors.md)).
- Generic structs with type parameters ([chapter 8](ch08-generics.md)).
- Used by Metaverse Engine chapters
  [1](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch001-a-frame-has-a-fixed-step.md),
  [39](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch039-ephemerides-time-scales-and-frames.md)
  and [46](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch046-time-warp-that-stops-for-every-event.md).
