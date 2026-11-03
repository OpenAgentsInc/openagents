# 5E Lab

A Verse world at https://openagents.com/worlds/5e-lab, not listed on the worlds page. It's the test bench for the shared SRD 5e rules engine: two sides of 24, the Circle of Moss and the Ember Pact, each with two of every one of the twelve SRD 5e classes at level 3, fight each other while an army of 160 Skeletons (the Restless Dead) stands against both sides, on a grid of 5-foot squares, seen either in 3D in a painterly glade like Everglade's or as a bare flat grid. Every attack roll, saving throw, damage roll and death save comes from the engine, and the log shows each one.

## Controls

- V, or the 2D VIEW / 3D VIEW button, switches between the 3D glade (the default) and the flat grid.
- M, or the ACTION COMBAT / SRD ROUNDS button, switches the clock (action combat in the 3D glade is the default) and starts a new battle. SRD rounds give each creature a full turn every 6 seconds, spent slots and uses come back only on a rest. Action combat plays like the Neverwinter MMO: each creature gets a turn's worth (action, Bonus Action, its share of movement) every 1.5 seconds, and spent spell slots, Rages and Focus Points come back on a cooldown (9 seconds; 4.5 for a Focus Point). Dice, Armor Class and spell durations are the SRD's either way, and repeat saves, death saves and Rage upkeep stay on 6-second rounds.
- In the glade, drag or hold the arrow keys to orbit the camera. Shift-drag or a two-finger drag pans it, and holding W, A, S or D flies it smoothly over the grid. The mouse wheel, a pinch, or holding + or - zooms it. C puts the camera back. The frame rate shows in the scene's bottom-right corner.
- Space pauses and resumes.
- F switches between normal speed and four times as fast.
- R starts a new battle.
- WORLDS at the top right, or Escape, goes to the [worlds list](https://openagents.com/worlds).

## What you are watching

- Each creature takes a turn every 6 seconds, in initiative order. In a turn it has one action, one Bonus Action, its Speed and one spell slot to spend.
- Each side has two of each class. The six spellcasting classes (Druid, Wizard, Warlock, Sorcerer, Bard and Cleric) stand in the two back files, and the Fighter, Paladin, Barbarian, Monk, Rogue and Ranger stand in front of them.
- Druids are on the Circle of the Land (a polar land): Produce Flame, Starry Wisp and Poison Spray; Cure Wounds, Healing Word, Thunderwave, Entangle, Faerie Fire and Hold Person. Land's Aid spends a Wild Shape use for 2d6 Necrotic in a 10-foot sphere and heals one creature there.
- Wizards are Evokers in Mage Armor (AC 15): Fire Bolt, Thunderwave, Scorching Ray and Hold Person. Potent Cantrip deals half damage on a miss or a successful save, and Shield adds 5 to AC as a Reaction.
- Warlocks have the Fiend Patron: Armor of Shadows (AC 15), Agonizing Blast and Repelling Blast on Eldritch Blast, and Dark One's Blessing. They cast Chill Touch, Hex, Hold Person and Scorching Ray with two level 2 Pact Magic slots.
- Sorcerers have Draconic Sorcery (3 more Hit Points, AC 10 + Dex + Cha): Fire Bolt, Thunderwave and Scorching Ray. Innate Sorcery adds 1 to their spell save DC and gives Advantage on spell attacks. Sorcery Points pay for Quickened Spell (a second Fire Bolt as a Bonus Action) and Seeking Spell (a missed spell attack rerolled). They also cast Shield.
- Bards are on the College of Lore: Bardic Inspiration (a d6 added to a friend's failed roll), Cutting Words (a Reaction that subtracts the die from a foe's hit), Vicious Mockery, Healing Word, Thunderwave and Hold Person.
- Clerics are on the Life Domain, in a Chain Shirt with a Shield (AC 16) and a Mace. They cast Sacred Flame, Guiding Bolt (the next attack against the target has Advantage) and Healing Word, which Disciple of Life strengthens. Channel Divinity powers Turn Undead (Skeletons that fail the save flee) or Preserve Life.
- Fighters are Champions in Chain Mail with the Defense style (AC 17), with a Greatsword and its Graze mastery. They have Second Wind, Action Surge, Improved Critical (a 19 or 20 is a Critical Hit) and Remarkable Athlete.
- Paladins are on the Oath of Devotion, in Chain Mail with a Shield (AC 18), with a Longsword and its Sap mastery. They have Lay On Hands, Divine Smite (once without a slot), Sacred Flame (Blessed Warrior) and Sacred Weapon.
- Rangers are Hunters with a Longbow (Slow mastery) and Archery, and a Shortsword (Vex mastery) at close range. Favored Enemy casts Hunter's Mark without a slot, and Colossus Slayer adds 1d8 against a wounded foe.
- Rogues are Thieves in Leather Armor with a Shortsword and a Shortbow (Vex mastery). They have Sneak Attack (2d6), Cunning Action (a Dash as a Bonus Action) and Steady Aim.
- Barbarians are on the Path of the Berserker: unarmored (AC 13), a Greataxe with the Cleave mastery, Rage (resistance to Bludgeoning, Piercing and Slashing, +2 damage), Reckless Attack, Danger Sense and Frenzy.
- Monks are Warriors of the Open Hand: unarmored (AC 15), Speed 40, Unarmed Strikes with the Martial Arts die, three Focus Points for Flurry of Blows (each hit can knock a foe Prone or push it 15 feet), Patient Defense, Step of the Wind and Deflect Attacks.
- Skeletons use the SRD's Skeleton stat block: Undead, AC 14, 13 Hit Points, a Shortsword and a Shortbow (+5, 1d6 + 3 Piercing), Vulnerability to Bludgeoning (a monk's fists do double), Immunity to Poison, and no Hold Person (it works only on Humanoids). All 160 stand in the middle of the glade from the start. Half close in with swords and half shoot from range, and like any SRD monster they die the instant they drop to 0 Hit Points. The heroes fight each other first and fight the dead when they come close. The last side standing wins.
- Some class features don't change a battle, such as a Thief's Fast Hands or a Paladin's Divine Sense. The engine has no Opportunity Attacks, so Disengage does nothing here.
- The CHARACTERS panel lists each hero's Hit Points, AC and what it has left: spell slots, Rages, Focus Points, Bardic Inspiration (INSP), Channel Divinity, Second Wind and Action Surge, Lay On Hands, Hunter's Marks without a slot (MARKS), Sneak Attack dice, Sorcery Points and Land's Aid.
- A ring of light marks whoever just took a turn. The ring at each figure's feet and its belt show its side: green for the Circle of Moss, blue for the Ember Pact. Each class has its own look. Druids wear green robes and carry a staff with a green glow. Wizards wear blue robes and hats and carry a staff with a pale glow. Warlocks wear crimson with gold trim and carry an iron rod with a violet glow. Sorcerers wear copper scales and hold a flame. Bards wear plum with a gold sash and carry a lute. Clerics wear white and gold and carry a mace. Fighters wear chain mail and a helm and carry a greatsword. Paladins wear bright plate and carry a blue shield. Rangers wear a green hood and carry a longbow. Rogues are hooded in black with a short blade. Monks wear saffron and carry nothing. Barbarians are bare-chested in fur kilts with a greataxe.
- Marks on a figure show its conditions. A green tangle marks a Restrained creature, a green cast a Poisoned one, a dark band over the eyes a Blinded one, grey an enfeebled one, a red rim Temporary Hit Points, a gold sheen a Paralyzed one, a flickering violet rim a hexed one, a violet ring an outlined one, and a small flame a lit Produce Flame. The thin line under the hit-point bar is the spell it's concentrating on.
- A creature at 0 Hit Points is Unconscious and makes a death saving throw at the start of each of its turns: green ticks are successes, red ticks failures.
- The battle ends when one side has nobody standing. A new one starts 6 seconds later.

## How it is made

- `aethyr/rules/srd/` is the engine: `engine.coax` (dice, abilities, hit points, death saves), `spells.coax` (the spell records and their effect steps), and `battle.coax` (the grid, casting, concentration, conditions and the real-time turn clock). `docs/dnd/README.md` is the specification.
- `apps/openagents-web/fivelab.coax` is the world: each class's decisions, the drawing and its checks. `fivelab-face.c` binds it for the browser, and `fivelab.wgsl` (from `gpu/fivelab.coax`) draws either view: the flat grid, or the glade traced per pixel against its trees and figures and lit with the painterly style's functions (`verse/style.coax`).

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.

## The Horde

The Horde is the default battle. All 48 heroes fight as one company, the Living, in the middle of the glade: the casting classes inside an 8 by 6 block and the rest round its rim. 160 Skeletons start on the grid's two outer rings and run at them, taking the Dash action until they're within reach. When one falls, a fresh Skeleton runs in from a free square on the edge a second later, until the Horde has sent 1,000. The Living win if they outlast all 1,000. Press H to switch between the Horde and the two armies.
