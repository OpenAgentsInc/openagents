# 102. Ask, act, present

> **Status: partly implemented.** The pipeline is
> [aethyr/action/deed.coax](../action/deed.coax) (#13072). The SRD mapping
> from conditions to a bearing is
> [rules/srd/condition-bearing.coax](../../rules/srd/condition-bearing.coax),
> and Everglade's feel table is
> [everglade/client/class-feel.coax](../../everglade/client/class-feel.coax).
> Everglade II, Everglade Experimental and Everglade v1's aimed spells wire
> their presses to it. The design note is
> [docs/verse/action-pipeline.md](../../docs/verse/action-pipeline.md).
> Not built yet: every zone's HUD parts, damage numbers, spell effects and
> class bars on shared modules (#13079 to #13088), and a runnable example in
> `aethyr/examples/`. This chapter names procedures and has no source
> listings yet.

A player presses a key to shoot a bow. Three things can go wrong:

- The bow draws, and no arrow fires.
- A spell's particles fly, and then the game says that you can't cast it.
- A hero at 0 Hit Points still stands and casts.

Players saw all three in Everglade. They have two causes. Presentation
started on the key press, before the rules decided. And a condition was a
number that the body, the input and the hotbar each read in their own way,
or didn't read.

This chapter builds one pipeline that every action of every class and
monster runs through, and one record that says what a body may do.

## What this chapter teaches

- Why nothing animates, sounds or emits an effect until the rules accept
  the action.
- How an action asks, waits for the authority's answer and presents.
- How one record, the bearing, drives the pose, the input lock and the
  hotbar from a body's conditions.
- How a feel contract makes a missing animation or sound a failed check
  and not a silent action.

## The method

### Three stages

1. **Ask.** A press asks whether the actor can act now. `deed_ask` answers
   from the actor's bearing, the slot's cooldown and whether an earlier
   press still waits for its answer. A slot that can't act turns grey. No
   text banner appears.
2. **Act.** The authority resolves the action. In a room, the room resolves
   it and answers with its broadcast or its refusal
   ([chapter 96](ch096-casts-and-combat-under-latency.md)). In a local
   realm, the local rules resolve it in the same frame. The realm passes the
   answer to `deed_answer`. An ask with no answer after 1.5 seconds counts
   as refused.
3. **Present.** Only an accepted answer presents. `deed_show` turns the
   action's feel and its result (hit, miss, whiff or effect) into what to
   play: the animation, the effect, the sound and the movement. A refused
   answer greys the slot and plays nothing.

A held action, such as drawing a bow, asks when the hold starts. The draw
is the action's wind-up, and it plays only after a yes. The release is the
request that the authority resolves.

The page and the room must reach the same answer for a legal press, so the
ask reads only state that both sides hold: the bearing, the cooldown and
the pending ask.

### Bearing: conditions in one place

A `DeedBearing` says how a body holds itself and what it may do:

- `pose`: stand, prone, held, cower, reel, still or down. When several
  apply, the strongest wins.
- `lock`: bits for move, act, react and speak.
- `crawl`: movement costs extra.

Three readers use the same bearing:

- The body picks its animation clip from `pose`.
- The input reads `deed_can_move`.
- The hotbar greys every slot while `deed_can_act` is false.

So a hero who is knocked out lies down, can't move and can't cast, all at
once, in every realm.

The engine defines the bearing. A rule set maps its own conditions onto it.
For SRD 5.2.1 the mapping is `rs_bearing`:

| SRD condition | Pose | Locks |
| --- | --- | --- |
| 0 Hit Points, dead, Unconscious | down | move, act, react, speak |
| Paralyzed, Petrified | still | move, act, react, speak |
| Stunned, Incapacitated | reel | act, react, speak |
| Grappled, Restrained | held | move |
| Prone | prone | none; movement crawls |
| Frightened | cower | none |
| Command's Halt | stand | move, act |

### The feel contract

Every usable action declares its feel in a `DeedFeel`:

- `anim`: the animation it plays, as an ID that the game defines.
- `fx`: its effect recipe.
- `sound`: its starting sound.
- `move`: how the body moves: still, plant, step, lunge, dash or leap.

`deed_feel_missing` names what an action lacks. A game's check walks every
action it fields and fails when any one lacks any of the four. An action
with no sound is then a build failure, found before a player finds it.

## A worked example: a knocked-out wizard presses Fire Bolt

A wizard drops to 0 Hit Points and presses the Fire Bolt slot.

1. `rs_bearing` maps 0 Hit Points to pose `down` with the move, act, react
   and speak locks.
2. The body reads `down` and plays the fallen clip.
3. The hotbar reads `deed_can_act`, which is false, and greys every slot.
4. The press reaches `deed_ask`. The act lock is set, so the ask is
   refused. Nothing is sent to the room.
5. Nothing is presented. No cast animation, no particles, no sound.

Before the pipeline, step 4 did not exist. The page started the cast
animation and the particles on the press, sent the cast, and the room
refused it.

Now the same wizard, standing, presses Fire Bolt with the slot ready:

1. `deed_ask` accepts and marks the slot pending.
2. The room checks range and sight and broadcasts the cast.
3. The page passes the broadcast to `deed_answer`.
4. `deed_show` reads Fire Bolt's feel and the result, and plays the cast
   animation, the bolt, the sound and a planted stance.

If the broadcast never arrives, the ask times out after 1.5 seconds. The
slot greys and nothing plays.

## Where the code lives

| Layer | Path | What |
| --- | --- | --- |
| Engine | `aethyr/action/deed.coax` | The bar, the ask and the answer, the bearing type, the feel contract and the show |
| Rules | `rules/srd/condition-bearing.coax` | SRD conditions to a bearing |
| Game | `everglade/client/class-feel.coax` | Everglade's feel for every class ability and monster action |
| Zone | `everglade/client/cast.coax`, `everglade/zone-combat.coax`, `everglade-spells.coax` | Each zone wires its press, its authority's answer and its bar to the pipeline |

A zone only wires the pipeline in. It holds no copy of it. Features that
more than one zone needs belong in the engine or in the game's shared
modules, never in one zone.

## Checks

```shell
coax/bin/coax test aethyr/action/deed.coax --library
coax/bin/coax test rules/srd/condition-bearing-checks.coax --library
coax/bin/coax test everglade/client/class-feel.coax --library
```

- The first checks the ask, the answer, the timeout and the show.
- The second holds `rs_bearing` together with the rules' own `rs_can_act`
  and `rs_can_move`, so the body and the rules can't disagree about a
  condition.
- The third walks every class's bar at every level from 1 to 6, and every
  monster action Everglade fields, and fails when one lacks a feel.

## What remains

- A runnable example in `aethyr/examples/` with a transcript: the two
  traces above, and a broken case where an action with no sound fails
  `deed_feel_missing`.
- Shared modules for what each zone still copies: damage numbers, name
  plates, the camera, spell effects, Dash, class bars, HUD parts, the
  condition display, area targeting and the bow (#13079 to #13088).
- Everglade v1's page doesn't load the SRD engine. Its bearing comes from
  its own status state, and its refusals keep the cast bar's refusal icon.

## Dependencies

[Chapter 4](ch004-input-becomes-typed-actions.md) (typed actions),
[chapter 14](ch014-figures-and-animation.md) (clips),
[chapter 15](ch015-ui-and-the-hud.md) (the hotbar) and
[chapter 96](ch096-casts-and-combat-under-latency.md) (the room's answer).
The [Everglade book](../../everglade/book/README.md) uses this pipeline for
its conditions, its guard and its action combat.
