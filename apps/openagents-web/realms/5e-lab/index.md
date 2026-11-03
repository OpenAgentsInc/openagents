# 5E Lab

A Verse world at https://openagents.com/worlds/5e-lab, not listed on the worlds page. It's the test bench for the shared SRD 5e rules engine: two circles of level 3 druids, three a side, fight on a grid of 5-foot squares, seen either in 3D in a painterly glade like Everglade's or as a bare flat grid. Every attack roll, saving throw, damage roll and death save comes from the engine, and the log shows each one.

## Controls

- V, or the 2D VIEW / 3D VIEW button, switches between the 3D glade (the default) and the flat grid.
- In the glade, drag or use the arrow keys to turn the camera, and + and - to zoom.
- Space pauses and resumes.
- F switches between normal speed and four times as fast.
- R starts a new battle.
- WORLDS at the top right, or Escape, goes to the [worlds list](https://openagents.com/worlds).

## What you are watching

- Each druid takes a turn every 6 seconds, in initiative order. In a turn it has one action, one Bonus Action, its Speed (30 feet) and one spell slot to spend.
- The spells: Produce Flame, Starry Wisp and Poison Spray (cantrips); Cure Wounds, Healing Word, Thunderwave, Entangle and Faerie Fire (level 1, cast with level 1 or level 2 slots).
- A ring of light marks whoever just took a turn. A green tangle marks a Restrained druid, a violet ring an outlined one, a small flame a lit Produce Flame. The thin line under the hit-point bar is the spell it's concentrating on.
- A druid at 0 Hit Points is Unconscious and makes a death saving throw at the start of each of its turns: green ticks are successes, red ticks failures.
- The battle ends when one side has nobody standing. A new one starts 6 seconds later.

## How it is made

- `aethyr/rules/srd/` is the engine: `engine.coax` (dice, abilities, hit points, death saves), `spells.coax` (the spell records and their effect steps), and `battle.coax` (the grid, casting, concentration, conditions and the real-time turn clock). `docs/dnd/README.md` is the specification.
- `apps/openagents-web/fivelab.coax` is the world: the druids' decisions, the drawing and its checks. `fivelab-face.c` binds it for the browser, and `fivelab.wgsl` (from `gpu/fivelab.coax`) draws either view: the flat grid, or the glade traced per pixel against its trees and druids and lit with the painterly style's functions (`verse/style.coax`).

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
