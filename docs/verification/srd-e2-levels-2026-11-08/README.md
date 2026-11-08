# Everglade II levels 1 to 6, bar counts and refused-cast fades

This is the #13046 world integration for SRD levels 1 to 6 in Everglade II (umbrella #13030).

## What changed

- **Levels.** Character Create has a LEVEL row (1 to 6). The appearance record saves it as
  CC4 past level 1. Level 1 is still saved as CC3, and a CC3 record reads as level 1. The
  hello's KIND says `srd-CLASS-L`. The room seats the creature at that level with the shared
  rules (`e5rr_join_level`). The room rejoins a seat whose level changes. Everglade's XP
  levels no longer rebuild an Everglade II seat at level 1.
- **Wire slots.** A slot is the ability's place in its class's level 1 to 6 catalog
  (`xk_wire`), so level 1 slots keep their numbers. The room takes a slot only from the
  seat's own level's bar (`xk_entry_at`).
- **Weapon attacks.** Attack, Reckless Attack and Flurry of Blows swing at the air. They pay
  the real Action, Bonus Action and Focus, and they never pick a creature. Action Surge goes
  through the shared rules and gives an Action back. Produce Flame conjures the flame first.
- **The bar.** The room sends `b N V...` for each seat after an accepted cast and whenever
  the bar changes. It says how many uses are left and whether a feature (Rage, Wild Shape)
  holds. The bar draws these as on-icon counts (red at 0) and a gold frame. It draws no text
  banners.
- **Refusals.** When the room refuses a cast (`r SEQ WHY`), the page undoes its prediction.
  The voice fades out (`-x<voice>`), the puffs fade to nothing over 0.25 s, and the queued
  fizzle is dropped.

## Real sockets

`coax/web/tool.sh coax/web/room/e2-restart-check.coax --levels` starts the room binary and
seats three players over real sockets. Output: `socket-levels.log`.

| Step | Held |
| --- | --- |
| Each player hears its own level's bar (Fighter 5: 4 slots, Fighter 1: 3, Barbarian 3: 4) | yes |
| The level 5 Fighter's Action Surge is taken | yes |
| and its count drops by one | yes |
| The level 1 Fighter sends the same slot; the room refuses it | yes |
| The level 5 Fighter's weapon Attack is taken | yes |
| The level 3 Barbarian's Rage is taken, and the room says it holds | yes |
| A second Rage while raging is refused | yes |

## Headless Chrome

`everglade2-crowd --levels` follows the real path with the mouse. Page 0 picks FIGHTER and
LEVEL 5 in Character Create. Page 1 picks BARBARIAN and LEVEL 3. Both press USE IN
EVERGLADE and sit in one local room. Then the check reads the bar counts and the Rage frame,
and it sees the refused second Rage undone and faded. Output: `crowd-levels.log` and
`levels-0.jpg` / `levels-1.jpg`.

```
coax/web/tool.sh coax/web/room/local.coax --room 9147 --page 9148 --static --no-build
coax/web/tool.sh everglade2-crowd 'http://localhost:9148/worlds/character-creator?relay=ws://localhost:9147' \
  --players 2 --seconds 3 --levels --shot DIR
```

| Step | Held |
| --- | --- |
| Page 0 picks FIGHTER, LEVEL 5; page 1 picks BARBARIAN, LEVEL 3 (mouse) | yes |
| Each page hears its own level's bar from the room, 4 slots each | yes |
| Fighter: Action Surge taken; its count goes 1 to 0 (red) | yes |
| Barbarian: Rage taken; its icon gets the gold frame | yes |
| A second Rage: the room refuses it; 1 cast undone, its fade seen running, then 0 | yes |
| The whole crowd check (seats, names, no pops) | yes |

`levels-0.jpg` shows the Fighter's bar: Second Wind 3, Action Surge 0 in red.
`levels-1.jpg` shows the Barbarian's Rage icon framed, with 2 Rages left.

The click-through also found a defect. Every room probe (`e2_net_probe`) copied the whole
`E2Net` onto the stack, and that overflowed it ("memory access out of bounds"). Then the
crowd check could not find USE IN EVERGLADE. The probes now read through a view.

## Checks

- `everglade2-rules-checks.coax`: 365 checks. `coax mutate` on `everglade2-rules.coax`:
  90 mutants, 90 killed.
- `everglade2-rules-native.coax`: every slot of every class's bar at levels 1 to 6 executes.
- `class-kit.coax`, `character-appearance.coax`, `everglade2-create.coax`, `fivebar-test.coax`,
  `everglade-wire.coax`, `everglade2-net.coax`, the `13046` checks in `everglade2.coax`, and the
  Everglade II room checks, including `pair-e2-levels`.
- Wasm (`wasm.log`): Everglade II is 660,903 bytes. Its budget goes from 660,000 to 680,000 for
  the level 1 to 6 bars, the LEVEL row, the room's word on the bar and the fades. fivelab is
  2,583,227 bytes, under its budget.

## Limits

- **Local page server.** It needed `realms/everglade2/assets/everglade2.vpk3`
  (`aethyr/tools/page-pack.coax`) and the `host-*.wasm` reactors (`coax/web/build-*.coax`)
  built in this tree. The other site files came from a copy of the main checkout's build
  directory.
- **Monsters, Reactions and Opportunity Attacks.** Everglade II has no hostile creature, so
  no Reaction or Opportunity Attack can trigger, even though the shared rules already have
  both. Adding monsters waits on the owner's social-profile decision.
- **Counts under unlimited presses.** Everglade II's declared world is unlimited presses
  (`RS_EVERGLADE_WORLD`), so each press refills uses. A count shows the room's state just
  after the cast, and it mostly reads max or max - 1.
