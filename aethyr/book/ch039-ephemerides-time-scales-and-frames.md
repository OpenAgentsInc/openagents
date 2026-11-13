# 39. Ephemerides, time scales and frames

> **Status: partly implemented.** [`aethyr/space/ephemeris.coax`](../../aethyr/space/ephemeris.coax)
> (280 mutants: 275 killed, 5 marked equivalent with their reason) reads
> JPL Horizons vector tables with their time scale, frame, centre,
> dataset and SHA-256, refuses a query outside a table's coverage, and
> computes light-time and aberration corrections that match Horizons' own to
> 1 m and 0.001 arcseconds. [`aethyr/space/frames.coax`](../../aethyr/space/frames.coax)
> carries whole states between an inertial frame and a turning one, `ω × r`
> included. Time scales come from Coax's [`coax/lib/clock.coax`](../../coax/lib/clock.coax).
> The SPICE SPK reader, IAU rotation models and interpolation between table
> rows are not built yet.

Every later chapter in this part asks where something is at a time. The
answer is only meaningful with four labels attached: which time scale, which
frame, which centre and which dataset. A mean-element ephemeris is good
enough to draw Belt 2200; it is not good enough to check a transfer against
JPL. This chapter makes the labels part of the data and turns the existing
ephemeris's error into a measured number.

## What this chapter teaches

- Reading a pinned ephemeris (Horizons vector tables first, then one SPICE
  SPK segment type) with its time scale, frame, centre, units, dataset
  version and coverage recorded.
- TAI, TT, TDB and UTC, and converting through a pinned leap-second table.
- Full state transforms between barycentric inertial, heliocentric and
  body-fixed rotating frames, including the `ω × r` velocity term.
- Geometric, light-time-corrected and aberration-corrected states as three
  distinct quantities.

## The method

An ephemeris record is a state with its provenance:

```jai
EphemerisState :: struct {
    r: D3; v: D3;            // metres, metres per second
    t: Epoch;                // seconds past J2000 in a named scale
    scale: TimeScale;        // TDB for planetary ephemerides
    frame: Frame;            // ICRF / ecliptic J2000 / body-fixed
    centre: Body;            // solar-system barycentre, Sun, Earth...
    dataset: DatasetId;      // e.g. DE440, with its content digest
}
```

Time scales differ by offsets and slow periodic terms: `TT = TAI + 32.184 s`;
`TAI = UTC + ΔAT`, with `ΔAT` from the pinned leap-second table; `TDB − TT`
is a periodic term below 2 ms, evaluated from its standard series. A UTC
instant inside a leap second, or after the table's last entry, is refused
rather than guessed.

A frame change transforms the whole state, not only the position. For a
frame rotating with angular velocity `ω` relative to an inertial one, with
rotation `R`:

```jai
r_inertial = R r_local
v_inertial = R (v_local + ω × r_local)
```

Dropping the `ω × r` term is the classic error; JPL's frames documentation
makes the same distinction between position and state transforms
([NAIF frames](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/frames.html)).

The light-time-corrected state of a target seen from an observer at
receive time `t_r` solves `t_r − t_e = |r_target(t_e) − r_obs(t_r)| / c` by
fixed-point iteration from `t_e = t_r`; stellar aberration then shifts the
apparent direction by the observer's velocity. Each correction is a separate
named query, so a caller cannot mix a geometric state with an apparent one.

## What runs today

Time scales are Coax's, not the OpenAgents Metaverse Engine's: [`coax/lib/clock.coax`](../../coax/lib/clock.coax)
(Coax [chapter 30](../../coax/book/ch30-clocks-and-ticks.md))
gives each scale its own instant type, `Instant(UTC)`, `Instant(TAI)`,
`Instant(TT)` and `Instant(TDB)`, with `utc_to_tai` through a pinned
leap-second table that refuses times outside it, `tai_to_tt` and
`tt_to_tdb`. Passing a TT instant where a TDB one is wanted is a type error.

[`aethyr/space/ephemeris.coax`](../../aethyr/space/ephemeris.coax) reads the
`$$SOE`..`$$EOE` block of a Horizons VECTORS response (CSV, `VEC_TABLE 2`)
into an `EphTable`: TDB instants, positions and velocities in metres, the
frame (ecliptic of J2000), the centre, the dataset name and the block's
SHA-256. A line with the wrong number of fields, a number it can't read or a
time that doesn't increase is refused with its line number. `eph_at` answers
a TDB instant from the nearest row carried by two-body motion, and refuses
one outside the first-to-last row. Each `EphState` says what kind of state
it is: `eph_light_time` takes only geometric states about the table's
centre and returns a `light_time` state; `eph_aberrate` takes only a
`light_time` state and returns an `apparent` one. A caller can't mix them.

The tables are in [`aethyr/space/fixtures/`](../../aethyr/space/fixtures/README.md):
each Horizons response as retrieved, its query parameters, and the SHA-256
of the block that `horizons.coax` holds as text. All are DE441 in TDB.

[`aethyr/space/frames.coax`](../../aethyr/space/frames.coax) turns a frame
about its own +z at a constant rate from an angle at J2000, tilted by a
fixed obliquity. `frame_to_inertial` and `frame_to_body` carry the whole
state; `frame_to_inertial_position_only` leaves out `ω × r` so the checks can
show what that gets wrong. `FRAME_EARTH` uses a sidereal day and 23.439° of
obliquity; it is a stated model, not the IAU Earth rotation model.

The example [aethyr/examples/read-an-ephemeris.coax](../examples/read-an-ephemeris.coax) reads the tables, carries a UTC time to TDB, corrects Mars as
seen from the Earth on 2 January 2025, transforms a point on the equator and
measures [`aethyr/physics/orbit.coax`](../physics/orbit.coax)'s mean elements
against DE441:

<!-- source: aethyr/examples/read-an-ephemeris.coax example -->
```jai
// The largest distance between orbit.coax's body k and a table's rows, and the year it happens.
worst :: (t: EphTable, k: int) -> [2] int {
  most := 0.0; at := 0;
  for i: 0..<t.rows {
    gap := d3_len(d3_sub(orbit_body_state(k, eph_seconds(t.at[i])).r, t.r[i]));
    if gap > most { most = gap; at = i; }
  }
  return [2] int.[km(most), 2000 + cast(int) floor(eph_seconds(t.at[at]) / YEAR + 0.5)];
}

main :: () {
  mars := eph_load(HZ_MARS, EphCentre.sun, GM_SUN);
  print("Mars about the Sun: % rows of %, TDB, ecliptic J2000, from day % to day % after J2000\n",
    mars.rows, mars.dataset.name, eph_seconds(mars.at[0]) / DAY, eph_seconds(mars.at[mars.rows - 1]) / DAY);
  print("a day before the first row: %; a day after the last: %\n",
    eph_at(mars, tdb_at(mars.at[0] - 86400000000000)).status, eph_at(mars, tdb_at(mars.at[mars.rows - 1] + 86400000000000)).status);

  // 2025-01-01T23:58:50.816 UTC is 2025-01-02T00:00:00 TDB to the millisecond:
  // 37 leap seconds, 32.184 s to TT, and TDB - TT from the USNO series.
  utc := utc_at(days_from_civil(2025, 1, 1) * 86400000000000 + 86330816000000);
  tai := utc_to_tai(utc); tdb := tt_to_tdb(tai_to_tt(tai.at));
  print("UTC 2025-01-01 23:58:50.816 is TDB % ms after 2025-01-02 00:00\n", (tdb.ns - days_from_civil(2025, 1, 2) * 86400000000000) / 1000000);
  print("a UTC time after the leap-second table: %\n", utc_to_tai(utc_at(days_from_civil(2027, 1, 1) * 86400000000000)).error);

  ssb_mars := eph_load(HZ_MARS_SSB, EphCentre.barycentre, GM_SUN);
  earth := eph_load(HZ_EARTH_SSB, EphCentre.barycentre, GM_SUN);
  geo := eph_load(HZ_MARS_GEO, EphCentre.earth, EPH_MU_EARTH);
  lt := eph_load(HZ_MARS_GEO_LT, EphCentre.earth, EPH_MU_EARTH);
  lts := eph_load(HZ_MARS_GEO_LTS, EphCentre.earth, EPH_MU_EARTH);
  obs := eph_at(earth, tdb_at(earth.at[1])).state;
  seen := eph_light_time(ssb_mars, obs);
  apparent := eph_aberrate(seen.state, obs.v);
  print("Mars from the Earth on 2025-01-02 00:00 TDB: % km away, light time % s\n",
    km(d3_len(seen.state.r)), floor(d3_len(seen.state.r) / EPH_C * 1000.0 + 0.5) / 1000.0);
  print("  geometric vs light-time: % km apart; ours vs Horizons' LT: % m\n",
    km(d3_len(d3_sub(geo.r[1], lt.r[1]))), cast(int) floor(d3_len(d3_sub(seen.state.r, lt.r[1])) + 0.5));
  print("  aberration turns it % arcsec; ours vs Horizons' LT+S: % arcsec\n",
    arcsec(eph_angle(lt.r[1], lts.r[1])), arcsec(eph_angle(apparent.state.r, lts.r[1])));

  ground := frame_to_inertial(FRAME_EARTH, 0.0, d3(6378137, 0, 0), d3(0, 0, 0));
  flat := frame_to_inertial_position_only(FRAME_EARTH, 0.0, d3(6378137, 0, 0), d3(0, 0, 0));
  print("a point on the equator: % m/s inertial with w x r, % m/s without\n", floor(d3_len(ground.v) * 10.0 + 0.5) / 10.0, d3_len(flat.v));

  emb := eph_load(HZ_EMB, EphCentre.sun, GM_SUN);
  jupiter := eph_load(HZ_JUPITER, EphCentre.sun, GM_SUN);
  e := worst(emb, BODY_EARTH); m := worst(mars, BODY_MARS); j := worst(jupiter, BODY_JUPITER);
  print("orbit.coax's mean elements against DE441, 2000-2200, every 5 years:\n");
  print("  Earth-Moon barycentre: at most % km (%)\n", e[0], e[1]);
  print("  Mars: at most % km (%)\n", m[0], m[1]);
  print("  Jupiter: at most % km (%)\n", j[0], j[1]);
}
```

```shell
coax/bin/coax run aethyr/examples/read-an-ephemeris.coax
```

```text
Mars about the Sun: 41 rows of DE441, TDB, ecliptic J2000, from day -0.5 to day 73039.5 after J2000
a day before the first row: outside; a day after the last: outside
UTC 2025-01-01 23:58:50.816 is TDB 0 ms after 2025-01-02 00:00
a UTC time after the leap-second table: after_table
Mars from the Earth on 2025-01-02 00:00 TDB: 97892185 km away, light time 326.533 s
  geometric vs light-time: 7424 km apart; ours vs Horizons' LT: 1 m
  aberration turns it 19.625 arcsec; ours vs Horizons' LT+S: 0.0 arcsec
a point on the equator: 465.1 m/s inertial with w x r, 0.0 m/s without
orbit.coax's mean elements against DE441, 2000-2200, every 5 years:
  Earth-Moon barycentre: at most 694953 km (2200)
  Mars: at most 3138904 km (2200)
  Jupiter: at most 3872562 km (2185)
```

The light-time correction moves Mars 7,424 km from its geometric position;
ours lands 1 m from Horizons' corrected vector after the fixed-point
iteration converges. Aberration turns the direction 19.6 arcseconds. Ours
is first order in `v/c` and agrees with Horizons' relativistic correction
to under 0.001 arcseconds. Without `ω × r` a point standing on the equator has no inertial velocity;
with it, 465.1 m/s.

The last three lines are the measurement this chapter promised:
`orbit.coax`'s J2000 mean elements are fine for drawing the solar system in
2200 and not for navigation. Over 2000-2200 the Earth-Moon barycentre drifts
up to 694,953 km from DE441, Mars up to 3.1 million km and Jupiter up to
3.9 million km, each growing with distance from J2000. A transfer checked
against JPL needs the pinned tables.

The checks run with:

```shell
coax/bin/coax test aethyr/space/ephemeris.coax --library
coax/bin/coax test aethyr/space/frames.coax --library
```

They read every table, refuse bad lines and steps back by line, pin the
README's SHA-256, refuse queries outside coverage, match Horizons' LT and
LT+S states, return a state through a round trip of the turning frame, and
catch the missing `ω × r` term.

## What it still builds

- One SPICE SPK segment type (Chebyshev position, type 2) beside the Horizons
  reader, with the same labels and coverage refusal.
- IAU rotation models for the bodies used, in place of `FRAME_EARTH`'s
  constant rate and obliquity, and body-fixed frames for the Moon and Mars.
- Interpolation between table rows within the dataset's stated precision;
  today a query between rows is two-body motion from the nearest one.
- Moving `aethyr/physics/orbit.coax`'s callers from plain f64 seconds onto TDB
  instants.

## Acceptance checks and oracles

- States and light-time/aberration corrections agree with independently
  retrieved [JPL Horizons](https://ssd.jpl.nasa.gov/horizons/manual.html)
  and [NAIF SPICE](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/frames.html)
  values within the dataset's stated precision.
- Transform round trips (inertial → rotating → inertial) return the state;
  a transform that omits `ω × r` is caught by a velocity check.
- A query outside coverage, and a UTC time beyond the leap-second table,
  are refused.
- `aethyr/physics/orbit.coax`'s mean-element ephemeris is measured against the
  pinned table over a stated span, so its position error per body is a
  reported number, not an assumption.

## Dependencies

The Metaverse Engine [chapter 1](ch001-a-frame-has-a-fixed-step.md)'s clock and
[chapter 2](ch002-spaces-and-units.md)'s typed spaces and units. From Coax:
R12 units and frames, R13 typed clocks. It can start on plain f64 fields
and move onto those forms as they ship.
