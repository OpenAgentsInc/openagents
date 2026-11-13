# Protocol bots and the nightly soak

This page describes the protocol bots, the nightly two-hour soak of the full
fleet, the capacity record each night leaves, and what makes a night fail
(#13296). The verdict lives in `coax/web/room/soak-policy.coax`. When a rule
here changes, change it there in the same commit.

## The bots

`coax/web/room/bots.coax` plays the room's wire with no browser. A bot uses the
tools' one wire client (`room-socket.coax`: the socket, the seat ticket, the
page's mirror of the state) and is played by the encounter simulator's
scripted player (`script-player.coax`). Each bot does what a page does:

| It | How |
| --- | --- |
| Sits | A guest player and a seat ticket from the API's directory, the shard's own endpoint, its build (`P`), then its hello |
| Moves | An input 30 times a second: down the path past the wolf packs and back, or at the nearest monster in view |
| Fights | The scripted player's choice each time a cast can go: a heal when hurt, its buffs once a fight, then harm at the nearest monster |
| Earns | The XP and levels the room grants it. Loot has no wire message yet (#13291); the bots count XP in its place |
| Reconnects | Every 15 minutes: it closes its socket, asks the directory again and sits where it stood |
| Travels | Every 10 minutes: it asks for the realm's next instance and joins it when the room says go |

A process holds 16 sockets, so `bots.coax run` starts one process for every 16
bots. One build host runs hundreds. The bots' realm is `pair-e2-soak`, a test
realm: no rooms list or usage count includes it.

```sh
coax/web/tool.sh coax/web/room/bots.coax run https://staging.openagents.com 32 600      # through the API, as pages do
coax/web/tool.sh coax/web/room/bots.coax run ws://localhost:8830 32 60 --direct         # a local room, tickets self-signed
```

The scripted player's bars come from `coax/web/room/bot-bars.coax`, which
`encounter-sim.coax --bars` writes from the rules. The room check fails when
the file is stale (`encounter-sim.coax --check-bars`).

## The soak

`coax/web/room/soak.coax run` starts the bots and watches the room while they
play. Every 5 seconds it reads the room's last 10-second window (tick p50, p95
and p99, resident memory, the oldest unsaved checkpoint, refused frames) and
times the API's health. The bots report their ticket times, input-to-ACK times
and bytes heard.

A run fails when any of these holds:

| Reason | Rule |
| --- | --- |
| `empty` | Nobody was seated |
| `slow-tick` | 2 or more windows had a tick p99 over 10 ms |
| `leak` | The room's resident memory grew 64 MB or more between the run's second and last quarters |
| `checkpoint` | 2 or more windows had a checkpoint unsaved for more than 15 s |
| `ticket` | The tickets' p95 was over 300 ms |
| `version` | The room refused a bot's build, so the run measured nothing |
| `blind` | Fewer than half the room's windows could be read |
| `lost` | At the end, fewer than nine in ten of the most ever seated were still seated |
| `astray` | A creature stood off the navigation cells' open ground, or was found inside a collider. The room counts both ([navigation](../verse/navigation.md#what-a-room-counts)) |

The targets are the service levels' ([service levels](service-levels.md)).

### The check of the check

`soak.coax self-check ROOM_BIN` plays a local room three times for a minute:
as built, with a seeded slow tick, and with a seeded leak. The first must hold
and the other two must fail for their own reason. The nightly job runs it
before the soak, so a soak that can't fail never reports a good night. The
seeded faults (`OA_ROOM_FAULT_TICK_US`, `OA_ROOM_FAULT_LEAK_KB`) work only in a
`--dev` room.

## Each night

The green-main watcher (`coax/remote/after.sh`, `soak_of`) runs
`coax/remote/check.sh soak` once a day on a head that went green:

1. It builds the room and runs the self-check.
2. It plays staging's rooms with 32 bots for two hours, through the staging API
   and its real database. Staging runs one shard, and the directory gives a
   shard two rooms of 16: 32 players is that shard full.
3. It keeps the night's record in `releases/after/soak/DAY.txt` under the
   pipeline's folder, and posts it to the production and staging APIs.
4. When the run fails, it opens a GitHub issue ("Nightly soak: staging's fleet
   does not hold") with the record and the log's end. While that issue is
   open, each failing night comments on it.

The soak holds staging's lock for its run (`coax/web/room/vm-deploy.sh lock`,
#13398). The watcher takes the lock as `soak COMMIT` before it starts the
bots, and releases it when the run ends.

- If a release holds staging, the soak does not start. The next green head
  tries again.
- A soak's lock gives way. A release or a deploy that asks for staging takes it
  from the soak. The watcher checks the lock once a minute. When the lock is no
  longer the soak's, the watcher stops the run and does not judge it, and no
  issue is opened.
- A second soak cannot take the lock from the first.
- `OA_AFTER_SOAK_LOCK=0` runs the soak without the lock.

Staging's alerts mail nobody, so a soak never emails the owner.

| Setting | Default | Meaning |
| --- | --- | --- |
| `OA_AFTER_SOAK` | 1 | 0 turns the nightly soak off |
| `OA_AFTER_SOAK_SECONDS` | 7200 | How long the fleet plays |
| `OA_AFTER_SOAK_PLAYERS` | 32 | The bots sent: staging's one shard, full. The record's `seated` is what it held |
| `OA_AFTER_SOAK_GAP` | 86400 | Seconds between soaks |
| `OA_SOAK_PUBLISH` | both APIs | The origins the record is posted to; 0 posts none |

## The capacity record

One night is one record: capacity record v1, written and checked by
`coax/web/api/soak-record.coax` (profile
`proto/verse/capacity-record-v1.json`). It is text, one `KEY VALUE` line each:
the day, the environment, the bots' commit, the seconds, the players sent and
seated, the worst window's tick p50, p95 and p99, the slow windows, refused
frames, checkpoint age, ticket and API p95, input-to-ACK p95, memory early and
late, bytes a player a second, rejoins, kills, and the verdict.

| Route | What |
| --- | --- |
| `POST /api/playtest/capacity-record` | Keeps a record. Signed as a room's `capacity` write ([room writes](../protocol/room-write-v1.md)). One record a day and environment; a later run of the day replaces the earlier |
| `GET /api/playtest/capacity` | The newest record as it was kept. `?day=YYYY-MM-DD` answers that day's |

The status page shows the newest record under the service levels: the players
held, the day and the worst tick p99, or the reasons a night failed.

## What a night does not prove

Staging has one room shard, on the VM production's six shards share. A night
proves one shard full, for two hours, against the real staging database. It
does not prove six shards at once: that needs six staging units, which is the
second room VM's question (#13295). A bot the directory places on a shard
staging does not run asks three times and is then out for the run
(`unplaced` in the bots' lines).

Each night's bots are new guest players in the staging database (32 rows a
night). Their realm is a test realm, so no usage count includes them.

## Cost

| Item | Cost a night |
| --- | --- |
| One job slot of a build host (c3d-highcpu-30 spot, $0.275 an hour for three slots) for about 2 hours 10 minutes | $0.20 when the host is shared, $0.60 when the soak keeps a host up alone |
| Egress from the room VM to the build host (about 15 KB a second a bot, same region) | about $0.05 |
| The room VM, the staging API and the database | Already running; no new resource |

The nightly soak adds no resource with a monthly bill. It runs inside the build
pool's $50-a-day cap (`coax/remote/remote.sh`).
