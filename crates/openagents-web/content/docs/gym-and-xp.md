# The Gym and XP

The Gym is where plugins are tested on Coder: the same tests with the
plugin and without it, so anyone can see whether it helps. It lives in the
chat, and its results also stand on boards in the Verse.

## In the chat

On the phone's Chat menu, tap a starter, or ask in any chat:

- **What's new.** The Gym's news, from published results, checks, our
  changelog, and our notes.
- **Check a result.** Rerun another trainer's published result. If you get
  the same verdict, your check confirms it; otherwise it disputes it. You
  can't check your own result.
- **Make a plugin.** Say "Help me make a plugin that …" and we draft it and
  its tests with you. See [Write a plugin](/docs/write-a-plugin).

**Add to the Gym** publishes a result. The sheet shows exactly what becomes
public before anything does.

## In the Verse

The Gym building stands straight ahead of where you start in the Grid. Its
boards show published results by test set, with the checks on each, and
our earlier coding results step by step. **See the board** under a
plugin's result in chat walks you there. See [The Verse](/docs/verse).

## XP

XP is a record of work other people used. It isn't money: it can't be
spent, sent, or exchanged.

You earn XP when:

- **Someone checks your result to protocol.** The checker earns 50 XP, the
  trainer who ran the result 25, and the test set's author 25, whether the
  check confirms or disputes.
- **Your plugin is adopted** into Coder's defaults. Its author earns 200
  XP, the author of the tests that showed it 100, and each tester whose
  result was confirmed 50.

These are the amounts in the current quests. Running, publishing, or
viewing earns nothing by itself.

**Tutorial quests.** Six quests each repeat one of our published coding
results on your own computer and pay 50 XP each, once per person. They
need a desktop computer with Docker and the command line, and a run costs
about a cent. The season closes 2026-12-25.

**Playtest XP** is separate: it comes from accepted feedback and bug
reports during the playtest, and shows beside your trainer XP.

## Your trainer card and level

**Account → Trainer** on the phone is your trainer card: your level, XP,
the XP to the next level, titles, and the awards behind them. Everyone
starts at level 1; level 2 comes at 100 XP, level 3 at 283, and level 4 at
520. Anyone can recompute your level from the signed records on our relay.

- **Show my level** lets other players see it over your head in the Grid.
  **Hide my level** takes it down.
- **Export card** signs your card and shares it.
- **Link a key** lets XP earned on a computer count toward your level: tap
  it, enter the computer key's npub, then run `microcoder xp link --relay
  wss://relay.openagents.com --trainer YOUR_NPUB` on the computer. Neither
  secret key moves.

The menu's **Profile** shows what you made and the XP each item earned.

Next: [The Verse](/docs/verse).
