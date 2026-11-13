# Everglade's first hour

This page describes what a new player does in Everglade's first hour, alone,
and what the room does when they fall. Everything here is rows of
`everglade/content/everglade.content` and rules the room owns. The page draws
it and asks. It decides nothing.

## The quests

The Warden stands at the glade's edge. Each quest is handed in to him, and the
same talk offers the next one.

| Order | Quest | Steps | Reward | You are level |
| --- | --- | --- | --- | --- |
| 1 | Thin the pack | Slay 1 Wolf (the lone wolf's den). Talk to the Warden. | Warden's cloak, 25 coin, 250 XP | 1, then 2 |
| 2 | The road south | Go to the Thicket overlook. Talk to the Warden. | 10 coin, 600 XP | 2, then 3 |
| 3 | The thicket pair | Go to The thicket. Slay 2 Wolves. Talk to the Warden. | 40 coin, 1,700 XP | 3, then 4 |
| 4 | The hollow | Go to The hollow. Slay 3 Wolves. Bring 1 Wolf pelt. Talk to the Warden. | 60 coin, 400 XP | 4 |
| 5 | The goblin camp | Go to the Goblin camp. Slay 2 Goblin Warriors. Talk to the Warden. | Traveller's ring, 50 coin, 150 XP | 4 |

A Wolf and a Goblin Warrior each give 50 XP, shared among the players who
fought it. Levels are the SRD's: 300 XP for level 2, 900 for level 3 and 2,700
for level 4. Drops and rewards are operations of the item ledger
(`coax/web/room/room-items.coax`). A reward is paid once.

### How a chain is made

A chain needs no code and no new column. A dialogue that one quest's step asks
for, and that offers another quest, is a link:

- Quest 1's last step is a talk to dialogue 3.
- Dialogue 3 offers quest 3 (The road south).

The room takes the offer only once the first quest has come to that talk
(`eq_offer_open` in `everglade/model/content.coax`). So one talk hands in the
first quest and takes the next, and no one takes a later quest early. The page
uses the same rule to decide when to show the Warden's prompt.

## Can every class win each step alone?

The numbers are from `docs/dnd/encounter-ttk.md`: 32 fights for each class and
level, in the real room's rules, with a scripted player who stands still and
presses what its bar allows.

| Step | Level | Result |
| --- | --- | --- |
| The lone Wolf | 1 | Every class wins most fights. The lowest are the Wizard (22 of 32), then the Bard, Druid and Rogue (24). |
| The pair | 3 | Every class wins at least 25 of 32. The Rogue is lowest (25), then the Wizard (28). |
| The three | 4 | Ten classes win at least 24 of 32. The Ranger wins 21 and the Rogue 18. |
| The camp's two Warriors | 4 | Every class wins at least 29 of 32. At level 3 the Cleric would win 21, the Rogue 22 and the Wizard 24. |

Who struggles, and why:

- **Rogue, against the three.** A Rogue makes one attack a turn, and Sneak
  Attack needs Advantage or an ally beside the target. Alone it often has
  neither, while three Wolves with Pack Tactics have Advantage on it and
  knock it Prone.
- **Ranger, against the three.** Its bar is a bow's. Three Wolves close at
  once, and a ranged attack with a foe within 5 feet has Disadvantage.
- **Wizard, Bard, Druid, at level 1.** They have the fewest Hit Points (6 to
  9) and a Wolf's bite does 5 on average, so two bites can drop them before
  their cantrips drop the Wolf.
- **The pair at level 2** would be too hard for the Monk (4 of 32), the Rogue
  (2), the Wizard (4), the Bard (11) and the Druid (12). That is why The road
  south, which has no fight, comes first and gives level 3.
- **The three at level 3** would be too hard for the Rogue (3 of 32), the
  Ranger (8), the Fighter (11), the Wizard (12) and the Monk (13). That is why
  the pair's reward is 1,700 XP: it gives level 4 before the hollow.

A lost fight costs time only: see the next section.

### How the camp is sized

Four Goblins at once beat every class at levels 1 to 4 (0 wins of 32). So the
tables split the camp, and the rules are unchanged:

- The two Warriors at the fire are one group. They notice a player at 18 m.
- Each archer is its own group and notices at 9 m, from its perch.

A player who comes from the north draws the two Warriors out and fights them
away from the archers. The quest asks for the two Warriors only.

## When you fall

At 0 Hit Points you are Unconscious, as the SRD says (5.2.1, "Dropping to 0 Hit
Points").

1. **You are dying.** The room rolls a Death Saving Throw on each of your
   turns, one every 6 seconds. The line over your bar counts them: "You are
   down · death saves: 1 saved, 2 failed". The monsters leave you and go home.
2. **An ally can help.** A heal stands you up. The Healer's Kit makes you
   Stable. A natural 20 on a save stands you up with 1 Hit Point.
3. **The saves settle.** Three successes make you Stable. Three failures, or
   one very heavy blow, kill you.
4. **You wake.** Ten seconds after the saves settle, you wake with all your
   Hit Points where you first arrived, in the glade, where no monster comes.
   If an ally who is up stands within 30 m, the room waits 60 seconds first,
   so the ally can heal or revive you where you lie.

You lose nothing: your quests, bag, coin and XP are as they were. Spell slots
and limited uses are not restored. At most it takes about 40 seconds from the
fall to the wake.

The room owns all of it (`ev_rise` in `everglade/server/progress.coax`). The
page moves you when the room says so, as it does for a moderator's move.

## Prove it

```sh
coax/remote/remote.sh run -- everglade/tools/loop-proof.sh 4 900 full
```

It runs these in local `--dev` rooms, each in its own room, and prints
`loop-proof: holds` or what was missing:

| Run | What it must show |
| --- | --- |
| party | Four bots together take the quests, kill, loot, hand in, are paid once, and put the cloak on. |
| fallen | One level 1 Wizard walks into the packs and presses nothing. It goes down and wakes in the glade. |
| solo | A Fighter, a Wizard and a Rogue, each alone, are paid the first two quests and reach level 3. Each line says how far down the chain the bot got. |

Set `LOOP_CLASSES` to other class numbers (0 Barbarian to 11 Wizard) to try
other classes alone.
