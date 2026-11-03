# 5E Lab

A Verse world at https://openagents.com/worlds/5e-lab, not listed on the worlds page. It's the test bench for the shared SRD 5e rules engine: five druids and seven monks (the Circle of Moss) fight five warlocks and seven barbarians (the Ember Pact), all level 3, while twenty Skeletons (the Restless Dead) rise against both sides, on a grid of 5-foot squares, seen either in 3D in a painterly glade like Everglade's or as a bare flat grid. Every attack roll, saving throw, damage roll and death save comes from the engine, and the log shows each one.

## Controls

- V, or the 2D VIEW / 3D VIEW button, switches between the 3D glade (the default) and the flat grid.
- M, or the ACTION COMBAT / SRD ROUNDS button, switches the clock (action combat in the 3D glade is the default) and starts a new battle. SRD rounds give each creature a full turn every 6 seconds, spent slots and uses come back only on a rest. Action combat plays like the Neverwinter MMO: each creature gets a turn's worth (action, Bonus Action, its share of movement) every 1.5 seconds, and spent spell slots, Rages and Focus Points come back on a cooldown (9 seconds; 4.5 for a Focus Point). Dice, Armor Class and spell durations are the SRD's either way, and repeat saves, death saves and Rage upkeep stay on 6-second rounds.
- In the glade, drag or use the arrow keys to orbit the camera. Shift-drag, the W, A, S and D keys, or a two-finger drag pans it. The mouse wheel, a pinch, or + and - zooms it. C puts the camera back.
- Space pauses and resumes.
- F switches between normal speed and four times as fast.
- R starts a new battle.
- WORLDS at the top right, or Escape, goes to the [worlds list](https://openagents.com/worlds).

## What you are watching

- Each creature takes a turn every 6 seconds, in initiative order. In a turn it has one action, one Bonus Action, its Speed and one spell slot to spend.
- The druids are the SRD's Druid class. The warlocks are the SRD's Warlock class with the Fiend Patron: Armor of Shadows (Mage Armor without a slot, AC 15), Agonizing Blast and Repelling Blast on Eldritch Blast, and Dark One's Blessing (Temporary Hit Points when a foe drops near them). Pact Magic gives them two level 2 slots for the whole battle.
- The Skeletons are the SRD's Skeleton stat block: Undead, AC 14, 13 Hit Points, a Shortsword and a Shortbow (+5, 1d6 + 3 Piercing), Vulnerability to Bludgeoning (a monk's fists do double), Immunity to Poison, and no Hold Person (it works only on Humanoids). They rise five at a time every 8 seconds, half closing in with swords and half shooting from range, and like any SRD monster they die the instant they drop to 0 Hit Points. The heroes fight each other first and the dead when they come close; the last side standing wins.
- The barbarians are the SRD's Barbarian class on the Path of the Berserker: unarmored (AC 13), a Greataxe with the Cleave mastery, Rage (resistance to Bludgeoning, Piercing and Slashing, +2 damage), Reckless Attack, Danger Sense and Frenzy.
- The monks are the SRD's Monk class, Warriors of the Open Hand: unarmored (AC 15), Speed 40, Unarmed Strikes with the Martial Arts die, three Focus Points for Flurry of Blows (each hit can knock a foe Prone or push it 15 feet), Patient Defense and Step of the Wind, and Deflect Attacks.
- The druids' spells: Produce Flame, Starry Wisp and Poison Spray (cantrips); Cure Wounds, Healing Word, Thunderwave, Entangle and Faerie Fire (level 1, cast with level 1 or level 2 slots).
- The warlocks' spells: Eldritch Blast and Chill Touch (cantrips); Hex (level 1, cast with a level 2 slot); Hold Person (level 2); Scorching Ray (level 2, always prepared for the Fiend Patron). Hex adds 1d6 Necrotic to each of the caster's hits and moves to a new foe when its target drops.
- A ring of light marks whoever just took a turn. Monks wear saffron robes and carry nothing; barbarians are bare-chested in fur kilts with a greataxe. Warlocks wear crimson robes with gold trim and carry an iron rod with a violet glow. A green tangle marks a Restrained creature, a green cast a Poisoned one, a dark band over the eyes a Blinded one, grey an enfeebled one, a red rim Temporary Hit Points, a gold sheen a Paralyzed one, a flickering violet rim a hexed one, a violet ring an outlined one, a small flame a lit Produce Flame. The thin line under the hit-point bar is the spell it's concentrating on.
- A creature at 0 Hit Points is Unconscious and makes a death saving throw at the start of each of its turns: green ticks are successes, red ticks failures.
- The battle ends when one side has nobody standing. A new one starts 6 seconds later.

## How it is made

- `aethyr/rules/srd/` is the engine: `engine.coax` (dice, abilities, hit points, death saves), `spells.coax` (the spell records and their effect steps), and `battle.coax` (the grid, casting, concentration, conditions and the real-time turn clock). `docs/dnd/README.md` is the specification.
- `apps/openagents-web/fivelab.coax` is the world: the druids' and warlocks' decisions, the drawing and its checks. `fivelab-face.c` binds it for the browser, and `fivelab.wgsl` (from `gpu/fivelab.coax`) draws either view: the flat grid, or the glade traced per pixel against its trees and figures and lit with the painterly style's functions (`verse/style.coax`).

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
