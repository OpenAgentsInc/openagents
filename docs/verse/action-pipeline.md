# The action pipeline

Every action of every class and monster runs through one pipeline: ask, act,
present. The rules decide first. Nothing animates, sounds or emits an effect
until they accept the action. Conditions drive the body's pose, the input lock
and the bar from one place. Issue: openagents #13072.

## Why

Players saw these bugs:

- The bow draws but no arrow fires.
- A spell's particles fly, and then the game says you can't cast it.
- You are knocked out but still standing and casting.
- Dash does nothing.

They share two causes. Presentation starts on input, before the rules decide.
And conditions are numbers that the body, the input and the bar don't read.

## The three stages

1. **Ask.** A press asks whether the actor can act now:
   `deed_ask` (`aethyr/action/deed.coax`). The answer comes from the actor's
   bearing (below), whether the slot is cooling down and whether an earlier
   press still waits for its answer. A slot that can't act turns grey. No text
   banner appears.
2. **Act.** The authority resolves the action. In a room, the room resolves it
   and answers with its broadcast (accepted) or its refusal. In a local realm,
   such as Everglade Experimental, the local rules resolve it in the same frame.
   The realm hands the answer to `deed_answer`. An unanswered ask times out
   after 1.5 seconds and counts as refused.
3. **Present.** Only an accepted answer presents. `deed_show` turns the
   action's feel and its result (hit, miss, whiff or effect) into what to play:
   the animation, the effect, the sound and the movement. A refused answer
   greys the slot and plays nothing.

A held action, such as drawing a bow, asks when the hold starts. The draw is
the action's wind-up and plays only after a yes. The loose is the request that
the authority resolves.

## Bearing: conditions in one place

A `DeedBearing` says how a body holds itself and what it may do:

- `pose`: stand, prone, held, cower, reel, still or down. When several apply,
  the strongest wins.
- `lock`: bits for move, act, react and speak.
- `crawl`: movement costs extra (Prone).

The body picks its clip from the pose. The input reads `deed_can_move`. The
bar greys every slot while `deed_can_act` is false. Every realm reads the same
bearing, so a knocked-out hero lies down, can't move and can't cast, all at
once.

The SRD 5.2.1 mapping is `rs_bearing` in
`aethyr/rules/srd/condition-bearing.coax`. It agrees with the rules' own
`rs_can_act` and `rs_can_move`, and a check holds them together:

| SRD condition | Pose | Locks |
| --- | --- | --- |
| 0 Hit Points, dead, Unconscious | down | move, act, react, speak |
| Paralyzed, Petrified | still | move, act, react, speak |
| Stunned, Incapacitated | reel | act, react, speak |
| Grappled, Restrained | held | move |
| Prone | prone | none (crawl) |
| Frightened | cower | none |
| Command's Halt | stand | move, act |

## The feel contract

Every usable action declares its feel (`DeedFeel`):

- `anim`: the animation it plays, as an ID the game defines;
- `fx`: its effect recipe;
- `sound`: its starting sound;
- `move`: how the body moves: still, plant, step, lunge, dash or leap.

`deed_feel_missing` names what an action lacks. A check fails when any usable
action lacks any of the four. Everglade's table is `xf_feel` in
`apps/openagents-web/class-feel.coax`. Its check walks every class's bar at
every level from 1 to 6, and every monster action Everglade fields.

## Where the code lives

| Layer | Path | What |
| --- | --- | --- |
| Engine | `aethyr/action/deed.coax` | The bar, the ask and answer, the bearing type, the feel contract, the show |
| Engine (SRD rules) | `aethyr/rules/srd/condition-bearing.coax` | SRD conditions to a bearing |
| Game | `apps/openagents-web/class-feel.coax` | Everglade's feel for every class ability and monster action |
| Zone | `everglade2-cast.coax`, `experimental-combat.coax`, `everglade-spells.coax` | Each realm wires its press, its authority's answer and its bar to the pipeline |

Zones only wire it in. They hold no copies of the pipeline.

Everglade v1's page doesn't load the SRD engine. Its bearing comes from its
own status state (down, or stopped by a condition), and its aimed spells
declare their feel in `sp_feel`. Its refusals keep the cast bar's refusal
icon.

## For work in flight

- **Unconscious at 0 HP:** read `rs_bearing` (or `rs_bearing_of` from the
  room's row) for the pose and the locks.
- **The bow:** ask with `deed_ask` when the draw starts, and present the loose
  from the answer.
- **Damage numbers:** they follow `deed_show`, through `react_present`.
- **Dash and sprint:** Dash's feel declares `dash` movement. Start the sprint
  from the accepted answer.
- **Mounts and the fantasy HUD:** grey a slot from `deed_look`, and dim the
  bar while the bearing locks acting.

## Checks

```sh
coax/bin/coax test aethyr/action/deed.coax --library
coax/bin/coax test aethyr/rules/srd/condition-bearing-checks.coax --library
coax/bin/coax test apps/openagents-web/class-feel.coax --library
```
