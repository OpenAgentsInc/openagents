# Encounter time to kill

Written by `coax/web/tool.sh coax/web/room/encounter-sim.coax --write docs/dnd/encounter-ttk.md` (#13288): every
class at levels 1 to 6, alone, against the third wolf pack (three Wolves), the path's guard (a level 10 Fighter),
the first pack (one Wolf) and the second (two Wolves),
in the real room's rules, 32 fights (seeds 1 to 32) a row. Wins: fights where every foe fell first. TTK: the mean seconds from
the player's first harmful press to the last foe's fall, over the wins. Damage/s, healed and taken: per fight.
Downs: fights where the player fell. HP and resources (spell slots, limited uses, Focus) left: at the fight's end.
A fight lasts at most 120 s. The after-landing check fails a change that moves a row's TTK, damage a second or
wins by more than 25%; run this again with --write in the same commit when the change is meant.

| Encounter | Class | Level | Wins | TTK s | Damage/s | Healed | Taken | Downs | HP left % | Resources left % |
|---|---|---|---|---|---|---|---|---|---|---|
| wolves | Barbarian | 1 | 18/32 | 5.0 | 5.2 | 0.0 | 10.7 | 14 | 24 | 100 |
| wolves | Barbarian | 2 | 21/32 | 6.4 | 4.3 | 0.0 | 13.6 | 11 | 41 | 100 |
| wolves | Barbarian | 3 | 27/32 | 7.8 | 3.7 | 0.0 | 17.9 | 5 | 44 | 100 |
| wolves | Barbarian | 4 | 32/32 | 6.9 | 4.8 | 0.0 | 12.7 | 0 | 68 | 100 |
| wolves | Barbarian | 5 | 32/32 | 2.1 | 14.9 | 0.0 | 3.7 | 0 | 93 | 100 |
| wolves | Barbarian | 6 | 32/32 | 1.8 | 16.3 | 0.0 | 2.1 | 0 | 96 | 100 |
| wolves | Bard | 1 | 7/32 | 2.6 | 11.3 | 0.9 | 8.2 | 25 | 19 | 74 |
| wolves | Bard | 2 | 16/32 | 3.6 | 7.3 | 5.4 | 14.5 | 16 | 39 | 66 |
| wolves | Bard | 3 | 19/32 | 6.3 | 4.7 | 10.9 | 22.7 | 13 | 44 | 43 |
| wolves | Bard | 4 | 29/32 | 5.6 | 5.2 | 12.5 | 21.8 | 3 | 65 | 51 |
| wolves | Bard | 5 | 32/32 | 6.5 | 5.1 | 7.1 | 16.2 | 0 | 72 | 42 |
| wolves | Bard | 6 | 32/32 | 1.3 | 25.3 | 0.0 | 0.2 | 0 | 100 | 84 |
| wolves | Cleric | 1 | 0/32 | - | 5.4 | 0.0 | 9.0 | 32 | 0 | 0 |
| wolves | Cleric | 2 | 6/32 | 5.3 | 3.4 | 2.5 | 15.3 | 26 | 14 | 41 |
| wolves | Cleric | 3 | 23/32 | 8.1 | 3.3 | 13.5 | 23.9 | 9 | 50 | 32 |
| wolves | Cleric | 4 | 26/32 | 7.3 | 3.9 | 16.3 | 25.8 | 6 | 64 | 36 |
| wolves | Cleric | 5 | 30/32 | 6.6 | 4.8 | 17.4 | 28.9 | 2 | 65 | 47 |
| wolves | Cleric | 6 | 31/32 | 3.7 | 8.3 | 7.3 | 19.8 | 1 | 68 | 66 |
| wolves | Druid | 1 | 4/32 | 3.7 | 8.0 | 0.0 | 8.1 | 28 | 10 | 0 |
| wolves | Druid | 2 | 0/32 | - | 3.4 | 0.0 | 15.0 | 32 | 0 | 80 |
| wolves | Druid | 3 | 0/32 | - | 4.4 | 0.0 | 21.0 | 32 | 0 | 75 |
| wolves | Druid | 4 | 12/32 | 5.6 | 5.1 | 0.0 | 23.3 | 20 | 14 | 77 |
| wolves | Druid | 5 | 17/32 | 7.2 | 4.3 | 0.0 | 27.0 | 15 | 18 | 81 |
| wolves | Druid | 6 | 27/32 | 5.1 | 6.0 | 0.0 | 18.0 | 5 | 53 | 78 |
| wolves | Fighter | 1 | 5/32 | 5.1 | 5.7 | 0.0 | 11.3 | 27 | 5 | 100 |
| wolves | Fighter | 2 | 8/32 | 4.6 | 5.4 | 0.0 | 17.6 | 24 | 12 | 66 |
| wolves | Fighter | 3 | 11/32 | 6.1 | 4.6 | 0.0 | 25.5 | 21 | 9 | 66 |
| wolves | Fighter | 4 | 25/32 | 5.7 | 5.5 | 0.0 | 22.6 | 7 | 37 | 75 |
| wolves | Fighter | 5 | 32/32 | 2.3 | 14.6 | 0.0 | 7.7 | 0 | 82 | 75 |
| wolves | Fighter | 6 | 32/32 | 2.2 | 14.9 | 0.0 | 6.1 | 0 | 88 | 75 |
| wolves | Monk | 1 | 0/32 | - | 3.7 | 0.0 | 9.0 | 32 | 0 | 100 |
| wolves | Monk | 2 | 0/32 | - | 3.6 | 0.0 | 15.0 | 32 | 0 | 40 |
| wolves | Monk | 3 | 13/32 | 13.0 | 2.2 | 0.0 | 16.6 | 19 | 21 | 25 |
| wolves | Monk | 4 | 29/32 | 12.0 | 2.6 | 0.0 | 12.9 | 3 | 52 | 21 |
| wolves | Monk | 5 | 32/32 | 3.5 | 9.4 | 0.0 | 3.5 | 0 | 89 | 35 |
| wolves | Monk | 6 | 32/32 | 3.4 | 9.6 | 0.1 | 2.5 | 0 | 94 | 59 |
| wolves | Paladin | 1 | 10/32 | 6.6 | 4.8 | 3.0 | 11.4 | 22 | 23 | 40 |
| wolves | Paladin | 2 | 12/32 | 6.9 | 3.3 | 8.4 | 21.9 | 20 | 25 | 23 |
| wolves | Paladin | 3 | 30/32 | 7.4 | 4.3 | 7.1 | 17.0 | 2 | 60 | 47 |
| wolves | Paladin | 4 | 30/32 | 5.7 | 5.5 | 6.4 | 18.2 | 2 | 63 | 60 |
| wolves | Paladin | 5 | 32/32 | 3.7 | 8.8 | 1.6 | 11.9 | 0 | 73 | 88 |
| wolves | Paladin | 6 | 32/32 | 4.2 | 8.0 | 1.3 | 12.6 | 0 | 75 | 91 |
| wolves | Ranger | 1 | 0/32 | - | 5.4 | 0.0 | 11.0 | 32 | 0 | 69 |
| wolves | Ranger | 2 | 6/32 | 6.8 | 4.6 | 0.0 | 16.4 | 26 | 9 | 58 |
| wolves | Ranger | 3 | 8/32 | 6.9 | 4.4 | 0.0 | 23.5 | 24 | 6 | 53 |
| wolves | Ranger | 4 | 21/32 | 6.4 | 4.7 | 0.0 | 23.1 | 11 | 27 | 58 |
| wolves | Ranger | 5 | 32/32 | 3.3 | 10.0 | 0.0 | 7.8 | 0 | 80 | 84 |
| wolves | Ranger | 6 | 32/32 | 3.2 | 10.4 | 0.0 | 5.3 | 0 | 88 | 85 |
| wolves | Rogue | 1 | 0/32 | - | 4.5 | 0.0 | 10.0 | 32 | 0 | 100 |
| wolves | Rogue | 2 | 0/32 | - | 2.6 | 0.0 | 17.0 | 32 | 0 | 100 |
| wolves | Rogue | 3 | 3/32 | 8.0 | 3.8 | 0.0 | 23.5 | 29 | 2 | 100 |
| wolves | Rogue | 4 | 18/32 | 8.1 | 3.8 | 0.0 | 25.3 | 14 | 18 | 100 |
| wolves | Rogue | 5 | 29/32 | 8.3 | 3.9 | 0.0 | 20.5 | 3 | 46 | 100 |
| wolves | Rogue | 6 | 32/32 | 8.5 | 3.9 | 0.0 | 18.8 | 0 | 58 | 100 |
| wolves | Sorcerer | 1 | 1/32 | 2.5 | 8.4 | 0.0 | 6.9 | 31 | 2 | 25 |
| wolves | Sorcerer | 2 | 6/32 | 4.8 | 6.2 | 0.0 | 10.8 | 26 | 10 | 38 |
| wolves | Sorcerer | 3 | 31/32 | 4.1 | 7.6 | 0.0 | 2.8 | 1 | 86 | 38 |
| wolves | Sorcerer | 4 | 32/32 | 2.7 | 12.0 | 0.0 | 2.5 | 0 | 90 | 57 |
| wolves | Sorcerer | 5 | 32/32 | 0.1 | 32.9 | 0.0 | 0.0 | 0 | 100 | 87 |
| wolves | Sorcerer | 6 | 32/32 | 0.0 | 33.0 | 0.0 | 0.0 | 0 | 100 | 88 |
| wolves | Warlock | 1 | 0/32 | - | 5.0 | 0.0 | 9.0 | 32 | 0 | 0 |
| wolves | Warlock | 2 | 4/32 | 5.2 | 4.5 | 0.0 | 14.4 | 28 | 4 | 46 |
| wolves | Warlock | 3 | 19/32 | 6.3 | 4.4 | 0.0 | 12.0 | 13 | 43 | 33 |
| wolves | Warlock | 4 | 29/32 | 7.2 | 4.5 | 0.0 | 9.4 | 3 | 65 | 33 |
| wolves | Warlock | 5 | 32/32 | 0.1 | 32.8 | 0.0 | 0.0 | 0 | 100 | 63 |
| wolves | Warlock | 6 | 32/32 | 0.1 | 32.9 | 0.0 | 0.0 | 0 | 100 | 85 |
| wolves | Wizard | 1 | 0/32 | - | 2.5 | 0.0 | 7.0 | 32 | 0 | 0 |
| wolves | Wizard | 2 | 1/32 | 7.5 | 2.7 | 0.0 | 11.8 | 31 | 2 | 0 |
| wolves | Wizard | 3 | 12/32 | 8.5 | 3.6 | 0.0 | 14.1 | 20 | 17 | 4 |
| wolves | Wizard | 4 | 24/32 | 6.5 | 4.6 | 0.0 | 11.0 | 8 | 50 | 6 |
| wolves | Wizard | 5 | 32/32 | 0.2 | 31.3 | 0.0 | 0.0 | 0 | 100 | 64 |
| wolves | Wizard | 6 | 32/32 | 0.3 | 29.8 | 0.0 | 0.0 | 0 | 100 | 68 |
| guard | Barbarian | 1 | 0/32 | - | 2.8 | 0.0 | 14.0 | 32 | 0 | 100 |
| guard | Barbarian | 2 | 0/32 | - | 3.6 | 0.0 | 23.0 | 32 | 0 | 100 |
| guard | Barbarian | 3 | 0/32 | - | 2.4 | 0.0 | 32.0 | 32 | 0 | 100 |
| guard | Barbarian | 4 | 0/32 | - | 2.4 | 0.0 | 41.0 | 32 | 0 | 100 |
| guard | Barbarian | 5 | 0/32 | - | 7.4 | 0.0 | 50.0 | 32 | 0 | 100 |
| guard | Barbarian | 6 | 0/32 | - | 5.5 | 0.0 | 59.0 | 32 | 0 | 100 |
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
| guard | Druid | 2 | 0/32 | - | 2.3 | 0.0 | 15.0 | 32 | 0 | 80 |
| guard | Druid | 3 | 0/32 | - | 3.1 | 0.0 | 21.0 | 32 | 0 | 75 |
| guard | Druid | 4 | 0/32 | - | 3.5 | 0.0 | 27.0 | 32 | 0 | 77 |
| guard | Druid | 5 | 0/32 | - | 4.0 | 0.0 | 33.0 | 32 | 0 | 81 |
| guard | Druid | 6 | 0/32 | - | 4.3 | 5.6 | 44.6 | 32 | 0 | 79 |
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
| one wolf | Barbarian | 1 | 32/32 | 0.6 | 8.4 | 0.0 | 0.1 | 0 | 99 | 100 |
| one wolf | Barbarian | 2 | 32/32 | 1.2 | 6.5 | 0.0 | 1.2 | 0 | 95 | 100 |
| one wolf | Barbarian | 3 | 32/32 | 1.3 | 5.9 | 0.0 | 1.1 | 0 | 96 | 100 |
| one wolf | Barbarian | 4 | 32/32 | 0.9 | 7.2 | 0.0 | 0.5 | 0 | 99 | 100 |
| one wolf | Barbarian | 5 | 32/32 | 0.2 | 11.0 | 0.0 | 0.0 | 0 | 100 | 100 |
| one wolf | Barbarian | 6 | 32/32 | 0.2 | 11.0 | 0.0 | 0.0 | 0 | 100 | 100 |
| one wolf | Bard | 1 | 24/32 | 2.5 | 3.2 | 1.8 | 5.4 | 8 | 59 | 66 |
| one wolf | Bard | 2 | 32/32 | 2.9 | 3.8 | 2.4 | 4.6 | 0 | 85 | 66 |
| one wolf | Bard | 3 | 32/32 | 2.4 | 4.5 | 0.9 | 2.9 | 0 | 90 | 76 |
| one wolf | Bard | 4 | 32/32 | 2.6 | 4.3 | 0.0 | 1.0 | 0 | 96 | 80 |
| one wolf | Bard | 5 | 32/32 | 2.6 | 4.3 | 0.0 | 1.3 | 0 | 96 | 74 |
| one wolf | Bard | 6 | 32/32 | 1.2 | 9.1 | 0.0 | 0.3 | 0 | 99 | 84 |
| one wolf | Cleric | 1 | 30/32 | 0.7 | 7.1 | 0.0 | 0.7 | 2 | 93 | 33 |
| one wolf | Cleric | 2 | 31/32 | 0.8 | 7.3 | 0.0 | 1.0 | 1 | 93 | 68 |
| one wolf | Cleric | 3 | 32/32 | 0.7 | 7.9 | 0.0 | 0.9 | 0 | 96 | 80 |
| one wolf | Cleric | 4 | 32/32 | 0.6 | 9.1 | 0.0 | 0.3 | 0 | 99 | 83 |
| one wolf | Cleric | 5 | 32/32 | 0.6 | 9.2 | 0.0 | 1.3 | 0 | 96 | 78 |
| one wolf | Cleric | 6 | 32/32 | 0.8 | 8.7 | 0.0 | 2.0 | 0 | 95 | 81 |
| one wolf | Druid | 1 | 24/32 | 3.4 | 3.0 | 0.0 | 3.1 | 8 | 66 | 0 |
| one wolf | Druid | 2 | 28/32 | 2.4 | 3.6 | 0.0 | 4.2 | 4 | 72 | 80 |
| one wolf | Druid | 3 | 32/32 | 3.0 | 3.6 | 0.0 | 2.9 | 0 | 86 | 75 |
| one wolf | Druid | 4 | 32/32 | 2.0 | 5.6 | 0.0 | 1.4 | 0 | 95 | 77 |
| one wolf | Druid | 5 | 32/32 | 2.0 | 5.3 | 0.0 | 0.4 | 0 | 99 | 81 |
| one wolf | Druid | 6 | 32/32 | 2.7 | 4.1 | 0.0 | 2.0 | 0 | 95 | 78 |
| one wolf | Fighter | 1 | 32/32 | 1.2 | 7.3 | 0.0 | 2.2 | 0 | 82 | 100 |
| one wolf | Fighter | 2 | 32/32 | 1.4 | 6.7 | 0.0 | 3.0 | 0 | 85 | 66 |
| one wolf | Fighter | 3 | 32/32 | 1.2 | 7.9 | 0.0 | 2.3 | 0 | 92 | 66 |
| one wolf | Fighter | 4 | 32/32 | 0.7 | 9.7 | 0.0 | 1.3 | 0 | 96 | 75 |
| one wolf | Fighter | 5 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 75 |
| one wolf | Fighter | 6 | 32/32 | 0.0 | 11.0 | 0.0 | 0.0 | 0 | 100 | 75 |
| one wolf | Monk | 1 | 27/32 | 2.1 | 4.5 | 0.0 | 3.5 | 5 | 61 | 100 |
| one wolf | Monk | 2 | 32/32 | 2.7 | 4.0 | 0.0 | 2.3 | 0 | 84 | 51 |
| one wolf | Monk | 3 | 32/32 | 2.5 | 4.3 | 0.0 | 0.0 | 0 | 100 | 53 |
| one wolf | Monk | 4 | 32/32 | 2.4 | 4.5 | 0.0 | 0.0 | 0 | 100 | 59 |
| one wolf | Monk | 5 | 32/32 | 0.3 | 10.8 | 0.0 | 0.0 | 0 | 100 | 95 |
| one wolf | Monk | 6 | 32/32 | 0.4 | 10.2 | 0.0 | 0.0 | 0 | 100 | 96 |
| one wolf | Paladin | 1 | 28/32 | 0.2 | 7.4 | 0.0 | 1.4 | 4 | 88 | 89 |
| one wolf | Paladin | 2 | 32/32 | 2.8 | 3.9 | 1.6 | 2.9 | 0 | 93 | 84 |
| one wolf | Paladin | 3 | 32/32 | 1.3 | 6.8 | 0.3 | 1.0 | 0 | 97 | 84 |
| one wolf | Paladin | 4 | 32/32 | 1.1 | 7.8 | 0.0 | 0.0 | 0 | 100 | 89 |
| one wolf | Paladin | 5 | 32/32 | 0.6 | 11.0 | 0.0 | 0.0 | 0 | 100 | 94 |
| one wolf | Paladin | 6 | 32/32 | 0.6 | 10.7 | 0.0 | 0.0 | 0 | 100 | 95 |
| one wolf | Ranger | 1 | 27/32 | 1.0 | 5.4 | 0.0 | 2.8 | 5 | 74 | 73 |
| one wolf | Ranger | 2 | 32/32 | 1.5 | 6.0 | 0.0 | 1.5 | 0 | 91 | 73 |
| one wolf | Ranger | 3 | 32/32 | 1.4 | 5.9 | 0.0 | 2.0 | 0 | 92 | 78 |
| one wolf | Ranger | 4 | 32/32 | 0.7 | 8.9 | 0.0 | 0.4 | 0 | 99 | 80 |
| one wolf | Ranger | 5 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 88 |
| one wolf | Ranger | 6 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 88 |
| one wolf | Rogue | 1 | 24/32 | 1.9 | 4.4 | 0.0 | 4.5 | 8 | 55 | 100 |
| one wolf | Rogue | 2 | 31/32 | 2.0 | 4.9 | 0.0 | 4.1 | 1 | 76 | 100 |
| one wolf | Rogue | 3 | 31/32 | 1.8 | 5.2 | 0.0 | 4.1 | 1 | 83 | 100 |
| one wolf | Rogue | 4 | 32/32 | 1.7 | 6.3 | 0.0 | 4.3 | 0 | 86 | 100 |
| one wolf | Rogue | 5 | 32/32 | 1.5 | 7.2 | 0.0 | 1.3 | 0 | 96 | 100 |
| one wolf | Rogue | 6 | 32/32 | 1.9 | 5.8 | 0.0 | 1.9 | 0 | 95 | 100 |
| one wolf | Sorcerer | 1 | 30/32 | 1.2 | 6.6 | 0.0 | 1.2 | 2 | 83 | 33 |
| one wolf | Sorcerer | 2 | 32/32 | 0.9 | 8.7 | 0.0 | 0.2 | 0 | 98 | 56 |
| one wolf | Sorcerer | 3 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 79 |
| one wolf | Sorcerer | 4 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 83 |
| one wolf | Sorcerer | 5 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 87 |
| one wolf | Sorcerer | 6 | 32/32 | 0.0 | 11.0 | 0.0 | 0.0 | 0 | 100 | 88 |
| one wolf | Warlock | 1 | 28/32 | 1.6 | 4.5 | 0.0 | 1.9 | 4 | 78 | 0 |
| one wolf | Warlock | 2 | 30/32 | 1.4 | 5.2 | 0.0 | 2.3 | 2 | 85 | 63 |
| one wolf | Warlock | 3 | 32/32 | 0.4 | 9.5 | 0.0 | 0.7 | 0 | 97 | 59 |
| one wolf | Warlock | 4 | 32/32 | 0.1 | 10.9 | 0.0 | 0.0 | 0 | 100 | 63 |
| one wolf | Warlock | 5 | 32/32 | 0.0 | 11.0 | 0.0 | 0.0 | 0 | 100 | 66 |
| one wolf | Warlock | 6 | 32/32 | 0.1 | 11.0 | 0.0 | 0.0 | 0 | 100 | 84 |
| one wolf | Wizard | 1 | 22/32 | 4.6 | 1.8 | 0.0 | 2.7 | 10 | 61 | 0 |
| one wolf | Wizard | 2 | 29/32 | 4.6 | 2.1 | 0.0 | 3.6 | 3 | 70 | 0 |
| one wolf | Wizard | 3 | 31/32 | 1.3 | 5.8 | 0.0 | 0.7 | 1 | 96 | 34 |
| one wolf | Wizard | 4 | 32/32 | 0.8 | 8.1 | 0.0 | 0.0 | 0 | 100 | 47 |
| one wolf | Wizard | 5 | 32/32 | 0.0 | 11.0 | 0.0 | 0.0 | 0 | 100 | 66 |
| one wolf | Wizard | 6 | 32/32 | 0.0 | 11.0 | 0.0 | 0.0 | 0 | 100 | 70 |
| two wolves | Barbarian | 1 | 29/32 | 4.2 | 4.8 | 0.0 | 5.5 | 3 | 61 | 100 |
| two wolves | Barbarian | 2 | 31/32 | 3.4 | 5.9 | 0.0 | 4.0 | 1 | 82 | 100 |
| two wolves | Barbarian | 3 | 32/32 | 4.1 | 5.3 | 0.0 | 4.9 | 0 | 85 | 100 |
| two wolves | Barbarian | 4 | 32/32 | 4.3 | 5.0 | 0.0 | 6.0 | 0 | 85 | 100 |
| two wolves | Barbarian | 5 | 32/32 | 0.7 | 20.7 | 0.0 | 0.0 | 0 | 100 | 100 |
| two wolves | Barbarian | 6 | 32/32 | 0.9 | 17.6 | 0.0 | 0.5 | 0 | 99 | 100 |
| two wolves | Bard | 1 | 2/32 | 4.1 | 3.1 | 2.2 | 10.9 | 30 | 3 | 69 |
| two wolves | Bard | 2 | 8/32 | 5.4 | 3.0 | 6.3 | 19.4 | 24 | 12 | 59 |
| two wolves | Bard | 3 | 28/32 | 5.6 | 3.8 | 5.7 | 12.6 | 4 | 67 | 43 |
| two wolves | Bard | 4 | 31/32 | 6.0 | 3.7 | 11.0 | 18.1 | 1 | 73 | 52 |
| two wolves | Bard | 5 | 32/32 | 6.7 | 3.3 | 4.7 | 12.8 | 0 | 75 | 44 |
| two wolves | Bard | 6 | 32/32 | 1.3 | 17.5 | 0.0 | 0.9 | 0 | 98 | 84 |
| two wolves | Cleric | 1 | 14/32 | 3.4 | 3.7 | 0.0 | 5.7 | 18 | 37 | 0 |
| two wolves | Cleric | 2 | 18/32 | 2.3 | 4.5 | 0.8 | 8.1 | 14 | 51 | 47 |
| two wolves | Cleric | 3 | 31/32 | 3.6 | 5.3 | 2.0 | 5.3 | 1 | 84 | 53 |
| two wolves | Cleric | 4 | 32/32 | 4.3 | 5.1 | 3.4 | 7.6 | 0 | 84 | 61 |
| two wolves | Cleric | 5 | 32/32 | 2.5 | 8.2 | 3.8 | 9.7 | 0 | 82 | 69 |
| two wolves | Cleric | 6 | 32/32 | 2.7 | 7.9 | 2.3 | 8.4 | 0 | 84 | 71 |
| two wolves | Druid | 1 | 2/32 | 4.0 | 2.9 | 0.0 | 8.6 | 30 | 5 | 0 |
| two wolves | Druid | 2 | 11/32 | 5.3 | 2.9 | 0.0 | 11.8 | 21 | 22 | 80 |
| two wolves | Druid | 3 | 12/32 | 6.8 | 2.5 | 0.0 | 17.1 | 20 | 18 | 75 |
| two wolves | Druid | 4 | 25/32 | 5.3 | 3.7 | 0.0 | 12.5 | 7 | 53 | 77 |
| two wolves | Druid | 5 | 29/32 | 5.3 | 3.9 | 0.0 | 12.1 | 3 | 63 | 81 |
| two wolves | Druid | 6 | 31/32 | 4.5 | 4.7 | 0.0 | 10.1 | 1 | 74 | 78 |
| two wolves | Fighter | 1 | 17/32 | 3.2 | 6.3 | 0.0 | 8.1 | 15 | 32 | 100 |
| two wolves | Fighter | 2 | 29/32 | 3.3 | 6.3 | 0.0 | 7.6 | 3 | 62 | 66 |
| two wolves | Fighter | 3 | 29/32 | 4.3 | 4.9 | 0.0 | 13.3 | 3 | 52 | 66 |
| two wolves | Fighter | 4 | 32/32 | 3.4 | 6.4 | 0.0 | 7.5 | 0 | 79 | 75 |
| two wolves | Fighter | 5 | 32/32 | 1.1 | 20.6 | 0.0 | 3.1 | 0 | 93 | 75 |
| two wolves | Fighter | 6 | 32/32 | 1.1 | 19.7 | 0.0 | 2.7 | 0 | 95 | 75 |
| two wolves | Monk | 1 | 2/32 | 6.3 | 3.3 | 0.0 | 8.6 | 30 | 4 | 100 |
| two wolves | Monk | 2 | 6/32 | 5.2 | 3.0 | 0.0 | 14.0 | 26 | 7 | 37 |
| two wolves | Monk | 3 | 31/32 | 9.1 | 2.3 | 0.0 | 5.3 | 1 | 74 | 25 |
| two wolves | Monk | 4 | 32/32 | 6.9 | 3.2 | 0.0 | 2.9 | 0 | 89 | 23 |
| two wolves | Monk | 5 | 32/32 | 1.9 | 11.6 | 0.0 | 0.3 | 0 | 99 | 67 |
| two wolves | Monk | 6 | 32/32 | 1.8 | 12.4 | 0.2 | 1.1 | 0 | 98 | 79 |
| two wolves | Paladin | 1 | 25/32 | 2.6 | 6.8 | 2.2 | 6.1 | 7 | 65 | 48 |
| two wolves | Paladin | 2 | 30/32 | 4.2 | 4.6 | 3.1 | 6.8 | 2 | 79 | 64 |
| two wolves | Paladin | 3 | 32/32 | 3.9 | 5.7 | 3.1 | 8.7 | 0 | 78 | 66 |
| two wolves | Paladin | 4 | 32/32 | 2.9 | 7.6 | 1.6 | 8.0 | 0 | 80 | 80 |
| two wolves | Paladin | 5 | 32/32 | 2.0 | 10.7 | 0.0 | 2.8 | 0 | 93 | 93 |
| two wolves | Paladin | 6 | 32/32 | 1.7 | 12.0 | 0.0 | 2.4 | 0 | 95 | 94 |
| two wolves | Ranger | 1 | 14/32 | 3.9 | 3.9 | 0.0 | 7.4 | 18 | 33 | 63 |
| two wolves | Ranger | 2 | 28/32 | 4.2 | 5.1 | 0.0 | 7.5 | 4 | 58 | 65 |
| two wolves | Ranger | 3 | 31/32 | 4.6 | 4.7 | 0.0 | 9.4 | 1 | 62 | 65 |
| two wolves | Ranger | 4 | 31/32 | 3.8 | 5.5 | 0.0 | 7.6 | 1 | 76 | 71 |
| two wolves | Ranger | 5 | 32/32 | 1.6 | 13.8 | 0.0 | 0.3 | 0 | 99 | 88 |
| two wolves | Ranger | 6 | 32/32 | 1.4 | 15.5 | 0.0 | 0.8 | 0 | 98 | 88 |
| two wolves | Rogue | 1 | 2/32 | 4.1 | 4.6 | 0.0 | 9.6 | 30 | 4 | 100 |
| two wolves | Rogue | 2 | 2/32 | 5.2 | 3.6 | 0.0 | 16.4 | 30 | 3 | 100 |
| two wolves | Rogue | 3 | 24/32 | 4.7 | 3.8 | 0.0 | 13.1 | 8 | 45 | 100 |
| two wolves | Rogue | 4 | 29/32 | 3.9 | 5.0 | 0.0 | 8.3 | 3 | 73 | 100 |
| two wolves | Rogue | 5 | 32/32 | 3.9 | 5.6 | 0.0 | 3.8 | 0 | 90 | 100 |
| two wolves | Rogue | 6 | 32/32 | 4.8 | 4.6 | 0.0 | 4.3 | 0 | 90 | 100 |
| two wolves | Sorcerer | 1 | 9/32 | 3.2 | 4.8 | 0.0 | 5.3 | 23 | 24 | 25 |
| two wolves | Sorcerer | 2 | 30/32 | 3.3 | 6.4 | 0.0 | 4.5 | 2 | 62 | 33 |
| two wolves | Sorcerer | 3 | 32/32 | 1.2 | 18.3 | 0.0 | 0.0 | 0 | 100 | 62 |
| two wolves | Sorcerer | 4 | 32/32 | 1.2 | 19.0 | 0.0 | 0.0 | 0 | 100 | 69 |
| two wolves | Sorcerer | 5 | 32/32 | 0.1 | 21.9 | 0.0 | 0.0 | 0 | 100 | 86 |
| two wolves | Sorcerer | 6 | 32/32 | 0.1 | 22.0 | 0.0 | 0.0 | 0 | 100 | 88 |
| two wolves | Warlock | 1 | 4/32 | 7.1 | 2.9 | 0.0 | 8.5 | 28 | 5 | 0 |
| two wolves | Warlock | 2 | 16/32 | 4.7 | 3.5 | 0.0 | 10.6 | 16 | 29 | 48 |
| two wolves | Warlock | 3 | 30/32 | 4.4 | 4.8 | 0.0 | 4.3 | 2 | 80 | 33 |
| two wolves | Warlock | 4 | 31/32 | 3.6 | 5.9 | 0.0 | 3.9 | 1 | 85 | 33 |
| two wolves | Warlock | 5 | 32/32 | 0.1 | 21.9 | 0.0 | 0.0 | 0 | 100 | 64 |
| two wolves | Warlock | 6 | 32/32 | 0.1 | 22.0 | 0.0 | 0.0 | 0 | 100 | 85 |
| two wolves | Wizard | 1 | 0/32 | - | 1.8 | 0.0 | 7.0 | 32 | 0 | 0 |
| two wolves | Wizard | 2 | 6/32 | 7.3 | 2.0 | 0.0 | 10.8 | 26 | 10 | 0 |
| two wolves | Wizard | 3 | 28/32 | 4.9 | 4.0 | 0.0 | 4.8 | 4 | 72 | 9 |
| two wolves | Wizard | 4 | 31/32 | 3.2 | 6.5 | 0.0 | 1.9 | 1 | 91 | 21 |
| two wolves | Wizard | 5 | 32/32 | 0.1 | 21.2 | 0.0 | 0.3 | 0 | 99 | 65 |
| two wolves | Wizard | 6 | 32/32 | 0.0 | 22.0 | 0.0 | 0.0 | 0 | 100 | 70 |
