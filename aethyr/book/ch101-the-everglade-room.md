# 101. The Everglade room

> **Status: partly implemented.** Everglade runs as server-authoritative
> rooms of 16 seats, 16 rooms to an oa-room process, on 6 Cloud Run shards
> placed by the API's instance directory (#12705, #12717), with the Cultist
> Raid of up to 60 standing raiders on the server (#12740). Placement holds
> 2 rooms a shard today, 192 players in all. The pieces still open are
> chapters 91, 92 and 95 to 98, and the drift check holding (chapter 100).

This chapter puts the part together: the full Everglade room, built from
chapters 86 to 100, at the scale it runs.

## What this chapter teaches

- How the chapters of this part compose into one room.
- Sizing: seats, rooms, processes and shards, and why at these numbers.
- A world event as a crowd test: 16 players and 60 raiders in one square.

## The room

<!-- source: coax/web/room/room-everglade.coax room-size -->
```jai
EG_ROOMS :: 16;                 // simulated rooms at once (256 players)
EG_SEATS :: 16;
EG_HZ :: 30;
```

| Layer | Chapter | Everglade |
| --- | --- | --- |
| Server | 86 | oa-room, one thread, 16 rooms at 30 Hz |
| Shared code | 87 | `everglade-step.coax`, the SRD rules, the `everglade` world document |
| Seats | 88 | HMAC seat tickets from the API, 16 a room |
| Intents | 89 | `I`, `C`, `E`, `R`; refusals with reasons |
| Prediction | 90 | your movement only; camera from your controls |
| Replication | 91, 92 | per-kind entries today; one record with a budget next (#12783) |
| Drawing others | 93 | one adaptive buffer, 80 to 300 ms |
| Effects | 94 | lasting abilities as entities; one status state |
| Offline | 95 | the page's own game today; a local worker host next (#12784) |
| Combat rules | 96 | cast bar, cooldown and refusals; written rules next (#12790) |
| Interest | 97 | one 120 m radius; hysteresis and privacy next (#12789) |
| Live operations | 98 | rejoin after silence; version check, drain and reconnect next |
| Measurement | 99 | tick percentiles and time by part |
| Proof | 100 | the drift check; bots that fight |

## Sizing

**Seats.** 16 a room keeps a full room's mutual visibility affordable: 16
players see each other at 30 Hz (chapter 92's estimate).

**Rooms per process.** One oa-room process holds 16 rooms, 256 players
(commit 6a846432d2). Measured locally, 16 full rooms in one process kept the
acknowledgement's p95 under 40 ms with 10.9 s of tick work a minute; in
production, 2 full rooms ran at a tick p95 of 3.9 ms (launch runbook).

**Shards.** Six Cloud Run services, each one process (`--max-instances 1`),
so a room has one owner, except while a deploy overlaps an old and a new
revision (#12797). The directory places at most 2 rooms on a shard, for 192
players; a migration can raise it to 3.

**Placement.** A player returning to their instance keeps it. Otherwise the
directory picks the lowest instance with a free seat, holds the seat for
20 s, and signs a ticket for that shard. `?with=PID` asks for a friend's
room while it has a seat.

## The raid as a crowd

The Cultist Raid puts up to `EV_STANDING_MAX` 60 raiders on the server at
once, with Darian Odiodei and 3 ward stones, 64 places in all. A raid is the
dense case the research doc warns about (section 11): area spells, many
targets, deaths and XP for every seat that hurt a raider. Each tick caps the
work it can make, and counts what it drops:

- 96 events and 6,144 event bytes a tick;
- 32 raider casts and 32 downs a step;
- 16 player casts in flight.

## Run it

```shell
coax/web/room/local.sh
coax/web/room/raid-proof.sh coax/web/room/bin/room 4 120
```

`local.sh` runs a room server and the page against one world document on
this machine. `raid-proof.sh` starts a room server, joins 4 bots from
`raid-check.coax`, starts the raid and lets it run 120 s; it exits 0 when the
raid ran.

## What is left

The rest of this part's design, and the gaps in [appendix
A](appendix-mmo-gap-analysis.md): reconnect (#12792), durable saves
(#12793, #12794), group seats (#12795), admission (#12796) and drain
(#12797).

## Acceptance checks

- The drift check holds at every profile (chapter 100).
- A raid with 16 seats and 60 raiders holds the tick and bandwidth bounds of
  chapters 92 and 99.
- A deploy during a raid loses no seat's HP, XP or place (#12797).

## Dependencies

Chapters 96 to 100.
