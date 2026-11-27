# 25. Cast while you run

> **Status: partly implemented.** Shared upper-body cast clip selection,
> held-weapon bow layering, client cast prediction, and room cast pacing live
> in [`everglade/client/cast-pose.coax`](../client/cast-pose.coax),
> [`everglade/held-kit.coax`](../held-kit.coax),
> [`everglade/client/cast.coax`](../client/cast.coax),
> [`everglade/shared/cast-pace.coax`](../shared/cast-pace.coax), and
> [`everglade/server/casts.coax`](../server/casts.coax). Layered skeleton masks
> live in [`metaverse-engine/anim/layers.coax`](../../metaverse-engine/anim/layers.coax)
> and [`metaverse-engine/anim/masks.coax`](../../metaverse-engine/anim/masks.coax).
> A standalone runnable book example remains to be extracted.

This chapter answers the player's question: "Why can I cast *Fire Bolt* while
running down the path without my legs sliding like a statue?"

Action combat breaks down if casting roots your feet—or if playing a full-body
cast animation freezes your legs while your capsule slides forward at 4.25 m/s.
Everglade splits body animation into lower-body locomotion and upper-body
action layers while enforcing SRD turn pacing in the room.

## What this chapter teaches

- How `everglade/client/cast-pose.coax` selects one clip per kit and weapon
  type (`BD_SLASH`, two-handed `BD_OWN + hold + 2` cleave, or spell cast)
  across both Experimental and Everglade II.
- How `held_bow_layer` and `metaverse-engine/anim/layers.coax` blend an
  upper-body aim or cast pose over walking and running legs using joint masks.
- How the server enforces one Action, one Bonus Action, and one leveled spell
  slot per 1.5-second turn (`everglade/shared/cast-pace.coax`) with 3 m aim
  slack (`EG_E2_CAST_SLACK`) while you move.

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| Cast clip selection | `cp_blow_clip(kit, held)` maps kit & held two-hander to slash vs cleave vs cast | [`everglade/client/cast-pose.coax`](../client/cast-pose.coax) |
| Held kit & bow layer | `held_bow_layer` layers upper-body bow draw and held weapon transforms over locomotion | [`everglade/held-kit.coax`](../held-kit.coax) |
| Engine animation layers | Joint masks and multi-layer pose blending (`metaverse-engine/anim/`) | [`metaverse-engine/anim/layers.coax`](../../metaverse-engine/anim/layers.coax), [`metaverse-engine/anim/masks.coax`](../../metaverse-engine/anim/masks.coax) |
| Client cast prediction | Plays cast pose immediately, sweeps hotbar cooldown, lifts sweep on refusal | [`everglade/client/cast.coax`](../client/cast.coax) |
| Server cast authority | Validates class kit (`xk_entry_held`), turn budget (`cast-pace.coax`), and `EG_E2_CAST_SLACK` (`3.0 m`) | [`everglade/server/casts.coax`](../server/casts.coax), [`everglade/shared/cast-pace.coax`](../shared/cast-pace.coax) |

## The method

### Upper-body action over lower-body locomotion

The base layer advances the walk, jog, or jump cycle by ground distance so feet
stay planted on the dirt path. When `e2k_press` triggers a spell or swing,
`cp_blow_clip` selects the upper-body clip (for a held two-hander, its owned
cleave; otherwise slash or cast), and the animation layer mask blends spine,
arms, and head onto that clip while leaving pelvis and leg joints driven by
movement. Drawing a Longbow uses the exact same upper-body layer
(`held_bow_layer`), letting a Ranger strafe while aiming.

### Movement slack matches the 30 Hz wire

Because you keep moving while your cast packet travels to the server,
`everglade/server/casts.coax` checks range with `EG_E2_CAST_SLACK` (`3.0 m`)
around the room's stepped position, validates the slot against the seat's
class kit and level (`xk_entry_held`), and checks that the seat's current
1.5-second turn still has its Action or Bonus Action free (`cast-pace.coax`).
Only after shared SRD execution commits does the room broadcast
`e cast SS SLOT AX AY AZ SEED` to every seat within `EG_VIEW`. If the room
refuses (`eg_refuse(..., "slot")` or `"pace"`), the client cancels the
predicted cooldown sweep and shows the reason icon over the hotbar.

## Worked trace: turn budget during a moving cast

```text
t = 0.0 s  Player runs at 4.25 m/s, presses Action cantrip (slot 1)
           -> Turn opens (1.5 s window); Action spent; Bonus Action still open
           -> Upper body plays cast clip; legs keep 4.25 m/s run cycle
t = 0.4 s  Player presses Bonus Action (Healing Word) while still moving
           -> Bonus Action spent; Action + Bonus Action both resolved in one 1.5 s turn
t = 0.7 s  Player presses Action cantrip again inside the same 1.5 s window
           -> Room refuses (`pace`); client lifts predicted cooldown sweep
t = 1.5 s  Next turn window opens; Action and Bonus Action refresh
```

## The checks

```shell
coax/bin/coax test --library everglade/client/client.coax --only "cast pose"
coax/bin/coax test --library everglade/shared/cast-rules-checks.coax
coax/bin/coax test --library metaverse-engine/anim/layers-checks.coax
```

## What comes next

Part VIII steps out of the woods into the city. [Chapter 26](ch026-walk-into-a-city.md)
carves the city clearing, lays out the 16-plot town grid and staged Housewright
buildings, and pins every gate and landmark to one coordinate table.
