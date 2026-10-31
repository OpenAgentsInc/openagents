# 71. Sky, weather and the time of day

> **Status: unimplemented — design draft.** Today each realm keeps its own
> clock of the day and its own sky: Everglade's 48-minute day and weather
> schedule, Fairy Forest's day and night in
> [`verse/fairy-forest/night.coax`](../../verse/fairy-forest/night.coax),
> Hecate's Cemetery's moon in
> [`apps/openagents-web/villa.coax`](../../apps/openagents-web/villa.coax),
> and the space realms' starfield in [`verse/stars.coax`](../../verse/stars.coax).
> This chapter plans one realm environment clock that drives the sky, the
> weather and [chapter 13](ch013-light-and-shadow.md)'s lighting record.
> Nothing in `aethyr/` runs yet.

The time of day and the weather change how every surface looks and some of
how the world behaves: who is outdoors, how high the river runs, how far
you can see. If each is a pure function of the realm's clock and seed,
every visitor, a late joiner and a replay see the same sky with no extra
messages.

## What this chapter teaches

- How to declare a realm's day clock: real time, a cycle of fixed length,
  or a fixed time for goldens.
- How to compute the sun's elevation and the moon's phase from the clock.
- How to schedule weather from a climate, a seed and the world tick, with
  local overlays that never change the schedule.
- How to pin the sky for tests and goldens.

## Outline

1. **The day clock.** A realm declares its clock in `realms.json` as
   `real` (the visitor's local time), `cycle` (for example a 48-minute day,
   `FFN_CYCLE_SECONDS` 2880) or `fixed:T` (0 midnight, 0.5 noon). A
   `?clock=T` page fact pins a shot. Goldens are always noon. The day clock
   reads [chapter 1](ch001-a-frame-has-a-fixed-step.md)'s simulation time;
   it never owns time.
2. **Sun and moon.** The sun's elevation runs from `sun_low` at midnight to
   `sun_high` at noon as `0.5 (low + high) − 0.5 (high − low) cos(2π clock)`
   (`verse_style_sun_elevation` in [`verse/style-preset.coax`](../../verse/style-preset.coax)).
   The moon's phase is the age since the mean new moon of 6 January 2000
   18:14 UTC over the mean synodic month, 29.530588853 days
   (`villa_moon_phase`); the lit share of the disc is `(1 − cos 2π phase) / 2`.
   Only one of them leads as the key light.
3. **Palette through the day.** Style keys at four clock keys (midnight,
   dawn, noon, dusk) mix by the clock. Dusk warms a palette and night takes
   it to a blue dusk (`verse_style_flat_time` in
   [`verse/style.coax`](../../verse/style.coax)).
4. **The sky.** A painted sky and haze per style; for space, star layers in
   screen cells picked by a hash, with twinkle, and a Sun of disc, corona
   and halo (`verse_star_layer`, `verse_sun_disc`, `verse_sun_corona`).
5. **The weather schedule.** Ten-minute blocks draw clear, overcast, fog,
   rain or storm from a climate's weights (`weather_temperate`,
   `weather_coastal`), blend over 60 s, and vary rain and wind with seeded
   smooth noise ([`verse/water-weather.coax`](../../verse/water-weather.coax)).
   Strong wind starts at 9 m/s and disperses fog.
6. **Weather on the ground.** Wetness and puddles integrate over the last
   hour on a 30 s grid fixed to the tick. Water rises by the mean rain over
   the last 30 minutes times 0.15 m, and currents speed up by up to 1.6
   ([chapter 70](ch070-rivers-ponds-and-the-sea.md)).
7. **Local overlays.** A `WeatherOverlay` (a spell's rain, fog cloud or
   storm) is an area with a start tick and a duration. It changes what is
   drawn and felt inside its area and never changes the schedule.
8. **Routines on the clock.** People's days are rows of time of day, place
   and route, so where they are is a pure function of the clock
   ([`verse/everglade/routines.coax`](../../verse/everglade/routines.coax)).
9. **Pinning.** `?weather=clear|overcast|fog|rain|storm` pins the weather; a
   still page keeps a calm sky unless pinned, so goldens don't change with
   the schedule.

## What exists today

- [verse/style-preset.coax](../../verse/style-preset.coax) and
  [verse/style.coax](../../verse/style.coax): style keys per clock key, sun
  elevation, haze and light shafts.
- [verse/water-weather.coax](../../verse/water-weather.coax): the weather
  schedule, SRD effects, overlays, ground wetness and rising water.
- [verse/fairy-forest/night.coax](../../verse/fairy-forest/night.coax): the
  three clock modes and resting to night or morning.
- [apps/openagents-web/villa.coax](../../apps/openagents-web/villa.coax):
  the moon's phase and its fixed direction, about 33 degrees up.
- [verse/stars.coax](../../verse/stars.coax): the starfield and the Sun for
  space backgrounds.

## What it builds

- `aethyr/world/day.coax`: the day clock, its modes and pinning.
- `aethyr/world/sky.coax`: sun elevation, moon phase and the sky inputs to
  the lighting record.
- `aethyr/world/weather.coax`: the schedule, overlays and ground state.

## Acceptance checks and oracles

The chapter is done when:

- Two clients and a late joiner at the same tick compute the same weather
  state and ground wetness, bit for bit.
- The moon's phase matches an independent ephemeris computation within a
  stated tolerance for a set of dates.
- At each clock key the key light and palette equal the preset's values,
  and the sun and moon never both lead.
- A weather overlay leaves the schedule unchanged before, during and after
  it.
- Goldens render with the pinned clock and weather and don't change when
  the schedule moves.
- Everglade in a storm stays inside its phone-tier GPU budget.

## Dependencies

Chapters 1 (the clock), 5 (replay), 12 (materials) and 13 (the lighting
record). Chapter 70 for rising water. From Coax: R13 typed clocks for civil
and simulation time.
