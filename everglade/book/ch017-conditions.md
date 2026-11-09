# 17. Conditions

> **Status: partly implemented.** The SRD 5.2.1's conditions are bits on a
> creature (`RS_BLINDED` to `RS_UNCONSCIOUS` in
> [`aethyr/rules/srd/engine.coax`](../../aethyr/rules/srd/engine.coax)). What
> each one permits is
> [`aethyr/rules/srd/condition-permissions.coax`](../../aethyr/rules/srd/condition-permissions.coax),
> and what it does to rolls is
> [`aethyr/rules/srd/condition-rolls.coax`](../../aethyr/rules/srd/condition-rolls.coax).
> Exhaustion and Prone have their own files. The Everglade room shows
> conditions through
> [`apps/openagents-web/everglade-status.coax`](../../apps/openagents-web/everglade-status.coax).
> Other agents are extending the rules files now, so this chapter's only
> listing is the status record, which already has markers.

Everglade aims to follow the SRD 5.2.1's conditions faithfully while adapting
them for action combat. This chapter answers the player's question: "Why can't
I act?"

A condition changes what you may do and how your rolls go. In real time, you
must also see it: on your character, on other players and in the line under
your Hit Points.

## What this chapter teaches

- How the fifteen conditions are stored and checked.
- Why moving and acting are separate permissions.
- How conditions change attack rolls, saving throws and damage.
- How a condition is shown, so that you know why you can't act.

## What exists today

| Rule | Where |
| --- | --- |
| The conditions, one bit each: Blinded, Charmed, Deafened, Exhaustion, Frightened, Grappled, Incapacitated, Invisible, Paralyzed, Petrified, Poisoned, Prone, Restrained, Stunned, Unconscious | `RS_CONDITION` in `engine.coax` |
| What you may do: move, take actions, take Reactions | `condition-permissions.coax` |
| Attack edges, automatic failures, critical hits and damage defenses | `condition-rolls.coax` |
| Exhaustion's six levels | [`aethyr/rules/srd/exhaustion.coax`](../../aethyr/rules/srd/exhaustion.coax) |
| Standing up and crawling | [`aethyr/rules/srd/prone-movement.coax`](../../aethyr/rules/srd/prone-movement.coax) |
| What the page draws and says about a condition | `st_fill` and `st_draw_on` in `everglade-status.coax`, `st_draw` in `apps/openagents-web/everglade-storm.coax` |

## What follows the SRD exactly

- **Moving is not acting.** Stunned imposes Incapacitated, not Speed 0, so a
  Stunned creature can't take actions but can still walk. Another condition,
  such as Grappled or Paralyzed, still stops movement.
- **Rolls.** A Prone attacker has Disadvantage. A ranged attack against a Prone
  target within 5 feet has Advantage from Prone and Disadvantage from attacking
  at close range, and the two cancel. A hit on a Paralyzed or Unconscious creature from within 5 feet
  is a Critical Hit, and one from farther away is not.
- **Automatic failures.** A Paralyzed, Petrified, Stunned or Unconscious
  creature fails Strength and Dexterity saves without rolling, and spends no
  rescue die on them.
- **Damage defenses.** Immunity applies first, then Resistance once, then
  Vulnerability, for all thirteen damage types.
- **Exhaustion.** Each level takes 2 from d20 tests and 5 feet from Speed. Six
  levels kill.
- **Concentration.** A condition that incapacitates you ends your
  concentration at once.

## What changes for action combat, and why

- **Walking is unpaid, but still permitted only when you may move.** In
  Everglade's world profile, walking doesn't spend the turn's feet
  ([chapter 18](ch018-turns-become-time.md)). A Grappled creature still can't
  walk at all.
- **Repeat saves keep the 6-second round.** Turns are 1.5 seconds in the action
  timing, but a creature repeats a failed save against a condition at the end
  of each 6-second round, not at the end of each short turn. Otherwise a
  condition would last a quarter as long.
- **Standing up costs half of the beat's movement**, not half your Speed. A
  1.5-second turn grants only a quarter of your Speed, so half a full Speed
  would be more than the beat has.
- **Five feet is 1.524 m.** The server measures "within 5 feet" in metres.
- **You see every condition.** The room sends each player's condition bits and
  each effect's cause and time left. The page draws them on the body, such as
  Hold Person's golden rings, and names them under your Hit Points, such as
  `HELD · HOLD PERSON · 2 S`.

One record holds what the page knows about a condition on a body:

<!-- source: apps/openagents-web/everglade-status.coax status-effect -->
```jai
// An effect on a body: WHO (ST_YOU, or a TGT code: 1000 + SS a seat, 0-999 a watch actor, 2000 + EID a
// creature), its condition bits (WC_*), the spell that made it (ST_*; -1 not said) and its seconds left (-1 held up, or not said).
StEffect :: struct { who: int = -1; cond: int; cause: int = -1; left: f32 = -1; }
EgStatus :: struct { n: int; e: [ST_MAX] StEffect; }
```

Everything that the page draws or says about a condition reads this record
only, so the drawing, the line under your Hit Points and whether you may move
can't disagree.

## A worked example: standing up from Prone

A Fighter with Speed 30 starts a turn Prone.

```text
strict turn:   grant = 30 ft = 6 squares
               stand = half of 30 ft = 15 ft = 3 squares
               left  = 3 squares

action beat:   grant = 10 ft = 2 squares        (a quarter-round, rounded to whole squares)
               stand = half of 10 ft = 5 ft = 1 square
               left  = 1 square
```

In both, standing up leaves half the turn's movement and uses no Action or
Bonus Action. Without the beat's own price, standing would cost 15 feet in a
turn that has 10, and you couldn't stand at all.

## The checks

- `aethyr/rules/srd/condition-permission-checks.coax`, `#test "accepted
  Stunned ends concentration and Dodge but allows walking and standing without
  actions"`.
- `aethyr/rules/srd/condition-roll-checks.coax`, `#test "a nearby ranged weapon
  hit on Paralyzed or Unconscious is critical and a more distant hit is
  ordinary"` and `#test "damage defenses apply immunity once resistance once
  then vulnerability for all thirteen damage types"`.
- `aethyr/rules/srd/exhaustion-checks.coax`, `#test "Exhaustion accumulates to
  six, kills without damage and interrupts owned effects"`.
- `aethyr/rules/srd/timing-checks.coax`, `#test "strict and action standing
  share a policy with an explicitly different movement price"`: the worked
  example.
- `aethyr/rules/srd/timing-checks.coax`, `#test "short action turns renew
  reactions but death saves and repeat saves remain six-second events"`.
- `apps/openagents-web/everglade-status.coax`, `#test "status: one word over a
  head while a condition lasts, the strongest: DOWN, HELD, STUNNED, ROOTED,
  RESTRAINED, PRONE, FRIGHTENED (#12862)"`.

```sh
coax/bin/coax test --library aethyr/rules/srd/condition-roll-checks.coax
coax/bin/coax test --library aethyr/rules/srd/timing-checks.coax
```

## Mistakes

- **Stunned as a freeze.** It is easy to treat Stunned as "can't do anything".
  In SRD 5.2.1 it doesn't stop movement. The permission checks hold movement
  and actions apart, and assert that a Stunned creature can still walk.
- **A condition you can't see.** A Hold Person with no rings looks like a
  frozen game. The status record drives every drawing and line, and the status
  test checks the one word shown over a held player's head.

## What comes next

[Chapter 18](ch018-turns-become-time.md) turns the SRD's rounds into time.
