# 101. The Everglade room

> **Status: partly implemented.** Everglade runs as server-authoritative
> rooms of 16 seats, 16 rooms to an oa-room process, on 6 Cloud Run shards
> placed by the API's instance directory (#12705, #12717), with the Cultist
> Raid of up to 60 standing raiders on the server (#12740). Placement holds
> 2 rooms a shard today, 192 players in all. Replication, budgets, interest,
> the version check, reconnect and vitals have landed (chapters 91, 92, 97,
> 98 and 99). Offline starts on the worker by default (chapter 95); drain
> and readiness are implemented (chapter 98). Shared command and
> replication interfaces still need extraction (#12845).

This chapter puts the part together: the full Everglade room, built from
chapters 86 to 100, at the scale it runs.

## What this chapter teaches

- How the chapters of this part compose into one room.
- Sizing: seats, rooms, processes and shards, and why at these numbers.
- A world event as a crowd test: 16 players and 60 raiders in one square.

## The room

<!-- source: everglade/server/seat.coax room-size -->
```jai
EG_ROOMS :: 4;                  // simulated rooms at once (64 players): the directory gives a shard 2 (verse.room_shards.rooms_cap), a 150-seat zone 1 (docs/decisions/0007)
EG_SEATS :: 16;
EG_HZ :: 30;
```

| Layer | Chapter | Everglade |
| --- | --- | --- |
| Server | 86 | oa-room, one thread, 4 rooms at 30 Hz |
| Shared code | 87 | `everglade-step.coax`, the SRD rules, the `everglade` world document |
| Seats | 88 | HMAC seat tickets from the API, 16 a room |
| Intents | 89 | `I`, `C`, `E`, `R`; refusals with reasons |
| Prediction | 90 | your movement only; camera from your controls |
| Replication | 91, 92 | one row for every entity, deltas against what the page heard, 1,000 bytes a tick a seat (#12783) |
| Drawing others | 93 | one adaptive buffer, 80 to 300 ms |
| Effects | 94 | lasting abilities as entities; one status state |
| Offline | 95 | local worker first by default; a remote room takes over after seating; `?offline=worker` keeps it local |
| Combat rules | 96 | range and sight at start and release, a 0.5 s command age, heals before harm |
| Interest | 97 | 32 m cells, in at 110 m and out past 130 m; blocked and hidden entities never sent (#12789) |
| Live operations | 98 | version check, reconnect with a 15 s grace hold, signed drain and receipt-gated shutdown |
| Measurement | 99 | the action path's vitals every 10 s on `/admin/rooms` (#12791) |
| Proof | 100 | the drift check; bots that fight |

## Sizing

**Seats.** 16 a room keeps a full room's mutual visibility affordable: 16
players see each other at 30 Hz (chapter 92's estimate).

**Rooms per process.** One oa-room process holds 4 rooms, 64 players. It held
16 until #13275 (commit 6a846432d2: 16 full rooms in one process kept the
acknowledgement's p95 under 40 ms with 10.9 s of tick work a minute), but the
directory never gives a shard more than its `rooms_cap`, which is 2, and a
zone of 150 players gets a process to itself (decision 0007). In production, 2
full rooms ran at a tick p95 of 3.9 ms (launch runbook).

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
coax/web/tool.sh coax/web/room/local.coax
coax/web/tool.sh coax/web/room/room-proof.coax raid coax/web/room/bin/room 4 120
```

`local.coax` runs a room server and the page against one world document on
this machine. `room-proof.coax raid` starts a room server, joins 4 bots from
`raid-check.coax`, starts the raid and lets it run 120 s; it exits 0 when the
raid ran.

## What is left

The remaining extraction in #12845 includes shared commands, accepted
results and replication, additional realm profiles and the adoption
inventory. Reconnect, party contracts, admission and drain have code;
they are not future work merely because an older gap table lists them.
Qualify every later persistence, ownership and scale claim with its own
consumer evidence.

## Acceptance checks

- The drift check holds at every profile (chapter 100).
- A raid with 16 seats and 60 raiders holds the tick and bandwidth bounds of
  chapters 92 and 99.
- A deploy during a raid loses no seat's HP, XP or place (#12797).

## Dependencies

Chapters 96 to 100.
