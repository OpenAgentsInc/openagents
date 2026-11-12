# 2. What the prototypes taught

> **Status: partly implemented.** Every prototype in this chapter runs today:
> Everglade v1 at `/worlds/everglade`, Everglade Experimental at
> `/worlds/everglade-experimental` and Character Create at
> `/worlds/character-creator`. Their code is in `apps/openagents-web/` and
> `verse/`, so this chapter names functions and has no source listings yet.

Everglade aims to follow the SRD's rules faithfully while adapting them for
action combat. Three prototypes came before Everglade II, and each answered one
question. This chapter answers the question a builder asks before starting
over: "What do I keep, and what do I throw away?"

## What this chapter teaches

- What each prototype settled.
- Which pieces Everglade II keeps, and which it rebuilds.
- One mistake from the test bench, and the check that now catches it.

## Everglade v1: a whole town in a browser

Everglade v1 ([`docs/verse/everglade.md`](../../docs/verse/everglade.md)) has a
gate, walls, townspeople, a raid and SRD combat, and a room that every player
shares. It showed that all of that runs in a browser tab.

What v1 settled, and Everglade II keeps:

- **One step for the page and the server.**
  [`everglade/shared/everglade-step.coax`](../../everglade/shared/everglade-step.coax)
  holds `eg_step_begin` and `eg_step_advance_among`. The page and the room
  server both call them, so your movement on your screen and on the server
  agree. [Aethyr chapter 87](../../aethyr/book/ch087-server-and-client-share-one-step.md)
  builds this step.
- **Casting under latency.** v1's room checks range and line of sight when a
  cast starts and when it releases, with some slack for the time in between
  ([`apps/openagents-web/everglade-cast-rules.coax`](../../apps/openagents-web/everglade-cast-rules.coax)).
  [Chapter 16](ch016-spells-and-slots.md) shows those rules.

What Everglade II leaves behind: v1's town, map and speeds. v1 runs at
7.5 m/s and walks at 3.5 m/s. Everglade II jogs at 4.25 m/s and walks at
1.5 m/s. It sets its own pace on the shared step (`e2_pace` in
[`everglade/shared/move.coax`](../shared/move.coax)),
so v1 keeps its speeds. v1 stays frozen.

## Everglade Experimental: the test bench

Everglade Experimental
([`docs/verse/everglade-experimental.md`](../../docs/verse/everglade-experimental.md))
is where new systems prove themselves first: hostiles, a dungeon, imported
characters and action combat.

What Experimental settled, and Everglade II keeps:

- **One declared rules profile.** Experimental calls
  `rs_set_world(b, RS_EVERGLADE_WORLD)` once
  (`everglade/zone-combat.coax`). The profile names every way
  the world departs from the SRD ([chapter 18](ch018-turns-become-time.md)).
  Everglade II uses the same profile.
- **The jump's timing.** A jump plays its takeoff over 0.2 s, loops while you
  are in the air and lands over 0.35 s, blended over the run at 0.45. These
  values came from Experimental (`charactercreator.coax`).

What Experimental taught by failing:

- **Bounds are not shapes.** Experimental approximates a non-box collider by
  its bounds. That works for a house and fails for a rock, whose bounds are
  bigger than what you see. [Chapter 8](ch008-if-you-can-see-it-you-can-walk-to-it.md)
  gives Everglade II's rocks their own hulls instead.

## Character Create: the glade

Character Create ([`docs/verse/character-creator.md`](../../docs/verse/character-creator.md))
is a wardrobe for the Quaternius people in a sunlit glade. You pick one of the
twelve SRD classes there.

It settled the look. Its glade is the most finished thing the engine renders,
and its light became the default for the art style
([chapter 6](ch006-the-glades-light.md)). Everglade II starts from the glade and
builds outward: one path, then a clearing, then a city.

What Everglade II leaves behind: the glade's own grass. At about 35 million
vertices a frame, it cost about 10 ms of GPU time, so Everglade II draws blade
grass instead ([chapter 10](ch010-hold-60-frames-per-second.md)).

## A worked example: the arrows nobody could see

After the hostiles landed (#13034), Experimental's village had a Goblin Warrior.
This is what a level 1 hero met before commit `9f9ba23ccf`:

```text
t = 0.00 s   you spawn, a level 1 hero
t = 0.75 s   a Goblin 9.4 m away shoots its Shortbow at you
             nothing flies, nothing sounds
t = 2.8 s    a Wizard is dead
```

The SRD part was right: each shot was a ranged attack roll against your Armor
Class, and the rolls hit. The action-combat part was missing. In real time, you
can only react to an attack that you can see and hear coming. An attack with no
arrow is a hidden die roll.

The fix (`verse/experimental-hostiles.coax`,
`everglade/zone-combat.coax`, `aethyr/fx/puff.coax`) makes
every shot a thing in the world:

- A Shortbow looses an arrow (`FXR_SHAFT`). An Ogre's Javelin and a Hill
  Giant's Trash Lob throw a rock.
- The attacker draws for 0.6 s first (`HS_DRAW`), on its attack animation.
- The attack resolves when the shot arrives. If you step out of its path, it
  misses and flies on.
- New sounds play on release, in flight and on impact.
- A hit from off screen shows a fading red wedge toward the attacker.
- Nothing wakes, attacks or lands within 12 m of spawn (`HS_SAFE_R`) or for
  3 s after you spawn (`HS_CALM`).

That is the rule this book follows for action combat: the SRD's roll decides
the hit, and you see and hear what you're rolling against.

## The checks

- `verse/experimental-hostiles.coax`, `#test "a shot flies before it lands:
  you can step out of its way, and one in the air lands where you stand"`.
- `verse/experimental-hostiles.coax`, `#test "a safe spawn: nothing wakes or
  strikes you by the spawn or in your first seconds, and you see it coming"`.
- `everglade/shared/move.coax`, `#test "the run is a jog and the
  walk a stroll"`: Everglade II's pace, apart from v1's.

## What comes next

[Chapter 3](ch003-a-realm-with-no-editor.md) sets up the files, the build and
the checks for a realm that you build as text.
