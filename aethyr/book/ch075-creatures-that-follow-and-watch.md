# 75. Creatures that follow and watch

> **Status: unimplemented — design draft.** Today each realm writes its own
> creatures. Everglade's bird companion uses
> [`verse/flock.coax`](../../verse/flock.coax), its town watch decides who
> sees a crime in `verse/everglade/watch.coax`, a guard's cosmetic choice can
> come from a decision model (`verse/everglade/guard-*.coax`, gated by
> [`verse/test_guard_ai.sh`](../../verse/test_guard_ai.sh)), and Hecate's
> Cemetery's dog follows you by the garden paths. Each has its own rules for
> what a creature knows and when it acts. This chapter plans one perception
> and decision layer for all of them. Nothing in `aethyr/` runs yet.

A creature in a realm does three things each step: it senses, it decides and
it acts. This chapter separates those three, so a guard, a bird and a dog use
the same rules for sight and hearing, decide from what they sensed rather
than from the true state, and replay exactly.

## What this chapter teaches

- How to step creatures at a fixed rate, independent of the frame rate.
- How to model sight and hearing as typed stimuli with ranges and line of
  sight, built on [chapter 7](ch007-ask-the-world-where-things-are.md)'s
  queries.
- How to keep a creature's knowledge separate from the world's truth, as
  [chapter 45](ch045-sensors-that-see-the-past.md) does for spacecraft.
- How to write decisions as small state machines and utility choices whose
  every input is recorded.
- How to admit a remote decision model's answer only as a choice from a
  fixed set, never as free text or a move.

## Outline

1. **A fixed step for behaviour.** `flock.coax` steps at `FLOCK_STEP`
   (1/120 s) with at most `FLOCK_CATCH_UP` (30) steps a frame. Aethyr moves
   that rule onto [chapter 1](ch001-a-frame-has-a-fixed-step.md)'s clock, so
   creatures step with physics and never on frame time.
2. **Following someone.** The flock's model: each of `FLOCK_BIRDS` (7) birds
   has a place on a ring of radius `FLOCK_RING` (1.5 m), `FLOCK_HEIGHT`
   (2.9 m) over the target's feet, turning at `FLOCK_TURN` (0.9 rad/s). A
   critically damped spring at rate `FLOCK_PULL` (6.0) pulls each bird to its
   place, capped at `FLOCK_SPEED` (16 m/s) and `FLOCK_ACCEL` (40 m/s²).
   Birds closer than `FLOCK_APART` (0.7 m) push apart, and a bird more than
   `FLOCK_LOST` (30 m) from its place is put back. The chapter generalizes
   this to a `Follow` behaviour that the dog and made-realm herds also use.
3. **Stimuli.** Sight is a range and an eye height plus a line-of-sight
   query: the watch uses `WATCH_SIGHT` (34 m) and `WATCH_EYE` (1.6 m), with
   no building, wall or palisade between. Hearing is a range per sound
   source: `WATCH_HEAR` is 85 m, 60 m, 85 m and 22 m for the four spells.
   Each stimulus is a record with a kind, a source entity, a position and the
   step it was sensed.
4. **Knowledge, not truth.** A creature keeps a small memory of stimuli and
   forgets them after a stated time. Everglade's wanted state already works
   this way: out of every hunting guard's sight for `WANTED_LOSE` seconds,
   you are no longer wanted. A decision reads only this memory. A check
   refuses a behaviour that reads true positions directly.
5. **Decisions.** States and transitions as data: a guard walks its beat,
   turns on you, runs at `GUARD_RUN` (5 m/s, slower than your 7.5 m/s run),
   swings every `SWING_EVERY` (1.9 s) and returns to its beat. Utility
   scores choose among actions when several apply. Combat rolls use the
   realm's seeded dice, so a fight replays.
6. **Remote choices.** The Everglade guard consumer
   ([pylon-guard-decisions.md](../../docs/verse/pylon-guard-decisions.md))
   lets a calm guard choose quiet, a greeting or a weather remark. The model
   returns an enum; the game supplies the lines. At most one request is
   pending per realm. A late answer can't make a fallen or hostile guard
   greet. Aethyr keeps this rule for every remote decision.
7. **Budgets and sleep.** Far creatures step less often or sleep, with the
   rates declared per realm and counted by
   [chapter 16](ch016-budgets-that-refuse.md)'s budgets.
8. **Inspecting a decision.** Each decision records its inputs and the rule
   that fired, so you can replay a session and ask why a guard turned.

## What exists today

- [verse/flock.coax](../../verse/flock.coax): the following flock, used by
  Everglade's bird companion (`eg-companion`) and the made-realm `flock`
  feature.
- [verse/everglade/watch.coax](../../verse/everglade/watch.coax): the town
  watch, crimes, sight and hearing, wanted state and SRD 5.2.1 guards.
- [verse/everglade/guard-decision.coax](../../verse/everglade/guard-decision.coax)
  and [guard-types.coax](../../verse/everglade/guard-types.coax): the
  cosmetic guard decision, its observation (`GaObservation`: calm, alive,
  near, visible, safe, playable, day, weather) and effects.
- [verse/test_guard_ai.sh](../../verse/test_guard_ai.sh): builds the guard
  decision library in debug, release and sanitized modes and runs its
  native checks.
- [apps/openagents-web/villa.coax](../../apps/openagents-web/villa.coax):
  the dog, which follows you by the paths with `verse_follower_step`.
- [docs/research/game-engine-systems.md](../../docs/research/game-engine-systems.md),
  section 37: the perception, knowledge and decision inventory.

## What it builds

- `aethyr/ai/stimulus.coax`: sight, hearing and touch stimuli over chapter
  7's queries.
- `aethyr/ai/memory.coax`: per-creature knowledge with forgetting.
- `aethyr/ai/decide.coax`: state machines and utility choices as data.
- `aethyr/ai/follow.coax`: the flock's spring model as a general follow
  behaviour.
- `aethyr/ai/remote.coax`: admission of remote enum choices.

## Acceptance checks and oracles

The chapter is done when:

- The flock's existing `#test`s pass unchanged on `aethyr/ai/follow.coax`,
  and Everglade's companion looks the same in its golden.
- A creature behaves the same at 30, 60 and 144 FPS, bit for bit.
- A guard never reacts to a crime it could not see or hear, checked over a
  recorded session with walls between.
- A behaviour that reads true state instead of its memory fails a check.
- The guard decision gate passes in debug, release and sanitized modes, and
  a remote answer outside the enum, or for a guard that is no longer calm,
  is refused and counted.
- A recorded fight replays to the same digest
  ([chapter 5](ch005-replay-a-session-exactly.md)).

## Dependencies

Chapters 1 (the clock), 5 (replay), 6 (entities), 7 (queries), 16 (budgets)
and 45 (truth and observation kept apart). Psionic's public decision edge
([Psionic chapter 27](../../psionic/book/ch027-a-public-mesh.md)) for remote
choices. Coax R13 typed clocks for stimulus stamps.
