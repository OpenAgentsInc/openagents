# The room VM

Issues: [#13174](https://github.com/OpenAgentsInc/openagents/issues/13174)
(the VM), [#13179](https://github.com/OpenAgentsInc/openagents/issues/13179)
(the shard directory), part of
[#13164](https://github.com/OpenAgentsInc/openagents/issues/13164)
(WebTransport for the room).
Code: `coax/web/room/vm-deploy.sh`, `coax/web/room/vm/`,
`coax/web/room/stage.sh`, `coax/web/room/compile.sh`,
`coax/web/room/failover-check.coax`.

The Everglade room server (`coax/web/room`, the `room` binary) runs on one
GCE VM. Cloud Run can't take UDP, and WebTransport needs it. Cloud Run's
`oa-room` services stay deployed, untouched, as the rollback.

## Roll back

One command sends production's rooms back to Cloud Run:

```sh
coax/web/room/vm-deploy.sh switch run
```

It points the url map's `/api/room/` rules for openagents.com and
everglade.world back at `oa-room` and `oa-room-2` to `oa-room-6`, restarts
each VM shard without its endpoint (so new seat tickets stop naming
room.openagents.com), then drains each one (its pages hear "restarting" and sit again, now on
Cloud Run) and stops it, so its directory reports end. Cloud Run's services
scale up from zero on the first socket.

`coax/web/room/vm-deploy.sh switch vm` moves them back to the VM, endpoints
included.

A bad room release on the VM rolls back on the VM:

```sh
coax/web/room/vm-deploy.sh rollback
```

## Measurements (#13174, 2026-11-11)

`coax/web/tool.sh everglade2-latency` (two headless pages, 16 trials or 8 an
input), against the VM's staging unit and Cloud Run's `oa-room`, on a clean
network and through a bad one (120 ms each way, 2% loss, ±30 ms jitter):

| Run | Round trip | Walk p50/p95 (ms) | Another player draws your arrow p50/p95 (ms) | Arrows confirmed | Others drawn behind |
| --- | --- | --- | --- | --- | --- |
| VM, clean | 32 ms | 13/15 | 83/118 | 16 of 16 | 100 ms |
| VM, bad network | 150 ms | 6/15 | 223/476 | 16 of 16 | 122 ms |
| Cloud Run, clean | 35 ms | 11/31 | none | 0 of 16 (its room refuses Ranger shots) | 300 ms |
| Cloud Run, bad network | 150 ms | 20/23 | none | 0 | 155 ms |

Server tick p95: VM 0.54 to 0.80 ms, Cloud Run 0.75 to 3.5 ms. Room input to
ack p95: VM 32 to 46 ms, Cloud Run 36 to 46 ms.

Production after the switch (openagents.com, page release 9f73ed6a58, two
headless pages in one room): round trip 34 and 37 ms, walk p50 19 ms and p95
23 ms, attack p50 16 ms, your own arrow in the air p50 15 ms; A loosed 16
arrows, the room confirmed all 16 (the last 0 mm from where A drew it), and B
drew A's arrow p50 93 ms and p95 204 ms after the button went up.

## Topology

| Piece | Name |
| --- | --- |
| VM | `oa-room-1`, e2-standard-2, Debian 12, us-central1-a, labels `app=oa-room`, `owner=openagents`, tag `oa-room`, no service account |
| Address | `oa-room-1-ip`, static, 34.41.219.6 |
| DNS (Cloudflare, not proxied) | `room.openagents.com` and `room-staging.openagents.com`, A to 34.41.219.6 |
| Firewall | `oa-room-public` (TCP and UDP 443 and 4431-4436 from anywhere), `oa-room-lb` (TCP 8081-8090 from the load balancer's ranges), `oa-room-iap-ssh` (TCP 22 from IAP only), `oa-room-deny-other-ingress` (everything else) |
| Units | `oa-room@s1` to `oa-room@s6` (production's shards 1 to 6, ports 8081 to 8086), `oa-room@staging` (8090) |
| Load balancer | instance group `oa-room-vm-ig` (named ports `s1`..`s6`, `staging`), health check `oa-room-vm-hc` (`/health` on the serving port), backend services `oa-room-vm-s1`..`s6` and `oa-room-vm-staging`, one-day timeout |
| URL map | `coaxial-openagents-web-staging`: the `production` and `everglade` path matchers' `/api/room/sK/` rules go to `oa-room-vm-sK`; the `staging` matcher's go to `oa-room-vm-staging` |

```
page ── wss://openagents.com/api/room/sK/… ── HTTPS load balancer ── oa-room-vm-sK ── oa-room-1:808K (oa-room@sK)
       wss://room.openagents.com:443K/api/room/sK/… ────── Caddy :443K ────────────┘
       https://room.openagents.com:443K/api/room/sK/… (WebTransport, UDP 443K) ── oa-room@sK
```

The directory hands pages each shard's own endpoint,
`room.openagents.com:443K` (#13187). A page opens WebTransport there on
UDP 443K, served by the shard itself, and falls back to the WebSocket on TCP
443K through Caddy. A page whose network blocks the port uses the load
balancer's path. Caddy answers `room.openagents.com` and
`room-staging.openagents.com` on TCP 443 too, routes `/api/room/sK/` to
shard K, and holds the certificates the rooms' WebTransport uses
(`vm/wt-cert.sh`).

The staging unit reports to the staging API (`OA_ROOM_API=https://staging.openagents.com`):
its directory, room control, vitals and checkpoints. staging.openagents.com's
pages play on it.

## Player state

Nothing a player owns lives on the VM. Each room posts its seats' HP, XP,
WANTED and checkpoints to the API (`room-host.c` `vitals_post` and
`checkpoint_post`: `/api/playtest/room-vitals` and
`/api/playtest/room-checkpoint-v2` on openagents.com, in
`openagents-production-pg`), as Cloud Run's rooms do, and reads them back
through the seat ticket. The production units leave `OA_ROOM_API` unset, so
they use the same store. Moving between Cloud Run and the VM, or restarting a
unit, loses nothing past the last checkpoint; a drain saves every seat first.

## Crashes

A room that crashes leaves a dump in `/var/lib/oa-room/UNIT/` (the unit's
`StateDirectory`), and its next start posts the crash report
([crash reports](crash-reports.md), #13298). `vm-deploy.sh crash staging`
crashes the staging unit on purpose, `vm-deploy.sh symbols` keeps a release's
debug file in the symbols bucket, and `vm-deploy.sh resolve BUILD OFFSET...`
resolves addresses from it.

## Deploy

```sh
coax/web/room/vm-deploy.sh deploy --staging   # build this checkout, run it as oa-room@staging
coax/web/room/vm-deploy.sh deploy             # promote staging's release to shards 1 to 6
```

`deploy --staging` turns the room's Coax into C on a build host
(`coax/remote/remote.sh run --no-fallback -- coax/web/room/stage.sh`, since
Coax to C runs every check the units keep), copies the C to the VM and
compiles it there with Debian bookworm's gcc, the steps Cloud Run's image
runs (`compile.sh`), into `/opt/oa-room/releases/NAME/room`. The start check
must hold. The staging unit restarts on it and must answer its health.

`deploy` promotes the release staging runs: `/opt/oa-room/current` moves to
it (the old one becomes `/opt/oa-room/previous`), and each running shard in
turn is drained, restarted and healthy before the next.

`coax/web/room/deploy.sh`, the Cloud Run deploy, sees from the url map when
production runs on the VM and runs these two steps instead.

## Drain

The same drain Cloud Run revisions get (#12797, `room-directory.coax`
`rm_drain_ask`): a POST to `/api/room/sK/drain` signed with the seat-ticket
key. The shard seats nobody new and tells the directory (its `d` line), waits
up to `OA_ROOM_DRAIN_WAIT` seconds (300) for its world events, tells every
page "restarting", saves every seat, and its health says "drain done". Then
systemd restarts it. `SIGTERM` alone (systemd's stop) still notes every seat
and flushes the outbox within 8 s.

```sh
coax/web/room/vm-deploy.sh drain 3 60   # drain shard 3, waiting up to 60 s
```

## TLS

Caddy (Debian's package) gets Let's Encrypt certificates for both names over
TLS-ALPN on TCP 443 and renews them itself; port 80 stays closed. The private
keys stay in `/var/lib/caddy` on the VM. Caddy serves HTTP/1.1 and HTTP/2
only, leaving UDP 443 to the room. The C library for TLS and QUIC is a stated
exception to "tools in Coax" (transport and crypto maturity); so is Caddy.

The seat-ticket key comes from Secret Manager (`oa-room-ticket-key`) over SSH
into `/etc/oa-room/secret.env` (root, 0600): `vm-deploy.sh secret`. The VM has
no service account.

## The shard directory (#13179)

Each VM unit names its own address in `OA_ROOM_ENDPOINT`
(`wss://room.openagents.com/api/room/sK/`; staging
`wss://room-staging.openagents.com/api/room/s1/`, written by `vm/setup.sh`).
Its directory report (`room-directory.coax`) then carries two more lines,
and comes every 5 s while the unit runs, rooms or none:

```
e wss://room.openagents.com/api/room/s2/    where its pages connect
h 4 640                                     its players and tick p95 in microseconds
```

The API (`rooms.c`) takes them out before `verse.room_report` reads the
report and keeps them on `verse.room_shards` (migration
`0140_room_shard_endpoints.sql`). A shard is:

| State | When | Placement |
| --- | --- | --- |
| live | reporting | new instances and reconnects; its seat tickets carry `"endpoint"` |
| draining | its `d` line (a drain, #12797) | none; its pages sit again elsewhere |
| down | it named an endpoint and hasn't reported for 15 s | none (`verse.room_fits` is false) |

After 120 s of silence its endpoint is forgotten and it counts as a shard
whose host says nothing, as Cloud Run's idle shards do, so a dead VM or a
`switch run` never strands placement. `GET /api/playtest/room-shards` lists
every shard's state, endpoint, players, tick p95 and instance count.

The page (`everglade/client/net.coax`) opens the ticket's endpoint when it names
one, and the load balancer's `/api/room/sK/` otherwise. When its socket goes
it asks the directory again, naming the realm, so a shard that died places
its players on a live one; the room restores each from the API's store.

### Failover check

```sh
coax/web/tool.sh coax/web/room/failover-check.coax https://staging.openagents.com \
  "coax/web/room/vm-deploy.sh kill staging"
```

Two players sit through the directory, walk, the staging unit gets
`SIGKILL` (systemd starts it again a second later), and each comes back on
its own: the page's first retry, a fresh seat ticket from the directory,
the socket again. It runs twice, once with the page's own position claim
(`A`) and once without. It holds when every player is back within 20 s each
time, and prints where each came back against where the room had it.

A place comes back from the API's checkpoint store (v4 tickets, the API's
`OA_ROOM_CHECKPOINTS=1`, `docs/protocol/character-checkpoint-v2.md`), on in
staging and production since 2026-11-11 (#13184). A killed room's players sit
again where its last accepted capture had them, with HP, XP, WANTED, spell
slots and the rest kept: at most one 5 s checkpoint interval of movement
behind. A drained room (a deploy, `switch run`) moves nobody, since it saves
and tells every page first. `--position` holds the place (1 m with `A`, 25 m
without); `--moving` keeps each player walking a circle right up to the kill
and holds both at 25 m, one interval of running (4.25 m/s) with slack.

Checkpoint writes: one per player per 5 s while anything about them changes,
plus one at each of their own events (placed, socket closed, leave, journey,
drain); an idle player writes nothing. Each is one upsert of a single
~1.1 KB row per character in `verse.character_checkpoints_v2`, committed
synchronously.

With checkpoints on, measured 2026-11-11 against staging with the room at
d916cb04aa:

| Run | Phase | Players back | Slowest | Largest error |
| --- | --- | --- | --- | --- |
| `--position --walk 20` | with `A` | 2 of 2 | 11.8 s | 0 mm |
| | without `A` | 2 of 2 | 10.9 s | 0 mm |
| `--position --moving --walk 23 --players 4` | with `A` | 4 of 4 | 19.9 s (the IAP kill took 10.2 s) | 0 mm |
| | without `A` | 4 of 4 | 12.1 s | 600 mm |

That last run wrote 53 checkpoints for four players over about a minute
each of play: 0.2 a second for a moving player, as designed.

Before checkpoints were on:

Measured 2026-11-11 against staging (the kill sent over IAP SSH, about 3 s
of each time below):

| Phase | Players back | Slowest, from the kill command |
| --- | --- | --- |
| with `A` | 2 of 2 | 10.5 s |
| without `A` | 2 of 2 | 10.7 s |

The directory's down state, checked by stopping the staging unit for 20 s:
22 s after its last report `room-shards` said `down`, and a new player's
ticket placed them on shard 2 with no endpoint. A second after it started
again it was `live`, its endpoint back.

On staging every `/api/room/sK/` rule goes to the one staging unit (shard 1),
so a ticket for another shard there finds no room; production has a unit per
shard.

### Adding a shard

1. A row: `INSERT INTO verse.room_shards (shard, rooms_cap) VALUES (7, 3);`
2. A unit: `/etc/oa-room/s7.env` (`PORT=8087`, `OA_ROOM_SHARD=7`,
   `OA_ROOM_ENDPOINT=wss://room.openagents.com:4437/api/room/s7/`, as `setup.sh`
   writes 1 to 6), a Caddy route for `/api/room/s7/*` and a site on :4437,
   UDP and TCP 4437 in `oa-room-public`, the shard in `vm/wt-cert.sh`, a named port `s7` on
   `oa-room-vm-ig`, a backend `oa-room-vm-s7`, and the url map's
   `/api/room/s7/` rules.

More shards than one VM holds means a second VM in its own instance group.
Once there is more than one shard per region on more than one VM, or the
rooms need more UDP ports than one address holds, put a passthrough network load balancer
(TCP and UDP 443) in front of each VM's address instead of pointing DNS at
it, as `coder-sync-1` does; the directory's endpoints then name each VM's own
host (`room-2.openagents.com`), so a page goes straight to its shard's VM.

## Status

```sh
coax/web/room/vm-deploy.sh status
```

It prints the releases, each unit's state, which backends the url map's room
rules use, and each shard's health.

## Cost

| Item | Per month |
| --- | --- |
| e2-standard-2, us-central1, on demand | about $49 |
| Static external IPv4, in use | about $3.65 |
| 20 GB pd-balanced | about $2 |
| Egress (room traffic, a few KB/s a player) | a few dollars |
| **Total** | **about $55-60** |

The backend services and health checks add no forwarding rule (they hang off
the existing load balancer). Cloud Run's rooms cost nothing while idle (min 0).
