# 5E MMO

A Verse world at https://openagents.com/worlds/5e-mmo, not listed on the worlds page. It plays the shared SRD 5e rules engine as an MMO, in Everglade. You play one level 6 hero. Three AI casters fight beside you against waves of six Skeletons, on a grid of 5-foot squares laid over the grass in the city's south-east corner. Every figure is one of Everglade's people, with the same bodies and the same clips: idle, walk, cast, slash and die.

## Controls

- W, A, S and D walk your hero. The camera follows behind you. Drag with the mouse, or hold the arrow keys, to turn the camera. The mouse wheel, or + and -, zooms it.
- On a phone, the left thumb walks you and the right thumb turns the camera.
- The class buttons along the top pick the class you play, and start a new battle. Fighter is the default. You can also open the page with `#class=wizard` (or any class's name) on its address.
- Click a Skeleton, or tap it, to make it your target. Tab, or T, picks the next one, nearest first.
- Keys 1 to 9 queue a slot of your class's hotbar for your next turn.
- Without WebGPU (a current Chrome, Edge or Safari), the page shows only its name and a link here.

## How your walking maps onto squares

The engine decides where everyone stands, a square at a time. Your body walks smoothly between them:

- The controller walks you at the engine's own pace. On the action clock every creature gets a turn every 1.5 seconds, and a turn moves its share of a 6-second round's Speed, rounded to whole squares: Speed 30 is 2 squares (10 feet) a turn, about 2 metres a second. That speed is your walking pace. A creature that can't move, such as a Restrained, Paralyzed or downed one, has a pace of 0.
- At the start of each of your turns, the engine steps you square by square toward the square under your body, up to your movement for that turn. Squares cost what the SRD says they do: 5 feet, or 10 in difficult terrain such as Entangle's plants.
- If the engine can't follow you, for example because a Skeleton stands in the square or the plants hold you, and your body gets more than 2.5 squares from your square in the battle, the body is pulled back to it.
- You can't walk off the grid, or through the other figures.
- Each turn you attack your target (or the nearest foe) unless you queued a slot.

## What you are watching

- Your party is you and three casters: a Wizard, a Cleric and a Warlock. If you play one of those classes, a Druid (and then a Sorcerer) takes its place. Each AI hero decides with 5E Lab's routine for its class.
- Skeletons use the SRD's Skeleton stat block. Half close in with shortswords and half shoot shortbows. A new wave comes 3 seconds after the last Skeleton of a wave falls. If the whole party falls, a new battle starts 6 seconds later.
- The name over each head is gold for you, green for a friend and red for a foe, and the bar under it is the creature's Hit Points. Gold marks either side of a bar show your target.
- Each class has its own look. Fighters wear blue with a sword and shield. Paladins wear white and gold with a sword and shield. Wizards wear long blue robes and carry a staff. Clerics wear white robes and carry a mace and shield. Warlocks are hooded in purple with a rod. Druids are hooded in green with a staff. Sorcerers wear crimson robes. Bards wear teal and carry a rapier. Barbarians wear brown and carry a greataxe. Monks wear saffron and carry nothing. Rogues are hooded in black with a dagger. Rangers wear the green hood and carry a longbow. Skeletons are a pale bone colour.

## How it is made

- `aethyr/rules/srd/` is the engine. `docs/dnd/README.md` is its specification.
- `apps/openagents-web/fivemmo-battle.coax` runs the battle on the engine, with 5E Lab's routines for each class. It is its own C unit, so its names stay apart from Everglade's.
- `apps/openagents-web/fivemmo.coax` is the page. It is Everglade's page (`everglade.coax`): the same ground, city, light, pack, people and shaders (`fivemmo.wgsl`, from `verse/everglade/gpu.coax`). It poses the figures before Everglade's frame, then draws the weapons, names and HUD.
- `apps/openagents-web/fivemmo-face.c` binds them for the browser, with `aethyr/player.coax` walking you.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
