# Game Lab MVP: try it on the Mac

This is the local-first MVP of [Game Lab](README.md) (#11616). You chat in
the OpenAgents web app running on this Mac. The chat opens, changes, plays
and compares The Ice Cream Man with Game Lab's tools, and the game runs in a
card under the answer.

## Start it

One command, from a checkout of `main` (the `coderdev` one is kept current):

```
~/work/openagents-coderdev/scripts/dev/full-local.sh start --lab
```

Then open **http://127.0.0.1:4301** and start a new chat. Signed in or not
both work: only this Mac reaches the Lab.

`--lab` starts Video Lab and Game Lab together. `--game` starts Game Lab
alone. Game Lab is the service on http://127.0.0.1:8793, and its log is
`~/.openagents/full-local/logs/game.log`. The first start builds the game for
the browser, which takes a few minutes. Until then the card says the game is
getting ready, and it refreshes by itself.

## Talk to it

Send these one at a time in the same chat:

1. **open the ice cream man**
   - The answer gives version 1, the game as it plays today, and what the
     bot measured.
   - The card shows the game in a frame, with Play, the bot-run stats, and
     "Note at this time and place".
2. **make the ice cream man 20% slower and give the player two more hiding spots**
   - The chat lowers his four speeds by 20% and adds two spare closets.
   - The bot plays 24 seeded houses with the change, and it's saved as
     version 2.
   - The answer names what changed. The card plays version 2, and its stats
     show two more hiding spots.
3. **play it**
   - The card's frame starts a run of version 2 straight away. Click in the
     frame to steer.
   - You can also press **Play** on any card.
4. **compare with the original**
   - The card shows version 1 and version 2 side by side, with the bot
     playing both on the same house.
   - Under them are the measured differences over the same 24 runs (escapes,
     the middle escape time, hits taken, times spotted, hiding spots) and
     the values that changed.
5. **make the front door locked forever**
   - Nothing is saved. The Lab refuses the edit with this reason: "With the
     front door locked for good there's no way out: the front door is the
     only exit, so nobody could ever escape." The chat says so in its own
     words.
   - Other edits are refused the same way:
     - a door that takes ten minutes to open, which the bot never escapes;
     - a setting the game doesn't have, which needs a code change (the chat
       offers Coder);
     - numbers out of bounds.

**Note at this time and place** under the game saves a note with the run's
time and room. "Read my notes" in the chat reads them with `game_notes`.

## The same tools in the terminal

```
openagents game list
openagents game open
openagents game play --version 2
openagents game edit --patch '[{"op":"replace","path":"/man/man_chase","value":4.48}]' --summary "He chases 20% slower"
openagents game versions
openagents game compare --a 1
openagents game notes
```

Each prints its result in words, and `open` and `play` give a link that plays
the version in its own window.

## Where things are

- **Versions:**
  `~/work/film-out/lab/games/the-ice-cream-man/vN/{doc.json,version.json}`.
  They're private and never in git.
- **Notes:** `~/work/film-out/lab/games/the-ice-cream-man/notes.json`.
- **The game's build:** `~/work/film-out/lab/games/builds/<code digest>/`.
  The game builds again only when its code changes.
- **The schema:** [schemas/game.v1.json](schemas/game.v1.json). To
  regenerate it, run `cargo run -p game-lab -- schema`.

## Not in this MVP

- Coder making code-level changes from the chat. The chat offers it, and it's
  the next slice.
- The `game_*` tools in Coder in the terminal.
- The phone card.
- Games other than The Ice Cream Man.
