# Everglade's creatures

This page describes how an Everglade room gets its creatures (#13293). The
runtime is `everglade/server/creatures.coax`. The data is six tables in
`everglade/content/everglade.content`.

## The tables

| Table | Columns | What a row is |
| --- | --- | --- |
| `creature` | `id name kind xp` | A creature's name, its look on the page (`kind`: 0 wolf, 1 goblin, 2 guard, 3 speaker) and the XP it gives. |
| `template` | `id creature block style notice leash run` | How the creature fights: its SRD stat block (the number in `rules/srd/monsters.coax`), its behavior, how far it notices a player and how far it follows one (centimetres), and its run (centimetres a second). |
| `spawn` | `id creature point count` | `count` of a creature at a point. One stands on the point. More stand 1.5 m round it. |
| `troop` | `id spawn group face respawn slot` | The spawn's place in the room: its group (spawns of one group fight as one), the way it faces at home (degrees, 0 north), the seconds from a fall to the next one, and the number its first member carries on the wire. |
| `troop_beat` | `id troop point rest` | One point of a patrol's beat, and the seconds it stands there. The troop walks its points in ID order, then back. |
| `troop_phase` | `id troop hp after style run` | One phase of a boss script. See "Boss scripts". |

A room holds 12 monsters (`E2M_N`, the wire's `WIRE_MONS`) and one patrol. A
troop's `slot` is its first member's number, 0 to 11. A number that no troop
names never spawns. The patrol has no monster number: the pages hear of it
through the guard's own row and events.

## Behaviors

`style` names one of the behaviors in `creatures.coax` (`E2S_*`):

| Style | Name | What it does |
| --- | --- | --- |
| 0 | pack | Runs in, circles its quarry while its blow readies, and closes from its own side. |
| 1 | melee | Closes and strikes. |
| 2 | archer | Looses from where it stands. Steps back from anyone who closes on it. |
| 3 | patrol | The path's guard. Friendly. Walks its beat and fights whoever harms it. See "The patrol". A room has one. |
| 4 | stands | A speaker, such as the Warden. The page draws it at its point (`everglade/client/speakers.coax`). It never fights. |

## The patrol

The path's guard is the room's creature of the patrol behavior (#13427). His
walk, his fight and his steering are the runtime's. `everglade/server/guard.coax`
only tells the pages about him.

- **Beat.** He walks the `troop_beat` points of his troop in ID order, then
  back. At a point with a `rest` he stands that long, turns, and walks on.
  With no beat he stands at his troop's point.
- **Threat.** He never harms first. A seat that harms him goes on his threat
  list with the Hit Points it took (a miss is 1). A monster that struck at a
  player within 12 m of him in the last ten seconds goes on it too. He fights
  the top of the list, as every creature does.
- **Steering.** He moves on the room's navigation cells through the same call
  as every creature (`e2mo_move`). Players are bodies to him: he goes round
  them and no step of his ends within 0.8 m of a player's middle.
- **Blows.** The rules' turn strikes (`egb_patrol_turn` in
  `everglade/server/battle.coax`): his Longsword in reach, his Longbow far
  off, a Dash to close, an Opportunity Attack on a seat that leaves his reach.
- **Letting go.** He drops a foe that goes past his template's `leash` from
  where the fight began, and drops everyone after 15 seconds with no blow
  either way. Then he walks to the nearest point of his beat.
- **Respawn.** His troop's `respawn` seconds after a fall, a new guard stands
  at the first point of his beat that no player is within 45 m of.
- **Sleep.** Like every creature, he steps only while his cell simulates (a
  player within 160 m).

## Boss scripts

A `troop_phase` row changes how a troop's member fights (#13427):

| Column | Meaning |
| --- | --- |
| `troop` | The troop whose members it scripts. |
| `hp` | The row holds while the creature is at this percent of its Hit Points or under. 0: not keyed on Hit Points. |
| `after` | The row holds once this many seconds have passed since the creature was roused. 0: not keyed on time. |
| `style` | The behavior it fights by while the row is in force: 0 pack, 1 melee, 2 archer. |
| `run` | Its run while the row is in force, in centimetres a second. 0 keeps the template's. |

The last row that holds, in ID order, is in force. With no row in force the
creature fights as its template says. A row of the archer style is passed
over for a stat block with no ranged attack. Going home and a new spawn clear
the script.

The camp's Ogre is the example (troop 13): a row at 100 percent makes it throw
Javelins from where it stands, and a row at 50 percent makes it close with its
Greatclub at 6.5 m/s. The check "a boss with two phases is rows" plays both
phases and a row keyed on time.

## Add a creature

To add a creature that uses a behavior above, add rows. Change no code.

1. Add a `creature` row.
2. Add a `template` row with its stat block and style.
3. Add `loot` rows that add up to 100.
4. Add a `point`, a `spawn` and a `troop` on free numbers. Add `troop_phase` rows for a boss.
5. Raise the revision in the file's first line.
6. Run the checks:

```sh
coax/web/tool.sh everglade/model/tools/content-check.coax everglade/content/everglade.content --carry
coax/web/tool.sh everglade/model/tools/zone-cells.coax --check everglade/world/zone-cells.coax everglade/content/everglade.content
coax/bin/coax test --library coax/web/room/room.coax --steps 4000000000 --only "e2 monsters"
```

The check "a new creature that uses a behavior the room has is rows" adds a
pack of Black Bears this way.

A room reads the tables once, at start. It reads the tables it is built with
(`everglade/content/shipped.coax`) and then the host's file, when that file has
the `template` and `troop` tables.

## Threat

Each monster keeps a threat list of up to eight seats. The monster fights the
seat with the most threat that it can still reach.

- Harm adds the Hit Points taken. A miss adds 1.
- Healing a seat that is on a monster's list adds half the Hit Points healed to
  the healer.
- Being noticed, or being a packmate's quarry, adds 1.
- A taunt (`e2_monster_taunted`) puts the taunter on top. A Barbarian's
  Reckless Attack is a taunt on the creature it is pressed at
  (`everglade/server/casts.coax`).
- Another seat takes a monster off its quarry only with 110 percent of the
  quarry's threat.
- A monster that goes home clears its list.

Seats that harmed a monster, or healed against it, share its XP, its drop and
its quest credit.

## What is still code

- The behaviors are code. A creature that needs a new behavior needs a new
  style.
- A phase switches a behavior and a run. It doesn't switch an ability: a stat
  block's attacks are picked by the behavior (its melee attack, or its ranged
  one).
- A taunt is keyed on the ability's kind in `everglade/server/casts.coax`. The
  `ability` table has `id name` only, so it can't be a column yet.
- A room holds 12 monsters. The count is the creature frames' (`EGC_MONS`,
  `WIRE_MONS` in `everglade/shared/everglade-wire.coax`) and the content
  registry's 12 `CT_SPAWN` keys. Raising it is a wire change and a content
  registry change.
- The Ogre is drawn with the goblin's look (`kind` 1). It has no model of its
  own.
