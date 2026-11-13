# Everglade's creatures

This page describes how an Everglade room gets its creatures (#13293). The
runtime is `everglade/server/creatures.coax`. The data is four tables in
`everglade/content/everglade.content`.

## The tables

| Table | Columns | What a row is |
| --- | --- | --- |
| `creature` | `id name kind xp` | A creature's name, its look on the page (`kind`: 0 wolf, 1 goblin, 2 guard, 3 speaker) and the XP it gives. |
| `template` | `id creature block style notice leash run` | How the creature fights: its SRD stat block (the number in `rules/srd/monsters.coax`), its behavior, how far it notices a player and how far it follows one (centimetres), and its run (centimetres a second). |
| `spawn` | `id creature point count` | `count` of a creature at a point. One stands on the point. More stand 1.5 m round it. |
| `troop` | `id spawn group face respawn slot` | The spawn's place in the room: its group (spawns of one group fight as one), the way it faces at home (degrees, 0 north), the seconds from a fall to the next one, and the number its first member carries on the wire. |

A room holds 12 monsters (`E2M_N`, the wire's `WIRE_MONS`). A troop's `slot`
is its first member's number, 0 to 11. Numbers never change meaning, so a
number that no troop names never spawns.

## Behaviors

`style` names one of the behaviors in `creatures.coax` (`E2S_*`):

| Style | Name | What it does |
| --- | --- | --- |
| 0 | pack | Runs in, circles its quarry while its blow readies, and closes from its own side. |
| 1 | melee | Closes and strikes. |
| 2 | archer | Looses from where it stands. Steps back from anyone who closes on it. |
| 3 | patrol | The path's guard (`everglade/server/guard.coax`). Friendly. Walks a beat from its point and fights whoever harms it. A room has one. |
| 4 | stands | A speaker, such as the Warden. The page draws it at its point (`everglade/client/speakers.coax`). It never fights. |

## Add a creature

To add a creature that uses a behavior above, add rows. Change no code.

1. Add a `creature` row.
2. Add a `template` row with its stat block and style.
3. Add `loot` rows that add up to 100.
4. Add a `point`, a `spawn` and a `troop` on free numbers.
5. Raise the revision in the file's first line.
6. Run the checks:

```sh
coax/web/tool.sh everglade/model/tools/content-check.coax everglade/content/everglade.content --carry
coax/web/tool.sh everglade/model/tools/zone-cells.coax --check everglade/world/zone-cells.coax everglade/content/everglade.content
coax/remote/remote.sh run -- coax/bin/coax test --library coax/web/room/room.coax --only "e2 monsters"
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

- The guard's behavior is its own file. Its creature, template, spawn and troop
  are rows. Its walk, its fight and its steering are `guard.coax`.
- The behaviors are code. A creature that needs a new behavior needs a new
  style.
- Boss scripts (phases keyed on health and time) are not built.
