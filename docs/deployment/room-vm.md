# The room VM

Issues: [#13174](https://github.com/OpenAgentsInc/openagents/issues/13174)
(the VM), [#13179](https://github.com/OpenAgentsInc/openagents/issues/13179)
(the shard directory), part of
[#13164](https://github.com/OpenAgentsInc/openagents/issues/13164)
(WebTransport for the room).
Code: `coax/web/room/vm-deploy.sh`, `coax/web/room/vm/`,
`coax/web/room/stage.sh`, `coax/web/room/compile.sh`.

The Everglade room server (`coax/web/room`, the `room` binary) runs on one
GCE VM. Cloud Run can't take UDP, and WebTransport needs it. Cloud Run's
`oa-room` services stay deployed, untouched, as the rollback.

## Roll back

One command sends production's rooms back to Cloud Run:

```sh
coax/web/room/vm-deploy.sh switch run
```

It points the url map's `/api/room/` rules for openagents.com and
everglade.world back at `oa-room` and `oa-room-2` to `oa-room-6`, then
drains each VM shard (its pages hear "restarting" and sit again, now on
Cloud Run) and stops it, so its directory reports end. Cloud Run's services
scale up from zero on the first socket.

`coax/web/room/vm-deploy.sh switch vm` moves them back to the VM.

A bad room release on the VM rolls back on the VM:

```sh
coax/web/room/vm-deploy.sh rollback
```

## Topology

| Piece | Name |
| --- | --- |
| VM | `oa-room-1`, e2-standard-2, Debian 12, us-central1-a, labels `app=oa-room`, `owner=openagents`, tag `oa-room`, no service account |
| Address | `oa-room-1-ip`, static, 34.41.219.6 |
| DNS (Cloudflare, not proxied) | `room.openagents.com` and `room-staging.openagents.com`, A to 34.41.219.6 |
| Firewall | `oa-room-public` (TCP and UDP 443 from anywhere), `oa-room-lb` (TCP 8081-8090 from the load balancer's ranges), `oa-room-iap-ssh` (TCP 22 from IAP only), `oa-room-deny-other-ingress` (everything else) |
| Units | `oa-room@s1` to `oa-room@s6` (production's shards 1 to 6, ports 8081 to 8086), `oa-room@staging` (8090) |
| Load balancer | instance group `oa-room-vm-ig` (named ports `s1`..`s6`, `staging`), health check `oa-room-vm-hc` (`/health` on the serving port), backend services `oa-room-vm-s1`..`s6` and `oa-room-vm-staging`, one-day timeout |
| URL map | `coaxial-openagents-web-staging`: the `production` and `everglade` path matchers' `/api/room/sK/` rules go to `oa-room-vm-sK`; the `staging` matcher's go to `oa-room-vm-staging` |

```
page ── wss://openagents.com/api/room/sK/… ── HTTPS load balancer ── oa-room-vm-sK ── oa-room-1:808K (oa-room@sK)
       wss://room.openagents.com/api/room/sK/… ─────────── Caddy :443 ─────────────┘
```

The pages reach the rooms through the load balancer, as they did on Cloud
Run. Caddy on the VM answers `room.openagents.com` and
`room-staging.openagents.com` on TCP 443 directly, routes `/api/room/sK/` to
shard K, and holds the certificates the room's WebTransport (UDP 443) will
use.

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
