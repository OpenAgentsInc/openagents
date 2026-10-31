# Hold the Stair

A Verse realm at https://openagents.com/realms/hold-the-stair: a popping tower defense on the SRD 5e rules (System Reference Document 5.2.1, CC BY 4.0). Bubble-beasts climb the stair of a wizard's tower from the cellar door to the top; you hold it with a party of adventurers whose attacks roll real dice.

## How to play

- Click or tap a floor tile beside the stair, then pick a class: FIGHTER (50 gold), WIZARD (85), CLERIC (65), ROGUE (60), RANGER (75), BARD (70) or PALADIN (80). Keys 1 to 7 pick them for the chosen tile.
- START WAVE (or Space) sends the next wave. Ten waves hold the stair; lives are the tower's hit points, and each beast that reaches the top takes what it holds inside.
- SPEED steps the game through 1, 2 and 3 times (F on a keyboard).
- Click a defender, or long-press it on a phone, for its card: its attack, its last roll (the d20, its bonus and the armor class or DC it met) and its hits, misses and pops. SELL gives back 70% of its cost.
- Gold comes from every layer popped and from each wave held.

## The bubble-beasts

| Layer | Armor class | Hit points | Pops into |
|---|---|---|---|
| Mote (pale blue) | 10 | 2 | nothing |
| Wisp (green, resists cold) | 11 | 4 | two motes |
| Glimmer (gold, fast) | 13 | 5 | two wisps |
| Ember (red, immune to fire) | 12 | 7 | a glimmer and a wisp |

## The party (level 1)

- Fighter: a longsword, d20 + 5 against the beast's armor class, 1d8 + 3 slashing; a natural 20 doubles the dice.
- Wizard: Fire Bolt, d20 + 5, 1d10 fire. An ember is immune to fire, so against one the wizard casts Magic Missile (three darts that never miss, 1d4 + 1 force each, two slots a wave) or turns its Fire Bolt on another beast.
- Cleric: Sacred Flame, the beast makes a Dexterity save against DC 13 or takes 1d8 radiant.
- Rogue: a shortsword, d20 + 5, 1d6 + 3 piercing, and Sneak Attack: 1d6 more when another of the party stands beside its target. Level 2 gives Cunning Action (its turns come a quarter sooner); level 3, Sneak Attack 2d6.
- Ranger: a longbow, d20 + 5, 1d8 + 3 piercing, at the longest reach. Its favoured layer is the strongest in the coming wave when it is placed: it shoots those first and adds Hunter's Mark (1d6). Level 2 marks whatever it hits; level 3 looses two arrows a turn.
- Bard: Vicious Mockery, a Wisdom save against DC 13 or 1d6 psychic, and Bardic Inspiration: each turn the nearest of the party in its reach adds a d6 to its next attack roll. Level 2 inspires two; level 3 mocks for 2d6.
- Paladin: a longsword, d20 + 5, 1d8 + 3 slashing. Its aura adds 1 to the attack rolls of the party within two tiles. Level 2 gives Divine Smite (2d8 radiant on its first hit of a wave); level 3 widens the aura to three tiles.

The red numbers rise over whatever is hit; MISS, SAVED and IMMUNE say why nothing was.

## How it is made

- `apps/openagents-web/stair.coax` is the game: the stair, the waves, the party's turns and the drawing. The dice and attack rolls are `verse/rules/srd5.coax` and `verse/rules/srd5-combat.coax`, the same rules Everglade plays by. `stair-face.c` binds it for the browser and `stair.wgsl` draws it.
- `stair-tables.coax` holds the content as text tables, a statement a line: the four layers, the three classes and their levels, the ten waves, the starting gold and lives, and the stair's corners. A table that breaks a rule is refused with its line and the reason.
- The dice are seeded: a map's table names its seed (`seed N`), `?seed=N` picks another, and starting over rolls the same dice from the first roll, so a replay or a shared match rolls the same game. A bubble's wobble and the numbers' drift use dice of their own and never shift a roll. Pops and splits go into a rules log for replays and analytics.
- The difficulty curve is checked by seeded players (`play_as` in `stair.coax`): one defender a round reaches wave 8 or later, and the best play holds the stair but loses lives to waves 9 and 10. `coax/bin/coax run apps/openagents-web/stair-sim.coax --release` plays them over many seeds.
- The design is `docs/verse/game-ideas/hold-the-stair.md` in the OpenAgents repository; the page is written from `realms.json` by `coax/web/realms.ts` (`docs/verse/realm-host.md`).
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## Make your own realm

An agent can build one from a shallow clone of the repository and open a pull request: [verse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md).
