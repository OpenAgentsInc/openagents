History: see everglade/ARCHITECTURE.md.

# WebTransport datagrams against the WebSocket profile

Date: 2026-11-05. Status: evaluation, decided: keep WebSocket (no-go for now).
Issue: #12858, under #12833. Background: the transport rows of the
[MMO gap analysis](2026-11-04-aethyr-verse-mmo-gap-analysis.md) (G3, G10) and
the [book appendix](../../metaverse-engine/book/appendix-mmo-gap-analysis.md).

This evaluation measures how much a lost TCP packet stalls an Everglade room's
state stream, models what unreliable datagrams would change on the same
network, lists what browsers and our hosting support, and records the
decision with the conditions that would reverse it.

## Summary

- On a good or ordinary network (up to 100 ms round trip with 2% loss), TCP's
  stalls fit inside the page's jitter buffer. The longest gap between state
  frames measured was 263 ms, below the 350 ms the page hides.
- On the drift check's hard profile (250 ms round trip, 30 ms jitter, 2%
  loss), TCP stalls exceed the buffer: state frames arrived up to 560 ms
  apart, and other actors freeze for a moment about once every two seconds.
  Datagrams remove those freezes in the model (zero freezes at 2% and 5%
  independent loss).
- 92% of browsers support WebTransport, including Safari 26.4. Our room
  hosting doesn't: rooms run on Cloud Run behind Google's Application Load
  Balancer, which carries no UDP to the service, and Coax has no QUIC or
  HTTP/3 stack.
- Decision: keep WebSocket. Reconsider when all three go conditions below hold.

## What was measured

### The WebSocket profile, measured

`coax/web/room/room-proof.coax transport` runs a local room server
(`coax/web/room/bin/room --dev`), the shaping proxy
(`coax/web/room/net-proxy.coax`) in front of it, and four `load.coax`
everglade players through the proxy for 60 seconds per profile. Each player
walks at 30 inputs a second, casts every 5 seconds and pings once a second.
The proxy delays every read by half the round trip, plus or minus half the
jitter, and holds a lost read for the larger of 200 ms and twice the round
trip, in order, as TCP does.

`load.coax` now reports the state stream's arrival gaps, so a stall is
visible directly: `d_gap_p99_ms` and `d_gap_max_ms` are the 99th percentile
and longest gap between new state frames (33 ms at the room's 30 Hz), and
`d_stalls_100ms` counts gaps of 100 ms or more, all players together.

```sh
coax/web/room/build.sh
coax/web/tool.sh coax/web/room/room-proof.coax transport "$PWD/coax/web/room/bin/room" 8981 4 60 0/0/0 100/30/20 250/30/20 250/30/50
```

Results on 2026-11-05, Apple Silicon Mac, room and players on one machine:

| Network (round trip / jitter / loss) | State gap p99 | State gap max | Gaps of 100 ms or more | Input-to-ACK p50 / p99 | Ping p50 / p99 |
| --- | --- | --- | --- | --- | --- |
| Local | 35 ms | 40 ms | 0 | 16 / 36 ms | 1 / 3 ms |
| 100 ms / 30 ms / 2% | 228 ms | 263 ms | 116 | 119 / 316 ms | 107 / 321 ms |
| 250 ms / 30 ms / 2% | 527 ms | 560 ms | 134 | 310 / 921 ms | 275 / 1,139 ms |
| 250 ms / 30 ms / 5% | 533 ms | 561 ms | 312 | 505 / 1,145 ms | 609 / 1,194 ms |

Every run kept all four sockets open and dropped no state ticks
(`d_dropped` 0); the losses show as late frames, not missing ones. Under
shaped networks `load.coax` prints `fails` because a few players' last ACK is
still in flight when the run stops (`ack_behind`); that check is meant for
localhost. The proxy holds every lost read for the same 500 ms at 250 ms, so
its maximum gap barely moves from 2% to 5% loss; the count of stalls is what
grows (134 to 312). The ping's
99th percentile at 250 ms is four times its median: that tail is
head-of-line blocking, because the ping answer waits behind the held bytes.

### Datagrams on the same network, modelled

A real WebTransport prototype needs a QUIC server, which we don't have (see
[Hosting](#hosting-and-browsers)). Instead, `coax/web/room/transport-model.coax`
generates one packet trace per network (delays, jitter and losses, with
Gilbert-Elliott bursts where asked) and delivers it two ways:

- TCP: bytes in order, so a frame behind a lost packet waits for its resend.
  The resend comes at the earlier of a fast retransmit (three later packets
  heard, then a round trip) or the timeout (the round trip plus 200 ms,
  doubled on each lost resend), as Linux does.
- Datagrams: a lost frame is gone; the next frame replaces it. Inputs going
  up carry the last three inputs in each datagram.

It measures, every millisecond, the age of the newest state the page holds,
and counts a freeze whenever that age passes the one-way delay plus 300 ms
(the deepest jitter buffer the page uses, `NET_HIGH` in
`apps/openagents-web/everglade-net.coax`). Its checks prove that with no loss
the two transports deliver identically, and that on any trace the datagram
page's newest state is never older than the TCP page's.

```sh
coax/web/tool.sh coax/web/room/transport-model.coax RTT_MS JITTER_MS LOSS_PERMILLE [BURST] [PACKETS] [SECONDS] [SEED]
```

Ten simulated minutes per row, three seeds each (worst seed for ages, mean
for freezes). "Packets" is packets per state frame: 1 for a small room, 4
for a raid's 1.1 Mbit/s.

| Network | Burst | Packets | TCP state age p99 / max | TCP freezes a minute | Datagram state age p99 / max | Datagram freezes a minute | Input p99, TCP / datagram |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 50 / 10 / 0.5% | 1 | 1 | 154 / 710 ms | 0 | 61 / 96 ms | 0 | 143 / 30 ms |
| 100 / 30 / 0.5% | 1 | 1 | 230 / 898 ms | 0 | 95 / 131 ms | 0 | 218 / 65 ms |
| 100 / 30 / 2% | 1 | 1 | 381 / 896 ms | 0 | 102 / 164 ms | 0 | 464 / 86 ms |
| 100 / 30 / 2% | 1 | 4 | 647 / 842 ms | 3 | 126 / 231 ms | 0 | 276 / 85 ms |
| 250 / 30 / 2% | 1 | 1 | 906 / 1,421 ms | 38 | 177 / 239 ms | 0 | 989 / 161 ms |
| 250 / 30 / 2% | 1 | 4 | 1,172 / 1,367 ms | 104 | 201 / 306 ms | 0 | 794 / 160 ms |
| 250 / 30 / 2% | 4 | 1 | 825 / 1,429 ms | 9 | 266 / 800 ms | 1 | 906 / 140 ms |
| 250 / 30 / 2% | 4 | 4 | 1,156 / 1,363 ms | 48 | 218 / 370 ms | 0 | 666 / 144 ms |
| 250 / 30 / 5% | 1 | 1 | 1,397 / 3,222 ms | 85 | 189 / 288 ms | 0 | 1,699 / 170 ms |
| 250 / 30 / 5% | 4 | 4 | 2,679 / 3,165 ms | 96 | 252 / 498 ms | 0 | 2,596 / 173 ms |

The model agrees with the measurement where they overlap: at 250 ms and 2%
the model's TCP state age p99 is 906 ms with a 125 ms one-way delay, and the
measured gap p99 is 527 ms on top of the same delay. The proxy holds a lost
read for 500 ms there; the model's fast retransmit takes about 350 ms but
also stacks a second loss behind the first.

Bursty loss costs datagrams inputs: with bursts of four packets, 1% to 3% of
inputs were lost even with three-fold redundancy. A datagram profile needs
input redundancy at least as long as the expected burst, or an acknowledged
input window.

### What was not measured

- Bytes and CPU on a datagram path: there is no prototype. Datagram framing
  adds QUIC's header (about 30 bytes a packet against TCP and WebSocket's
  about 46), and delta compression against acknowledged baselines carries
  over unchanged.
- Reconnect and compatibility of a datagram path, for the same reason.
- Phones on real cellular links. Both tables use shaped localhost links.

## Hosting and browsers

- Browsers: WebTransport is in Chrome 97, Edge 98, Firefox 114, and Safari
  26.4 on macOS and iOS: 92% of global usage
  ([caniuse](https://caniuse.com/webtransport)). The other 8% need the
  WebSocket fallback.
- Hosting: rooms run on Cloud Run (`coax/web/room/deploy.sh`), reached through
  the global Application Load Balancer. That balancer speaks HTTP/3 to
  browsers but forwards HTTP/1.1 or HTTP/2 to the service; it carries no
  WebTransport sessions or UDP to Cloud Run. Datagrams need the rooms on
  Compute Engine VMs behind a passthrough Network Load Balancer, with our own
  TLS certificates on the VMs.
- Code: Coax has TCP, TLS and WebSocket (`coax/lib/net/`), and no QUIC,
  HTTP/3 or WebTransport. A QUIC stack (TLS 1.3 handshake inside QUIC,
  loss recovery, congestion control and pacing per RFC 9002, datagrams per
  RFC 9221) is a large new authority to write and qualify.
- Congestion: datagrams share QUIC's congestion controller. A sender must
  still bound its state rate; replaceable state must be dropped, not queued,
  when the window is full.

## Decision

No-go: keep WebSocket over TCP as the room transport.

The stalls are real on the hard profile, but the hard profile is a stress
test, not today's supported network: the drift check requires that a page on
it never drifts from the offline game, which holds; it doesn't promise smooth
motion of other actors. The cost of the change (new hosting, a QUIC stack, a
second transport kept compatible with the first) is large against a benefit
that only players on long, lossy links see.

Go when all of these hold:

1. The product supports players at 250 ms round trip with 2% loss as a
   first-class profile, with a stated freeze budget (for example, no more
   than one freeze a minute), and the WebSocket path measures above it.
2. Rooms run where UDP reaches them (Compute Engine or equivalent), for
   reasons of their own or as part of this change.
3. A Coax QUIC and WebTransport stack is planned as its own qualified work,
   with WebSocket kept as the fallback for browsers without WebTransport.

Until then, cheaper steps reduce the cost of a stall on WebSocket:

- Newest-state replication and age-bounded send queues (#12837), so a stall
  delivers one fresh frame rather than a backlog.
- An adaptive jitter buffer that may go deeper than 300 ms on a measured
  lossy link, trading latency for fewer freezes (`NET_HIGH`).
- Keeping delivery classes (replaceable state; reliable commands and
  lifecycle; durable receipts) separate in the session contract (#12845), so
  a later transport can carry each class differently without changing
  gameplay.
