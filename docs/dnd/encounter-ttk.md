# Encounter time to kill

Written by `coax/web/tool.sh coax/web/room/encounter-sim.coax --write docs/dnd/encounter-ttk.md` (#13288): every
class at levels 1 to 6, alone, against the third wolf pack (three Wolves), the path's guard (a level 10 Fighter),
the first pack (one Wolf), the second (two Wolves) and the goblin camp (its two Goblin Warriors, fought at their fire; the two archers, each on its own, stay on their perches),
in the real room's rules, 32 fights (seeds 1 to 32) a row. Wins: fights where every foe fell first. TTK: the mean seconds from
the player's first harmful press to the last foe's fall, over the wins. Damage/s, healed and taken: per fight.
Downs: fights where the player fell. HP and resources (spell slots, limited uses, Focus) left: at the fight's end.
A fight lasts at most 120 s. The after-landing check fails a change that moves a row's TTK, damage a second or
wins by more than 25%; run this again with --write in the same commit when the change is meant.

| Encounter | Class | Level | Wins | TTK s | Damage/s | Healed | Taken | Downs | HP left % | Resources left % |
|---|---|---|---|---|---|---|---|---|---|---|
| wolves | Barbarian | 1 | 17/32 | 4.8 | 5.5 | 0.0 | 10.1 | 15 | 28 | 50 |
| wolves | Barbarian | 2 | 26/32 | 6.3 | 4.7 | 0.0 | 12.2 | 6 | 47 | 50 |
| wolves | Barbarian | 3 | 30/32 | 7.7 | 4.1 | 0.0 | 15.3 | 2 | 52 | 66 |
| wolves | Barbarian | 4 | 32/32 | 5.7 | 5.8 | 0.0 | 9.6 | 0 | 76 | 66 |
| wolves | Barbarian | 5 | 32/32 | 2.0 | 15.5 | 0.0 | 3.1 | 0 | 94 | 66 |
| wolves | Barbarian | 6 | 32/32 | 1.7 | 17.6 | 0.0 | 1.6 | 0 | 97 | 75 |
| wolves | Bard | 1 | 8/32 | 3.7 | 9.5 | 0.9 | 8.0 | 24 | 21 | 74 |
| wolves | Bard | 2 | 16/32 | 3.5 | 7.1 | 5.1 | 14.3 | 16 | 39 | 66 |
| wolves | Bard | 3 | 19/32 | 5.6 | 5.1 | 10.3 | 21.7 | 13 | 45 | 46 |
| wolves | Bard | 4 | 30/32 | 6.1 | 5.2 | 11.0 | 20.1 | 2 | 66 | 51 |
| wolves | Bard | 5 | 32/32 | 6.5 | 5.1 | 6.1 | 15.0 | 0 | 73 | 43 |
| wolves | Bard | 6 | 32/32 | 1.3 | 25.3 | 0.0 | 0.2 | 0 | 100 | 84 |
| wolves | Cleric | 1 | 0/32 | - | 5.3 | 0.0 | 9.0 | 32 | 0 | 0 |
| wolves | Cleric | 2 | 9/32 | 7.4 | 3.5 | 2.5 | 15.0 | 23 | 16 | 41 |
| wolves | Cleric | 3 | 23/32 | 6.3 | 4.0 | 12.0 | 21.1 | 9 | 56 | 34 |
| wolves | Cleric | 4 | 26/32 | 7.4 | 3.9 | 13.8 | 24.5 | 6 | 60 | 39 |
| wolves | Cleric | 5 | 29/32 | 5.3 | 5.4 | 15.3 | 27.0 | 3 | 64 | 49 |
| wolves | Cleric | 6 | 31/32 | 3.3 | 9.3 | 5.5 | 16.8 | 1 | 71 | 68 |
| wolves | Druid | 1 | 6/32 | 4.1 | 8.0 | 0.0 | 7.8 | 26 | 14 | 0 |
| wolves | Druid | 2 | 13/32 | 4.5 | 7.6 | 0.5 | 11.4 | 19 | 27 | 43 |
| wolves | Druid | 3 | 24/32 | 5.7 | 5.5 | 11.9 | 19.7 | 8 | 63 | 39 |
| wolves | Druid | 4 | 29/32 | 5.6 | 5.8 | 12.1 | 22.0 | 3 | 63 | 45 |
| wolves | Druid | 5 | 31/32 | 3.8 | 8.5 | 2.5 | 11.2 | 1 | 73 | 64 |
| wolves | Druid | 6 | 32/32 | 4.3 | 7.7 | 3.3 | 13.8 | 0 | 73 | 70 |
| wolves | Fighter | 1 | 4/32 | 5.4 | 5.9 | 0.0 | 11.2 | 28 | 7 | 100 |
| wolves | Fighter | 2 | 11/32 | 4.9 | 5.7 | 0.0 | 17.1 | 21 | 14 | 66 |
| wolves | Fighter | 3 | 10/32 | 5.0 | 4.8 | 0.0 | 23.8 | 22 | 15 | 66 |
| wolves | Fighter | 4 | 26/32 | 5.8 | 5.5 | 0.0 | 21.0 | 6 | 41 | 75 |
| wolves | Fighter | 5 | 32/32 | 2.2 | 15.2 | 0.0 | 7.5 | 0 | 83 | 75 |
| wolves | Fighter | 6 | 32/32 | 2.2 | 15.2 | 0.0 | 6.1 | 0 | 88 | 75 |
| wolves | Monk | 1 | 0/32 | - | 3.6 | 0.0 | 9.0 | 32 | 0 | 100 |
| wolves | Monk | 2 | 0/32 | - | 4.1 | 0.0 | 15.0 | 32 | 0 | 40 |
| wolves | Monk | 3 | 13/32 | 12.8 | 2.5 | 0.0 | 16.5 | 19 | 21 | 25 |
| wolves | Monk | 4 | 31/32 | 10.7 | 3.1 | 0.0 | 12.2 | 1 | 54 | 21 |
| wolves | Monk | 5 | 32/32 | 3.4 | 9.7 | 0.0 | 3.0 | 0 | 91 | 41 |
| wolves | Monk | 6 | 32/32 | 3.1 | 10.5 | 0.0 | 2.4 | 0 | 94 | 62 |
| wolves | Paladin | 1 | 12/32 | 6.8 | 4.6 | 2.7 | 10.6 | 20 | 27 | 44 |
| wolves | Paladin | 2 | 21/32 | 8.1 | 3.5 | 8.6 | 18.7 | 11 | 44 | 21 |
| wolves | Paladin | 3 | 31/32 | 5.7 | 5.6 | 3.4 | 11.7 | 1 | 67 | 65 |
| wolves | Paladin | 4 | 31/32 | 5.2 | 6.1 | 4.4 | 15.2 | 1 | 66 | 69 |
| wolves | Paladin | 5 | 32/32 | 3.7 | 8.9 | 1.3 | 10.2 | 0 | 77 | 89 |
| wolves | Paladin | 6 | 32/32 | 3.5 | 9.4 | 0.0 | 8.8 | 0 | 80 | 94 |
| wolves | Ranger | 1 | 0/32 | - | 5.4 | 0.0 | 11.0 | 32 | 0 | 69 |
| wolves | Ranger | 2 | 5/32 | 5.9 | 4.8 | 0.0 | 16.0 | 27 | 11 | 58 |
| wolves | Ranger | 3 | 8/32 | 6.7 | 4.6 | 0.0 | 23.0 | 24 | 8 | 54 |
| wolves | Ranger | 4 | 22/32 | 6.2 | 4.8 | 0.0 | 21.4 | 10 | 33 | 60 |
| wolves | Ranger | 5 | 32/32 | 3.4 | 9.8 | 0.0 | 6.9 | 0 | 82 | 84 |
| wolves | Ranger | 6 | 32/32 | 3.2 | 10.3 | 0.0 | 5.0 | 0 | 89 | 85 |
| wolves | Rogue | 1 | 0/32 | - | 4.5 | 0.0 | 10.0 | 32 | 0 | 100 |
| wolves | Rogue | 2 | 0/32 | - | 2.6 | 0.0 | 17.0 | 32 | 0 | 100 |
| wolves | Rogue | 3 | 3/32 | 9.1 | 3.8 | 0.0 | 23.5 | 29 | 2 | 100 |
| wolves | Rogue | 4 | 22/32 | 6.9 | 4.4 | 0.0 | 22.8 | 10 | 26 | 100 |
| wolves | Rogue | 5 | 29/32 | 7.5 | 4.2 | 0.0 | 17.8 | 3 | 53 | 100 |
| wolves | Rogue | 6 | 31/32 | 7.9 | 4.1 | 0.0 | 18.4 | 1 | 59 | 100 |
| wolves | Sorcerer | 1 | 2/32 | 4.8 | 7.9 | 0.0 | 6.8 | 30 | 2 | 25 |
| wolves | Sorcerer | 2 | 6/32 | 5.3 | 6.2 | 0.0 | 10.8 | 26 | 10 | 37 |
| wolves | Sorcerer | 3 | 31/32 | 4.1 | 7.7 | 0.0 | 2.7 | 1 | 86 | 38 |
| wolves | Sorcerer | 4 | 32/32 | 2.7 | 12.0 | 0.0 | 2.5 | 0 | 90 | 57 |
| wolves | Sorcerer | 5 | 32/32 | 0.1 | 32.9 | 0.0 | 0.0 | 0 | 100 | 87 |
| wolves | Sorcerer | 6 | 32/32 | 0.0 | 33.0 | 0.0 | 0.0 | 0 | 100 | 88 |
| wolves | Warlock | 1 | 0/32 | - | 5.0 | 0.0 | 9.0 | 32 | 0 | 0 |
| wolves | Warlock | 2 | 7/32 | 5.4 | 4.8 | 0.0 | 13.9 | 25 | 7 | 46 |
| wolves | Warlock | 3 | 19/32 | 6.7 | 4.3 | 0.0 | 11.4 | 13 | 45 | 33 |
| wolves | Warlock | 4 | 30/32 | 6.9 | 4.8 | 0.0 | 6.6 | 2 | 75 | 33 |
| wolves | Warlock | 5 | 32/32 | 0.1 | 32.8 | 0.0 | 0.0 | 0 | 100 | 63 |
| wolves | Warlock | 6 | 32/32 | 0.1 | 32.9 | 0.0 | 0.0 | 0 | 100 | 85 |
| wolves | Wizard | 1 | 0/32 | - | 2.5 | 0.0 | 7.0 | 32 | 0 | 0 |
| wolves | Wizard | 2 | 1/32 | 7.5 | 2.7 | 0.0 | 11.9 | 31 | 1 | 0 |
| wolves | Wizard | 3 | 12/32 | 8.2 | 3.6 | 0.0 | 13.5 | 20 | 20 | 4 |
| wolves | Wizard | 4 | 25/32 | 7.1 | 4.4 | 0.0 | 9.3 | 7 | 57 | 5 |
| wolves | Wizard | 5 | 32/32 | 0.2 | 31.3 | 0.0 | 0.0 | 0 | 100 | 64 |
| wolves | Wizard | 6 | 32/32 | 0.3 | 29.8 | 0.0 | 0.0 | 0 | 100 | 68 |
| guard | Barbarian | 1 | 0/32 | - | 2.8 | 0.0 | 14.0 | 32 | 0 | 50 |
| guard | Barbarian | 2 | 0/32 | - | 3.6 | 0.0 | 23.0 | 32 | 0 | 50 |
| guard | Barbarian | 3 | 0/32 | - | 2.4 | 0.0 | 32.0 | 32 | 0 | 66 |
| guard | Barbarian | 4 | 0/32 | - | 2.4 | 0.0 | 41.0 | 32 | 0 | 66 |
| guard | Barbarian | 5 | 0/32 | - | 7.4 | 0.0 | 50.0 | 32 | 0 | 66 |
| guard | Barbarian | 6 | 0/32 | - | 5.5 | 0.0 | 59.0 | 32 | 0 | 75 |
| guard | Bard | 1 | 0/32 | - | 4.8 | 0.0 | 9.0 | 32 | 0 | 80 |
| guard | Bard | 2 | 0/32 | - | 5.9 | 0.2 | 15.2 | 32 | 0 | 82 |
| guard | Bard | 3 | 0/32 | - | 4.0 | 5.9 | 26.9 | 32 | 0 | 64 |
| guard | Bard | 4 | 0/32 | - | 3.2 | 16.9 | 43.9 | 32 | 0 | 56 |
| guard | Bard | 5 | 0/32 | - | 3.3 | 32.7 | 65.7 | 32 | 0 | 32 |
| guard | Bard | 6 | 0/32 | - | 4.8 | 41.5 | 80.5 | 32 | 0 | 24 |
| guard | Cleric | 1 | 0/32 | - | 0.0 | 0.0 | 9.0 | 32 | 0 | 47 |
| guard | Cleric | 2 | 0/32 | - | 1.8 | 2.6 | 17.6 | 32 | 0 | 72 |
| guard | Cleric | 3 | 0/32 | - | 1.6 | 9.6 | 30.6 | 32 | 0 | 73 |
| guard | Cleric | 4 | 0/32 | - | 2.4 | 15.5 | 42.5 | 32 | 0 | 67 |
| guard | Cleric | 5 | 0/32 | - | 5.7 | 33.8 | 66.8 | 32 | 0 | 48 |
| guard | Cleric | 6 | 1/32 | 7.6 | 4.4 | 77.3 | 115.3 | 31 | 3 | 26 |
| guard | Druid | 1 | 0/32 | - | 5.8 | 0.0 | 9.0 | 32 | 0 | 50 |
| guard | Druid | 2 | 0/32 | - | 5.1 | 0.3 | 15.3 | 32 | 0 | 78 |
| guard | Druid | 3 | 0/32 | - | 3.7 | 3.9 | 24.9 | 32 | 0 | 80 |
| guard | Druid | 4 | 0/32 | - | 3.2 | 9.0 | 36.0 | 32 | 0 | 76 |
| guard | Druid | 5 | 0/32 | - | 5.1 | 17.4 | 50.4 | 32 | 0 | 71 |
| guard | Druid | 6 | 0/32 | - | 4.8 | 33.8 | 72.8 | 32 | 0 | 63 |
| guard | Fighter | 1 | 0/32 | - | 4.8 | 0.0 | 12.0 | 32 | 0 | 100 |
| guard | Fighter | 2 | 0/32 | - | 6.1 | 0.0 | 20.0 | 32 | 0 | 93 |
| guard | Fighter | 3 | 0/32 | - | 5.2 | 0.0 | 28.0 | 32 | 0 | 87 |
| guard | Fighter | 4 | 0/32 | - | 8.2 | 0.0 | 36.0 | 32 | 0 | 84 |
| guard | Fighter | 5 | 3/32 | 6.6 | 16.3 | 0.0 | 42.7 | 29 | 3 | 80 |
| guard | Fighter | 6 | 11/32 | 4.2 | 19.7 | 0.0 | 47.0 | 21 | 10 | 78 |
| guard | Monk | 1 | 0/32 | - | 2.3 | 0.0 | 9.0 | 32 | 0 | 100 |
| guard | Monk | 2 | 0/32 | - | 1.6 | 0.0 | 15.0 | 32 | 0 | 51 |
| guard | Monk | 3 | 0/32 | - | 1.6 | 0.0 | 21.0 | 32 | 0 | 28 |
| guard | Monk | 4 | 0/32 | - | 2.2 | 0.0 | 27.0 | 32 | 0 | 21 |
| guard | Monk | 5 | 3/32 | 26.6 | 5.2 | 0.0 | 32.0 | 29 | 3 | 16 |
| guard | Monk | 6 | 6/32 | 23.3 | 5.6 | 11.1 | 47.3 | 26 | 7 | 12 |
| guard | Paladin | 1 | 0/32 | - | 4.3 | 0.6 | 11.6 | 32 | 0 | 86 |
| guard | Paladin | 2 | 0/32 | - | 3.8 | 3.8 | 21.8 | 32 | 0 | 68 |
| guard | Paladin | 3 | 0/32 | - | 3.7 | 9.5 | 34.5 | 32 | 0 | 40 |
| guard | Paladin | 4 | 0/32 | - | 4.0 | 14.4 | 46.4 | 32 | 0 | 32 |
| guard | Paladin | 5 | 2/32 | 12.6 | 8.8 | 22.7 | 60.3 | 30 | 3 | 23 |
| guard | Paladin | 6 | 9/32 | 13.1 | 9.1 | 29.4 | 71.6 | 23 | 8 | 13 |
| guard | Ranger | 1 | 0/32 | - | 1.1 | 0.0 | 11.0 | 32 | 0 | 74 |
| guard | Ranger | 2 | 0/32 | - | 1.3 | 0.0 | 18.0 | 32 | 0 | 67 |
| guard | Ranger | 3 | 0/32 | - | 3.6 | 0.0 | 25.0 | 32 | 0 | 74 |
| guard | Ranger | 4 | 0/32 | - | 3.0 | 0.0 | 32.0 | 32 | 0 | 66 |
| guard | Ranger | 5 | 0/32 | - | 8.8 | 0.0 | 39.0 | 32 | 0 | 78 |
| guard | Ranger | 6 | 0/32 | - | 9.0 | 0.0 | 46.0 | 32 | 0 | 75 |
| guard | Rogue | 1 | 0/32 | - | 2.7 | 0.0 | 10.0 | 32 | 0 | 100 |
| guard | Rogue | 2 | 0/32 | - | 3.1 | 0.0 | 17.0 | 32 | 0 | 100 |
| guard | Rogue | 3 | 0/32 | - | 1.9 | 0.0 | 24.0 | 32 | 0 | 100 |
| guard | Rogue | 4 | 0/32 | - | 2.8 | 0.0 | 31.0 | 32 | 0 | 100 |
| guard | Rogue | 5 | 0/32 | - | 3.1 | 0.0 | 38.0 | 32 | 0 | 100 |
| guard | Rogue | 6 | 0/32 | - | 1.8 | 0.0 | 45.0 | 32 | 0 | 100 |
| guard | Sorcerer | 1 | 0/32 | - | 2.9 | 0.0 | 7.0 | 32 | 0 | 50 |
| guard | Sorcerer | 2 | 0/32 | - | 8.4 | 0.0 | 12.0 | 32 | 0 | 49 |
| guard | Sorcerer | 3 | 0/32 | - | 7.2 | 0.0 | 20.0 | 32 | 0 | 37 |
| guard | Sorcerer | 4 | 0/32 | - | 10.4 | 0.0 | 26.0 | 32 | 0 | 28 |
| guard | Sorcerer | 5 | 1/32 | 4.0 | 17.8 | 0.0 | 31.7 | 31 | 1 | 49 |
| guard | Sorcerer | 6 | 12/32 | 4.3 | 22.6 | 0.0 | 31.8 | 20 | 16 | 51 |
| guard | Warlock | 1 | 0/32 | - | 1.9 | 0.0 | 9.0 | 32 | 0 | 0 |
| guard | Warlock | 2 | 0/32 | - | 0.4 | 0.0 | 15.0 | 32 | 0 | 64 |
| guard | Warlock | 3 | 5/32 | 13.7 | 4.9 | 0.0 | 17.7 | 27 | 16 | 43 |
| guard | Warlock | 4 | 7/32 | 11.8 | 5.2 | 0.0 | 21.9 | 25 | 19 | 42 |
| guard | Warlock | 5 | 0/32 | - | 20.4 | 0.0 | 33.0 | 32 | 0 | 44 |
| guard | Warlock | 6 | 0/32 | - | 19.3 | 0.0 | 39.0 | 32 | 0 | 73 |
| guard | Wizard | 1 | 0/32 | - | 0.8 | 0.0 | 7.0 | 32 | 0 | 0 |
| guard | Wizard | 2 | 0/32 | - | 1.0 | 0.0 | 12.0 | 32 | 0 | 0 |
| guard | Wizard | 3 | 0/32 | - | 2.4 | 0.0 | 17.0 | 32 | 0 | 15 |
| guard | Wizard | 4 | 0/32 | - | 3.0 | 0.0 | 22.0 | 32 | 0 | 14 |
| guard | Wizard | 5 | 0/32 | - | 10.7 | 0.0 | 27.0 | 32 | 0 | 19 |
| guard | Wizard | 6 | 0/32 | - | 12.5 | 0.0 | 32.0 | 32 | 0 | 21 |
| one wolf | Barbarian | 1 | 32/32 | 0.7 | 8.0 | 0.0 | 0.4 | 0 | 97 | 50 |
| one wolf | Barbarian | 2 | 32/32 | 1.1 | 6.8 | 0.0 | 0.8 | 0 | 96 | 50 |
| one wolf | Barbarian | 3 | 32/32 | 0.8 | 8.1 | 0.0 | 0.5 | 0 | 98 | 66 |
| one wolf | Barbarian | 4 | 32/32 | 0.8 | 8.1 | 0.0 | 0.4 | 0 | 99 | 66 |
| one wolf | Barbarian | 5 | 32/32 | 0.2 | 11.0 | 0.0 | 0.0 | 0 | 100 | 66 |
| one wolf | Barbarian | 6 | 32/32 | 0.2 | 11.0 | 0.0 | 0.0 | 0 | 100 | 75 |
| one wolf | Bard | 1 | 27/32 | 2.7 | 3.4 | 1.8 | 5.0 | 5 | 63 | 66 |
| one wolf | Bard | 2 | 32/32 | 3.1 | 3.5 | 1.8 | 4.3 | 0 | 83 | 66 |
| one wolf | Bard | 3 | 32/32 | 2.3 | 4.7 | 0.3 | 2.1 | 0 | 91 | 77 |
| one wolf | Bard | 4 | 32/32 | 2.6 | 4.3 | 0.0 | 1.0 | 0 | 96 | 80 |
| one wolf | Bard | 5 | 32/32 | 2.6 | 4.2 | 0.0 | 1.2 | 0 | 96 | 73 |
| one wolf | Bard | 6 | 32/32 | 1.2 | 9.1 | 0.0 | 0.3 | 0 | 99 | 84 |
| one wolf | Cleric | 1 | 30/32 | 0.7 | 7.1 | 0.0 | 0.7 | 2 | 93 | 33 |
| one wolf | Cleric | 2 | 32/32 | 0.9 | 7.6 | 0.0 | 0.9 | 0 | 94 | 68 |
| one wolf | Cleric | 3 | 32/32 | 0.7 | 7.9 | 0.0 | 0.9 | 0 | 96 | 80 |
| one wolf | Cleric | 4 | 32/32 | 0.6 | 9.1 | 0.0 | 0.3 | 0 | 99 | 83 |
| one wolf | Cleric | 5 | 32/32 | 0.6 | 9.2 | 0.0 | 1.2 | 0 | 96 | 78 |
| one wolf | Cleric | 6 | 32/32 | 0.8 | 8.7 | 0.0 | 1.8 | 0 | 95 | 81 |
| one wolf | Druid | 1 | 27/32 | 3.6 | 2.9 | 0.0 | 3.0 | 5 | 67 | 0 |
| one wolf | Druid | 2 | 32/32 | 3.3 | 3.3 | 0.3 | 2.7 | 0 | 84 | 37 |
| one wolf | Druid | 3 | 32/32 | 2.9 | 3.8 | 0.2 | 2.1 | 0 | 91 | 60 |
| one wolf | Druid | 4 | 32/32 | 2.3 | 4.9 | 0.3 | 2.4 | 0 | 92 | 68 |
| one wolf | Druid | 5 | 32/32 | 0.7 | 9.0 | 0.0 | 0.2 | 0 | 100 | 85 |
| one wolf | Druid | 6 | 32/32 | 0.4 | 10.7 | 0.0 | 0.0 | 0 | 100 | 89 |
| one wolf | Fighter | 1 | 31/32 | 1.1 | 7.6 | 0.0 | 2.2 | 1 | 82 | 100 |
| one wolf | Fighter | 2 | 32/32 | 1.3 | 6.9 | 0.0 | 2.9 | 0 | 86 | 66 |
| one wolf | Fighter | 3 | 32/32 | 1.2 | 7.9 | 0.0 | 2.2 | 0 | 92 | 66 |
| one wolf | Fighter | 4 | 32/32 | 0.7 | 9.7 | 0.0 | 1.3 | 0 | 96 | 75 |
| one wolf | Fighter | 5 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 75 |
| one wolf | Fighter | 6 | 32/32 | 0.0 | 11.0 | 0.0 | 0.0 | 0 | 100 | 75 |
| one wolf | Monk | 1 | 30/32 | 2.3 | 4.5 | 0.0 | 3.3 | 2 | 63 | 100 |
| one wolf | Monk | 2 | 30/32 | 2.6 | 3.8 | 0.0 | 2.8 | 2 | 81 | 51 |
| one wolf | Monk | 3 | 32/32 | 2.2 | 4.9 | 0.0 | 0.0 | 0 | 100 | 54 |
| one wolf | Monk | 4 | 32/32 | 2.4 | 4.5 | 0.0 | 0.0 | 0 | 100 | 59 |
| one wolf | Monk | 5 | 32/32 | 0.3 | 10.8 | 0.0 | 0.0 | 0 | 100 | 95 |
| one wolf | Monk | 6 | 32/32 | 0.4 | 10.2 | 0.0 | 0.0 | 0 | 100 | 96 |
| one wolf | Paladin | 1 | 30/32 | 0.4 | 8.8 | 0.0 | 0.9 | 2 | 92 | 89 |
| one wolf | Paladin | 2 | 32/32 | 2.2 | 5.0 | 0.3 | 1.6 | 0 | 93 | 94 |
| one wolf | Paladin | 3 | 32/32 | 1.0 | 8.3 | 0.0 | 0.5 | 0 | 98 | 85 |
| one wolf | Paladin | 4 | 32/32 | 1.1 | 7.8 | 0.0 | 0.0 | 0 | 100 | 89 |
| one wolf | Paladin | 5 | 32/32 | 0.6 | 11.0 | 0.0 | 0.0 | 0 | 100 | 94 |
| one wolf | Paladin | 6 | 32/32 | 0.6 | 10.7 | 0.0 | 0.0 | 0 | 100 | 95 |
| one wolf | Ranger | 1 | 29/32 | 1.2 | 5.5 | 0.0 | 2.5 | 3 | 77 | 72 |
| one wolf | Ranger | 2 | 32/32 | 1.4 | 6.1 | 0.0 | 1.4 | 0 | 92 | 73 |
| one wolf | Ranger | 3 | 32/32 | 1.3 | 6.5 | 0.0 | 1.2 | 0 | 95 | 78 |
| one wolf | Ranger | 4 | 32/32 | 0.7 | 8.9 | 0.0 | 0.4 | 0 | 99 | 80 |
| one wolf | Ranger | 5 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 88 |
| one wolf | Ranger | 6 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 88 |
| one wolf | Rogue | 1 | 30/32 | 2.2 | 4.8 | 0.0 | 3.7 | 2 | 63 | 100 |
| one wolf | Rogue | 2 | 32/32 | 2.0 | 5.4 | 0.0 | 3.4 | 0 | 80 | 100 |
| one wolf | Rogue | 3 | 31/32 | 1.7 | 5.4 | 0.0 | 3.5 | 1 | 85 | 100 |
| one wolf | Rogue | 4 | 32/32 | 1.5 | 7.3 | 0.0 | 3.4 | 0 | 89 | 100 |
| one wolf | Rogue | 5 | 32/32 | 1.4 | 7.7 | 0.0 | 1.1 | 0 | 97 | 100 |
| one wolf | Rogue | 6 | 32/32 | 1.6 | 6.7 | 0.0 | 1.7 | 0 | 96 | 100 |
| one wolf | Sorcerer | 1 | 29/32 | 1.1 | 6.4 | 0.0 | 1.2 | 3 | 83 | 33 |
| one wolf | Sorcerer | 2 | 32/32 | 0.9 | 8.7 | 0.0 | 0.2 | 0 | 98 | 56 |
| one wolf | Sorcerer | 3 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 79 |
| one wolf | Sorcerer | 4 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 83 |
| one wolf | Sorcerer | 5 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 87 |
| one wolf | Sorcerer | 6 | 32/32 | 0.0 | 11.0 | 0.0 | 0.0 | 0 | 100 | 88 |
| one wolf | Warlock | 1 | 29/32 | 1.8 | 4.6 | 0.0 | 1.7 | 3 | 81 | 0 |
| one wolf | Warlock | 2 | 32/32 | 1.6 | 5.6 | 0.0 | 1.6 | 0 | 89 | 65 |
| one wolf | Warlock | 3 | 32/32 | 0.4 | 9.5 | 0.0 | 0.5 | 0 | 98 | 59 |
| one wolf | Warlock | 4 | 32/32 | 0.1 | 10.9 | 0.0 | 0.0 | 0 | 100 | 63 |
| one wolf | Warlock | 5 | 32/32 | 0.0 | 11.0 | 0.0 | 0.0 | 0 | 100 | 66 |
| one wolf | Warlock | 6 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 84 |
| one wolf | Wizard | 1 | 30/32 | 4.9 | 2.1 | 0.0 | 1.6 | 2 | 77 | 0 |
| one wolf | Wizard | 2 | 32/32 | 4.8 | 2.2 | 0.0 | 1.3 | 0 | 90 | 0 |
| one wolf | Wizard | 3 | 32/32 | 1.4 | 6.2 | 0.0 | 0.2 | 0 | 99 | 34 |
| one wolf | Wizard | 4 | 32/32 | 0.8 | 8.1 | 0.0 | 0.0 | 0 | 100 | 47 |
| one wolf | Wizard | 5 | 32/32 | 0.0 | 11.0 | 0.0 | 0.0 | 0 | 100 | 66 |
| one wolf | Wizard | 6 | 32/32 | 0.0 | 11.0 | 0.0 | 0.0 | 0 | 100 | 70 |
| two wolves | Barbarian | 1 | 28/32 | 3.5 | 5.4 | 0.0 | 4.7 | 4 | 66 | 50 |
| two wolves | Barbarian | 2 | 32/32 | 3.4 | 6.3 | 0.0 | 4.3 | 0 | 81 | 50 |
| two wolves | Barbarian | 3 | 32/32 | 3.0 | 7.0 | 0.0 | 3.3 | 0 | 89 | 66 |
| two wolves | Barbarian | 4 | 32/32 | 3.7 | 5.8 | 0.0 | 4.3 | 0 | 89 | 66 |
| two wolves | Barbarian | 5 | 32/32 | 0.7 | 20.7 | 0.0 | 0.0 | 0 | 100 | 66 |
| two wolves | Barbarian | 6 | 32/32 | 0.8 | 18.3 | 0.0 | 0.4 | 0 | 99 | 75 |
| two wolves | Bard | 1 | 4/32 | 9.3 | 2.6 | 2.2 | 10.8 | 28 | 5 | 69 |
| two wolves | Bard | 2 | 13/32 | 7.4 | 2.8 | 6.3 | 18.3 | 19 | 19 | 59 |
| two wolves | Bard | 3 | 31/32 | 5.3 | 4.1 | 4.6 | 9.6 | 1 | 76 | 43 |
| two wolves | Bard | 4 | 32/32 | 5.7 | 3.9 | 7.4 | 14.7 | 0 | 73 | 51 |
| two wolves | Bard | 5 | 32/32 | 6.8 | 3.2 | 5.0 | 12.9 | 0 | 75 | 45 |
| two wolves | Bard | 6 | 32/32 | 1.3 | 17.5 | 0.0 | 0.9 | 0 | 98 | 84 |
| two wolves | Cleric | 1 | 16/32 | 4.1 | 3.8 | 0.0 | 5.3 | 16 | 41 | 0 |
| two wolves | Cleric | 2 | 22/32 | 3.9 | 4.2 | 0.8 | 7.1 | 10 | 58 | 46 |
| two wolves | Cleric | 3 | 32/32 | 4.1 | 5.4 | 2.1 | 4.3 | 0 | 89 | 54 |
| two wolves | Cleric | 4 | 31/32 | 3.0 | 6.9 | 0.8 | 4.8 | 1 | 85 | 64 |
| two wolves | Cleric | 5 | 32/32 | 2.3 | 9.1 | 2.4 | 7.6 | 0 | 84 | 69 |
| two wolves | Cleric | 6 | 32/32 | 2.5 | 8.6 | 1.3 | 7.3 | 0 | 84 | 72 |
| two wolves | Druid | 1 | 6/32 | 7.3 | 2.8 | 0.0 | 8.6 | 26 | 4 | 0 |
| two wolves | Druid | 2 | 19/32 | 6.1 | 3.0 | 3.1 | 12.5 | 13 | 37 | 25 |
| two wolves | Druid | 3 | 30/32 | 6.4 | 3.4 | 9.0 | 14.9 | 2 | 71 | 32 |
| two wolves | Druid | 4 | 32/32 | 5.4 | 4.1 | 2.9 | 9.3 | 0 | 76 | 45 |
| two wolves | Druid | 5 | 32/32 | 3.6 | 6.0 | 2.6 | 9.7 | 0 | 78 | 66 |
| two wolves | Druid | 6 | 32/32 | 3.3 | 6.7 | 0.3 | 7.7 | 0 | 81 | 75 |
| two wolves | Fighter | 1 | 20/32 | 3.3 | 6.4 | 0.0 | 7.6 | 12 | 37 | 100 |
| two wolves | Fighter | 2 | 31/32 | 3.3 | 6.5 | 0.0 | 6.8 | 1 | 66 | 66 |
| two wolves | Fighter | 3 | 31/32 | 4.2 | 5.1 | 0.0 | 9.7 | 1 | 65 | 66 |
| two wolves | Fighter | 4 | 32/32 | 3.2 | 6.8 | 0.0 | 6.3 | 0 | 82 | 75 |
| two wolves | Fighter | 5 | 32/32 | 1.1 | 20.6 | 0.0 | 3.1 | 0 | 93 | 75 |
| two wolves | Fighter | 6 | 32/32 | 1.1 | 19.7 | 0.0 | 2.7 | 0 | 95 | 75 |
| two wolves | Monk | 1 | 0/32 | - | 3.6 | 0.0 | 9.0 | 32 | 0 | 100 |
| two wolves | Monk | 2 | 7/32 | 5.5 | 3.1 | 0.0 | 13.0 | 25 | 14 | 37 |
| two wolves | Monk | 3 | 32/32 | 8.0 | 2.7 | 0.0 | 3.6 | 0 | 83 | 25 |
| two wolves | Monk | 4 | 32/32 | 6.1 | 3.6 | 0.0 | 2.6 | 0 | 90 | 23 |
| two wolves | Monk | 5 | 32/32 | 1.8 | 12.3 | 0.0 | 0.3 | 0 | 99 | 67 |
| two wolves | Monk | 6 | 32/32 | 1.6 | 13.4 | 0.0 | 0.8 | 0 | 98 | 80 |
| two wolves | Paladin | 1 | 25/32 | 2.3 | 7.6 | 1.9 | 5.5 | 7 | 67 | 53 |
| two wolves | Paladin | 2 | 31/32 | 3.9 | 5.3 | 2.2 | 6.1 | 1 | 78 | 72 |
| two wolves | Paladin | 3 | 32/32 | 3.7 | 6.0 | 1.4 | 6.0 | 0 | 82 | 75 |
| two wolves | Paladin | 4 | 32/32 | 2.8 | 8.0 | 0.6 | 5.8 | 0 | 84 | 84 |
| two wolves | Paladin | 5 | 32/32 | 2.0 | 11.0 | 0.0 | 2.6 | 0 | 93 | 94 |
| two wolves | Paladin | 6 | 32/32 | 1.6 | 12.6 | 0.0 | 2.2 | 0 | 95 | 94 |
| two wolves | Ranger | 1 | 14/32 | 3.7 | 4.3 | 0.0 | 7.6 | 18 | 31 | 65 |
| two wolves | Ranger | 2 | 28/32 | 3.6 | 5.8 | 0.0 | 6.5 | 4 | 64 | 65 |
| two wolves | Ranger | 3 | 29/32 | 4.7 | 4.4 | 0.0 | 10.1 | 3 | 60 | 66 |
| two wolves | Ranger | 4 | 31/32 | 3.7 | 5.6 | 0.0 | 7.8 | 1 | 75 | 68 |
| two wolves | Ranger | 5 | 32/32 | 1.7 | 13.0 | 0.0 | 1.0 | 0 | 97 | 87 |
| two wolves | Ranger | 6 | 32/32 | 1.4 | 15.5 | 0.0 | 0.8 | 0 | 98 | 88 |
| two wolves | Rogue | 1 | 3/32 | 5.1 | 4.6 | 0.0 | 9.5 | 29 | 5 | 100 |
| two wolves | Rogue | 2 | 9/32 | 5.4 | 4.0 | 0.0 | 15.2 | 23 | 10 | 100 |
| two wolves | Rogue | 3 | 28/32 | 4.3 | 4.6 | 0.0 | 9.0 | 4 | 62 | 100 |
| two wolves | Rogue | 4 | 30/32 | 4.0 | 5.2 | 0.0 | 7.4 | 2 | 76 | 100 |
| two wolves | Rogue | 5 | 32/32 | 3.7 | 6.0 | 0.0 | 2.9 | 0 | 92 | 100 |
| two wolves | Rogue | 6 | 32/32 | 4.3 | 5.1 | 0.0 | 3.5 | 0 | 92 | 100 |
| two wolves | Sorcerer | 1 | 9/32 | 3.4 | 5.5 | 0.0 | 5.4 | 23 | 23 | 25 |
| two wolves | Sorcerer | 2 | 26/32 | 2.7 | 7.6 | 0.0 | 4.7 | 6 | 61 | 35 |
| two wolves | Sorcerer | 3 | 32/32 | 1.2 | 18.3 | 0.0 | 0.0 | 0 | 100 | 61 |
| two wolves | Sorcerer | 4 | 32/32 | 1.2 | 19.0 | 0.0 | 0.2 | 0 | 99 | 69 |
| two wolves | Sorcerer | 5 | 32/32 | 0.1 | 21.9 | 0.0 | 0.0 | 0 | 100 | 86 |
| two wolves | Sorcerer | 6 | 32/32 | 0.1 | 22.0 | 0.0 | 0.0 | 0 | 100 | 88 |
| two wolves | Warlock | 1 | 1/32 | 11.5 | 3.2 | 0.0 | 8.9 | 31 | 1 | 0 |
| two wolves | Warlock | 2 | 14/32 | 4.6 | 3.4 | 0.0 | 11.7 | 18 | 22 | 48 |
| two wolves | Warlock | 3 | 30/32 | 4.3 | 4.9 | 0.0 | 3.2 | 2 | 85 | 33 |
| two wolves | Warlock | 4 | 29/32 | 2.8 | 6.8 | 0.0 | 3.9 | 3 | 85 | 33 |
| two wolves | Warlock | 5 | 32/32 | 0.1 | 21.9 | 0.0 | 0.0 | 0 | 100 | 64 |
| two wolves | Warlock | 6 | 32/32 | 0.1 | 22.0 | 0.0 | 0.0 | 0 | 100 | 85 |
| two wolves | Wizard | 1 | 0/32 | - | 1.7 | 0.0 | 7.0 | 32 | 0 | 0 |
| two wolves | Wizard | 2 | 9/32 | 7.2 | 2.0 | 0.0 | 9.8 | 23 | 18 | 0 |
| two wolves | Wizard | 3 | 30/32 | 4.2 | 4.9 | 0.0 | 2.5 | 2 | 85 | 12 |
| two wolves | Wizard | 4 | 32/32 | 3.1 | 7.0 | 0.0 | 0.8 | 0 | 96 | 25 |
| two wolves | Wizard | 5 | 32/32 | 0.1 | 21.2 | 0.0 | 0.3 | 0 | 99 | 65 |
| two wolves | Wizard | 6 | 32/32 | 0.0 | 22.0 | 0.0 | 0.0 | 0 | 100 | 70 |
| goblin camp | Barbarian | 1 | 30/32 | 4.8 | 3.8 | 0.0 | 5.5 | 2 | 60 | 50 |
| goblin camp | Barbarian | 2 | 31/32 | 4.9 | 3.9 | 0.0 | 5.8 | 1 | 74 | 50 |
| goblin camp | Barbarian | 3 | 32/32 | 5.4 | 3.7 | 0.0 | 6.8 | 0 | 79 | 66 |
| goblin camp | Barbarian | 4 | 32/32 | 4.7 | 4.3 | 0.0 | 6.2 | 0 | 84 | 66 |
| goblin camp | Barbarian | 5 | 32/32 | 1.6 | 12.1 | 0.0 | 1.6 | 0 | 97 | 66 |
| goblin camp | Barbarian | 6 | 32/32 | 1.1 | 15.1 | 0.0 | 1.7 | 0 | 97 | 75 |
| goblin camp | Bard | 1 | 11/32 | 6.1 | 3.1 | 2.3 | 9.7 | 21 | 18 | 64 |
| goblin camp | Bard | 2 | 15/32 | 3.8 | 3.3 | 1.8 | 12.0 | 17 | 32 | 53 |
| goblin camp | Bard | 3 | 32/32 | 4.3 | 4.7 | 2.9 | 8.3 | 0 | 74 | 46 |
| goblin camp | Bard | 4 | 32/32 | 3.6 | 5.6 | 1.1 | 6.2 | 0 | 80 | 59 |
| goblin camp | Bard | 5 | 32/32 | 4.9 | 4.1 | 1.0 | 5.2 | 0 | 87 | 60 |
| goblin camp | Bard | 6 | 32/32 | 1.2 | 17.2 | 0.0 | 1.2 | 0 | 97 | 84 |
| goblin camp | Cleric | 1 | 11/32 | 7.7 | 2.4 | 2.2 | 9.5 | 21 | 19 | 8 |
| goblin camp | Cleric | 2 | 11/32 | 4.9 | 1.9 | 2.0 | 13.8 | 21 | 21 | 42 |
| goblin camp | Cleric | 3 | 21/32 | 9.1 | 1.5 | 14.2 | 25.2 | 11 | 47 | 28 |
| goblin camp | Cleric | 4 | 30/32 | 8.7 | 2.0 | 7.8 | 16.1 | 2 | 69 | 40 |
| goblin camp | Cleric | 5 | 32/32 | 0.8 | 13.9 | 0.5 | 2.8 | 0 | 93 | 82 |
| goblin camp | Cleric | 6 | 32/32 | 0.4 | 19.3 | 0.0 | 1.8 | 0 | 95 | 85 |
| goblin camp | Druid | 1 | 15/32 | 6.6 | 2.7 | 2.0 | 7.8 | 17 | 35 | 6 |
| goblin camp | Druid | 2 | 26/32 | 4.6 | 3.9 | 2.2 | 10.1 | 6 | 47 | 30 |
| goblin camp | Druid | 3 | 32/32 | 3.5 | 5.8 | 2.2 | 7.1 | 0 | 76 | 55 |
| goblin camp | Druid | 4 | 32/32 | 4.0 | 5.0 | 2.0 | 8.1 | 0 | 77 | 55 |
| goblin camp | Druid | 5 | 32/32 | 3.0 | 6.7 | 0.5 | 5.7 | 0 | 84 | 69 |
| goblin camp | Druid | 6 | 32/32 | 2.4 | 8.2 | 0.0 | 4.8 | 0 | 88 | 79 |
| goblin camp | Fighter | 1 | 22/32 | 3.3 | 5.1 | 0.0 | 7.2 | 10 | 40 | 100 |
| goblin camp | Fighter | 2 | 31/32 | 2.8 | 7.2 | 0.0 | 5.2 | 1 | 74 | 71 |
| goblin camp | Fighter | 3 | 32/32 | 2.4 | 8.3 | 0.0 | 4.2 | 0 | 85 | 75 |
| goblin camp | Fighter | 4 | 32/32 | 2.2 | 9.2 | 0.0 | 2.4 | 0 | 93 | 79 |
| goblin camp | Fighter | 5 | 32/32 | 0.9 | 17.6 | 0.0 | 1.8 | 0 | 96 | 96 |
| goblin camp | Fighter | 6 | 32/32 | 0.6 | 19.3 | 0.0 | 2.3 | 0 | 96 | 100 |
| goblin camp | Monk | 1 | 7/32 | 5.8 | 2.8 | 0.0 | 7.9 | 25 | 12 | 100 |
| goblin camp | Monk | 2 | 17/32 | 8.0 | 2.3 | 0.0 | 11.9 | 15 | 20 | 33 |
| goblin camp | Monk | 3 | 32/32 | 8.2 | 2.4 | 0.0 | 2.7 | 0 | 87 | 27 |
| goblin camp | Monk | 4 | 32/32 | 6.5 | 3.1 | 0.0 | 1.0 | 0 | 96 | 25 |
| goblin camp | Monk | 5 | 32/32 | 2.0 | 9.7 | 0.0 | 0.0 | 0 | 100 | 63 |
| goblin camp | Monk | 6 | 32/32 | 1.7 | 10.8 | 0.0 | 0.3 | 0 | 99 | 76 |
| goblin camp | Paladin | 1 | 22/32 | 2.9 | 4.9 | 2.5 | 7.8 | 10 | 51 | 46 |
| goblin camp | Paladin | 2 | 30/32 | 3.8 | 4.7 | 1.9 | 6.5 | 2 | 74 | 73 |
| goblin camp | Paladin | 3 | 32/32 | 4.1 | 4.9 | 2.0 | 7.1 | 0 | 80 | 73 |
| goblin camp | Paladin | 4 | 32/32 | 3.7 | 5.3 | 0.0 | 5.5 | 0 | 83 | 87 |
| goblin camp | Paladin | 5 | 32/32 | 2.6 | 7.4 | 0.0 | 4.5 | 0 | 88 | 93 |
| goblin camp | Paladin | 6 | 32/32 | 1.9 | 9.6 | 0.0 | 4.7 | 0 | 89 | 94 |
| goblin camp | Ranger | 1 | 6/32 | 5.6 | 1.8 | 0.0 | 10.0 | 26 | 9 | 66 |
| goblin camp | Ranger | 2 | 12/32 | 6.8 | 2.2 | 0.0 | 14.7 | 20 | 18 | 61 |
| goblin camp | Ranger | 3 | 25/32 | 6.4 | 2.7 | 0.0 | 14.3 | 7 | 43 | 61 |
| goblin camp | Ranger | 4 | 31/32 | 5.5 | 3.6 | 0.0 | 8.7 | 1 | 73 | 68 |
| goblin camp | Ranger | 5 | 32/32 | 2.8 | 7.3 | 0.0 | 4.3 | 0 | 89 | 86 |
| goblin camp | Ranger | 6 | 32/32 | 3.0 | 6.7 | 0.0 | 6.6 | 0 | 85 | 83 |
| goblin camp | Rogue | 1 | 8/32 | 5.6 | 3.0 | 0.0 | 9.1 | 24 | 9 | 100 |
| goblin camp | Rogue | 2 | 18/32 | 5.3 | 3.2 | 0.0 | 12.2 | 14 | 28 | 100 |
| goblin camp | Rogue | 3 | 22/32 | 6.2 | 2.7 | 0.0 | 17.1 | 10 | 29 | 100 |
| goblin camp | Rogue | 4 | 30/32 | 6.1 | 3.2 | 0.0 | 13.6 | 2 | 56 | 100 |
| goblin camp | Rogue | 5 | 31/32 | 5.4 | 3.5 | 0.0 | 7.8 | 1 | 79 | 100 |
| goblin camp | Rogue | 6 | 32/32 | 5.7 | 3.5 | 0.0 | 7.6 | 0 | 83 | 100 |
| goblin camp | Sorcerer | 1 | 1/32 | 2.5 | 3.1 | 0.0 | 6.9 | 31 | 1 | 41 |
| goblin camp | Sorcerer | 2 | 24/32 | 3.4 | 4.9 | 0.0 | 4.8 | 8 | 60 | 28 |
| goblin camp | Sorcerer | 3 | 31/32 | 1.5 | 11.2 | 0.0 | 1.9 | 1 | 91 | 50 |
| goblin camp | Sorcerer | 4 | 32/32 | 1.2 | 16.7 | 0.0 | 2.1 | 0 | 92 | 64 |
| goblin camp | Sorcerer | 5 | 32/32 | 0.0 | 20.0 | 0.0 | 0.0 | 0 | 100 | 87 |
| goblin camp | Sorcerer | 6 | 32/32 | 0.0 | 20.0 | 0.0 | 0.0 | 0 | 100 | 88 |
| goblin camp | Warlock | 1 | 5/32 | 9.2 | 1.5 | 0.0 | 8.0 | 27 | 11 | 0 |
| goblin camp | Warlock | 2 | 12/32 | 3.8 | 2.2 | 0.0 | 10.8 | 20 | 28 | 54 |
| goblin camp | Warlock | 3 | 26/32 | 7.7 | 2.4 | 0.0 | 9.8 | 6 | 53 | 33 |
| goblin camp | Warlock | 4 | 29/32 | 6.5 | 2.8 | 0.0 | 9.3 | 3 | 65 | 33 |
| goblin camp | Warlock | 5 | 32/32 | 0.0 | 20.0 | 0.0 | 0.0 | 0 | 100 | 66 |
| goblin camp | Warlock | 6 | 32/32 | 0.1 | 19.9 | 0.0 | 0.2 | 0 | 100 | 84 |
| goblin camp | Wizard | 1 | 3/32 | 9.5 | 1.1 | 0.0 | 6.6 | 29 | 6 | 0 |
| goblin camp | Wizard | 2 | 4/32 | 10.5 | 1.1 | 0.0 | 10.7 | 28 | 11 | 0 |
| goblin camp | Wizard | 3 | 24/32 | 7.6 | 2.4 | 0.0 | 6.2 | 8 | 63 | 0 |
| goblin camp | Wizard | 4 | 31/32 | 5.5 | 3.6 | 0.0 | 3.9 | 1 | 82 | 13 |
| goblin camp | Wizard | 5 | 32/32 | 0.1 | 19.3 | 0.0 | 0.1 | 0 | 100 | 59 |
| goblin camp | Wizard | 6 | 32/32 | 0.0 | 20.0 | 0.0 | 0.6 | 0 | 98 | 65 |
