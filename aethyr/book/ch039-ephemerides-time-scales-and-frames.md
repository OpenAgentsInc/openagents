# 39. Ephemerides, time scales and frames

> **Status: unimplemented — design draft.** Aethyr's space games already place
> the Sun, planets, the Moon, Ceres and Vesta from J2000 mean elements
> (`orbit_body` and `orbit_body_state` in [`aethyr/orbit.coax`](../../aethyr/orbit.coax)),
> on one time axis with no named scale or frame. This chapter plans a pinned
> ephemeris with recorded time scales and full state transforms. Nothing here
> runs yet.

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

## What exists today

[`aethyr/orbit.coax`](../../aethyr/orbit.coax) holds `Orbit` (a, e, i, node,
periapsis, mean anomaly at epoch, mu) and `State` (r, v), with
`orbit_state`, `orbit_from_state` and `orbit_propagate`. `orbit_body(k)`
returns J2000 mean elements for the Sun, Mercury to Saturn, the Moon about
the Earth, Ceres and Vesta, with DE440 GM and radii; `ORBIT_2200` is
1 January 2200. Its checks put the Earth at 1 AU with a 365.25-day year in
2200 and the Moon 384,400 km from it. Time is a plain f64 of seconds with
no scale, frames are implicit (ecliptic, heliocentric) and there is no
light-time correction ([space primitives](../../docs/verse/space.md)).

## What it builds

- `aethyr/space/time.coax`: `Epoch`, `TimeScale`, the leap-second table as
  pinned data, and TAI/TT/TDB/UTC conversions that refuse out-of-table times.
- `aethyr/space/frames.coax`: frame identities and full state transforms,
  with rotation models for the bodies used.
- `aethyr/space/ephemeris.coax`: a Horizons vector-table reader, one SPK
  segment type, coverage and dataset identity, and the three state kinds.
- `aethyr/space/fixtures/`: retrieved Horizons/SPICE tables with their query
  parameters and digests.

## Acceptance checks and oracles

- States and light-time/aberration corrections agree with independently
  retrieved [JPL Horizons](https://ssd.jpl.nasa.gov/horizons/manual.html)
  and [NAIF SPICE](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/frames.html)
  values within the dataset's stated precision.
- Transform round trips (inertial → rotating → inertial) return the state;
  a transform that omits `ω × r` is caught by a velocity check.
- A query outside coverage, and a UTC time beyond the leap-second table,
  are refused.
- `aethyr/orbit.coax`'s mean-element ephemeris is measured against the
  pinned table over a stated span, so its position error per body is a
  reported number, not an assumption.

## Dependencies

Aethyr [chapter 1](ch001-a-frame-has-a-fixed-step.md)'s clock and
[chapter 2](ch002-spaces-and-units.md)'s typed spaces and units. From Coax:
R12 units and frames, R13 typed clocks. It can start on plain f64 fields
and move onto those forms as they ship.
