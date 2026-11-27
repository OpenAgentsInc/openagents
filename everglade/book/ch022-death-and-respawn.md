# 22. Death and respawn

> **Status: partly implemented.** Server-authoritative 0 HP transitions,
> death-save timing on the 6-second SRD round, Short/Long Rest recovery
> (`Z`/`z`), seat reset, and progress persistence live in
> [`everglade/server/battle.coax`](../server/battle.coax),
> [`everglade/server/rest.coax`](../server/rest.coax),
> [`everglade/server/seat.coax`](../server/seat.coax), and
> [`everglade/server/progress.coax`](../server/progress.coax). A standalone
> runnable book example and dedicated fade-in respawn animation capture remain
> to be extracted.

This chapter answers the player's question: "I hit 0 Hit Points—am I dead,
can someone heal me, and where do I wake up?"

Real-time combat must handle 0 HP cleanly without trapping a player on a frozen
screen or letting a client forge a free heal. The room server owns every state
transition from falling Unconscious to standing back up at the glade—and also
owns Short and Long Rests so recovering HP and Hit Dice obeys the SRD clock.

## What this chapter teaches

- How 0 HP triggers `RS_UNCONSCIOUS`, breaks Concentration, dismounts riders
  immediately, and ticks death saves on the SRD 6-second round.
- How allies stabilize or heal a downed player before three failures accumulate.
- How Short and Long Rests (`everglade/server/rest.coax`) verify that a player
  is grounded and motionless on the room's own step before admitting recovery.
- How respawn resets position, clears transient combat conditions, and records
  durable vitals through the checkpoint path ([chapter 13](ch013-coming-back.md)).

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| 0 HP & death-save rules | SRD Unconscious, instant-death overflow, 6-second death saves, Concentration drop | [`everglade/server/battle.coax`](../server/battle.coax), `rules/srd/hit-points.coax` |
| Authoritative rests | `e2_rest_step`, `e2_rest_settled`, `e2_heard_rest` (`Z SEQ REQ` → `z SEQ CODE DICE`) | [`everglade/server/rest.coax`](../server/rest.coax) |
| Seat lifecycle & respawn | Seat spawn, reset, and departure cleanup | [`everglade/server/seat.coax`](../server/seat.coax), [`everglade/server/seats.coax`](../server/seats.coax) |
| Progress & vitals carry | Carries XP, HP, and rest history across reconnects and checkpoints | [`everglade/server/progress.coax`](../server/progress.coax) |

## The method

### Death saves stay on the 6-second round

Although action turns run every 1.5 seconds ([chapter 18](ch018-turns-become-time.md)),
death saving throws remain on the SRD's 6-second round so a downed player gives
allies a real 18-second window to reach them with *Cure Wounds* or *Healing
Word*. Falling to 0 HP immediately ends Concentration and removes the seat from
any mount (`everglade/server/riding.coax`), preventing a dead rider from
floating on a flying dragon.

### Rests require server-observed stillness

To prevent moving while regenerating, `e2_heard_rest` never trusts a client's
claim that it stopped walking. `e2_rest_step` inspects the room's own stepped
pose (`was` → `p`) and requires `grounded && |v| < 0.01 && !keys && !jump`,
plus zero movement keys in any queued inputs (`e2_rest_settled`). Any
subsequent movement calls `rs_rest_observe_motion`, immediately interrupting a
Short Rest or turning a Long Rest's watch into exertion. Legacy whole-heal `L`
frames (`e2_heard_old_rest`) are explicitly refused (`eg_refuse(..., "rest")`).

## Worked trace: rest admission and interruption

```text
1. Player releases keys; room steps pose with |v| < 0.01 -> rest_settled = true
2. Client sends `Z 42 1` (Short Rest) -> e2_rest_settled == true -> admits rest (`z 42 ...`)
3. Player taps W before rest finishes -> walked == true -> rs_rest_observe_motion interrupts rest
4. Client sends retired v1 `L 43`     -> e2_heard_old_rest refuses (`refused: rest`)
```

## The checks

```shell
coax/bin/coax test coax/web/room/room.coax --only "e2 rest" --steps 4000000000
coax/bin/coax test --library rules/srd/timing-checks.coax
```

## What comes next

Part VII turns to aiming and action combat. [Chapter 23](ch023-mouse-look-and-the-crosshair.md)
gives the player mouse look, a crosshair, and Tab cursor release.
