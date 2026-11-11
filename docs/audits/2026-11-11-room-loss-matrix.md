# The room under loss: WebSocket against WebTransport (#13177)

Date: 2026-11-11. Every cell's JSON is in
`docs/audits/2026-11-11-room-loss-matrix.jsonl`, with a `phase` field.

## Summary

- WebTransport stays behind its switch: `E2N_WT_DEFAULT` is still false.
  The pass bar is the remote arrow's p95 under 2 RTT at 2% loss, 120 ms
  and ±30 ms, with no cell worse than WebSocket. It isn't met: WT's p95 is
  313 ms, against a 240 ms bar and WebSocket's 255 ms.
- The matrix found a real bug and 6133b87c69 fixed it. The room's QUIC
  writer sent each tick's state datagrams before its streams. Under loss
  the congestion window shrank, the datagrams filled it, and the reliable
  streams (outcomes, cues) starved. With the fix, 10% at 50 ms went from
  2 of 12 arrows seen to 12 of 12. 2% at 100 ms went from 0 of 12 to
  12 of 12, with p95 175 ms against WebSocket's 341 ms.
- Three WebTransport defects remain. They're tracked in #13187: the
  caster misses some of its own cast answers, keyframe storms at 150 ms,
  and silent sessions at 5% and 10% loss at 150 ms.
- Against the real VM (`room-staging.openagents.com`, UDP 443) a seat over
  WebTransport held for 15 s. State came at 29 ticks a second, with 1.1%
  of ticks missed, input round trip p50 71 ms and p95 196 ms, and
  2.2 KB/s in.

## How it was measured

- **Pages:** `coax/web/room/net-matrix.coax` runs `everglade2-latency`:
  two headless Chrome 157 pages, a Ranger looses 12 arrows, and B times
  each one.
- **Network:** the pages reach a local room through
  `coax/web/room/net-proxy.coax`. Each cell sets a loss rate and an RTT,
  with jitter ±30 ms throughout.
- **Seed:** 13177.
- **Machine:** an Apple-silicon Mac shared with other agents. Its load
  average ran from 10 to 40.
- **Fresh rooms:** each round trip ran on a fresh local room with an
  empty checkpoint store, to stay clear of #13185.
- **Trials:** 12 per cell, so a cell's p95 is close to its maximum. Treat
  differences under about one tick (33 ms) as noise.

Columns:

- **Arrows seen:** the arrows B saw, of 12.
- **Remote arrow:** A's button up to B drawing the arrow, in ms.
- **Walk:** W down to A's own drawn movement, in ms.
- **Behind:** how far behind the others are drawn, in ms.
- **Bytes/s:** page A's bytes a second, in and out.
- **Tick:** the room's tick CPU, p50 and p95, in µs.
- **Keys:** the keyframes A received.

WebSocket's cells come from the first run. The fix doesn't touch the
WebSocket path. WebTransport's cells come from the run after 6133b87c69,
with the "before" figure in brackets where it differed in kind.

| Transport | Loss | RTT ms | Arrows seen | Remote arrow p50 / p95 | Walk p50 / p95 | Behind | Bytes/s in / out | Tick p50 / p95 | Keys |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| WS | 0% | 50 | 12/12 | 92 / 114 | 9 / 15 | 100 | 1752 / 824 | 300 / 600 | 2 |
| WT | 0% | 50 | 12/12 | 92 / 118 | 9 / 16 | 100 | 1826 / 2795 | 200 / 200 | 4 |
| WS | 2% | 50 | 12/12 | 86 / 194 | 10 / 16 | 140 | 1709 / 825 | 300 / 500 | 8 |
| WT | 2% | 50 | 12/12 | 105 / 183 | 12 / 17 | 100 | 1806 / 2654 | 200 / 300 | 3 |
| WS | 5% | 50 | 12/12 | 130 / 451 | 12 / 21 | 134 | 1666 / 824 | 200 / 400 | 8 |
| WT | 5% | 50 | 12/12 | 97 / 120 | 12 / 33 | 100 | 1754 / 2616 | 200 / 400 | 3 |
| WS | 10% | 50 | 12/12 | 205 / 427 | 11 / 57 | 170 | 1672 / 819 | 300 / 500 | 8 |
| WT | 10% | 50 | 12/12 (2/12) | 128 / 272 | 18 / 47 | 100 | 1633 / 2745 | 200 / 300 | 3 |
| WS | 0% | 100 | 12/12 | 141 / 180 | 9 / 19 | 100 | 2090 / 819 | 300 / 500 | 3 |
| WT | 0% | 100 | 12/12 | 166 / 173 | 11 / 24 | 100 | 2118 / 3853 | 200 / 300 | 5 |
| WS | 2% | 100 | 12/12 | 144 / 341 | 11 / 16 | 100 | 1715 / 823 | 400 / 700 | 5 |
| WT | 2% | 100 | 12/12 (0/12) | 148 / 175 | 10 / 15 | 100 | 1772 / 3668 | 200 / 300 | 4 |
| WS | 5% | 100 | 12/12 | 174 / 327 | 12 / 16 | 126 | 1698 / 824 | 300 / 700 | 10 |
| WT | 5% | 100 | 12/12 (0/12) | 156 / 271 | 12 / 15 | 100 | 1734 / 3743 | 200 / 300 | 4 |
| WS | 10% | 100 | 12/12 | 317 / 366 | 9 / 15 | 132 | 1717 / 824 | 400 / 1400 | 5 |
| WT | 10% | 100 | **0/12** (12/12) | - | 11 / 32 | 100 | 1612 / 5953 | 300 / 500 | 5 |
| WS | 0% | 150 | 12/12 | 197 / 234 | 10 / 16 | 100 | 2197 / 822 | 400 / 800 | 6 |
| WT | 0% | 150 | 12/12 | 184 / 224 | 10 / 15 | 100 | 1864 / 5883 | 200 / 400 | 6 |
| WS | 2% | 150 | 12/12 | 197 / 574 | 9 / 14 | 124 | 1777 / 824 | 300 / 600 | 12 |
| WT | 2% | 150 | **5/12** (12/12) | 196 / 229 | 12 / 18 | 100 | 1695 / 3788 | 200 / 400 | **60** |
| WS | 5% | 150 | 12/12 | 360 / 615 | 9 / 16 | 130 | 1796 / 824 | 400 / 700 | 12 |
| WT | 5% | 150 | **0/12** (2/12) | - | 13 / 18 | 100 | 11 / 23377 | 100 / 100 | 0 |
| WS | 10% | 150 | 12/12 | 452 / 787 | 11 / 18 | 235 | 1913 / 821 | 300 / 1000 | 11 |
| WT | 10% | 150 | **0/12** (0/12) | - | 5 / 15 | 100 | 4 / 23373 | 100 / 100 | 0 |

The pass bar, 16 trials each, on fresh rooms after the fix:

| Transport | Loss | RTT ms | Arrows seen | Remote arrow p50 / p95 | Bar (2 RTT) | Bytes/s in / out | Keys |
| --- | --- | --- | --- | --- | --- | --- | --- |
| WS | 2% | 120 | 16/16 | 170 / 255 | 240 | 1615 / 814 | 5 |
| WT | 2% | 120 | 15/16 | 171 / 313 | 240 | 1662 / 4299 | 4 |

## What the table says

- **When WebTransport works, it beats WebSocket under loss.** In the cells
  where every arrow came through, WT's p95 stays near one RTT plus a
  tick. WebSocket's grows with TCP's resends: at 5% and 50 ms it's 120 ms
  against 451 ms, and at 2% and 100 ms 175 ms against 341 ms. The others
  are drawn 100 ms behind on WT in every cell. WebSocket's buffer grows
  to 235 ms.
- **It doesn't always work.** Five WT cells lose most arrows or all of
  them. The pass-bar cell misses one of 16, and its p95 is over the bar.
  #13187 tracks the causes.
- **Bytes:** page A receives the same on both transports, about
  1.6 to 2.1 KB/s. WT sends 3 to 7 times more, 2.6 to 5.9 KB/s, because
  each input datagram repeats the inputs the room hasn't acknowledged.
  When the acknowledgements stop coming (the silent cells), every
  datagram is a full 1,200-byte bundle, about 23 KB/s.
- **Server CPU per tick** is 200 to 400 µs at p50 and under 1.4 ms at p95
  on both transports. The one exception was 77.6 ms at p95, in the broken
  5%/150 ms WT cell before the fix.

## Twenty clients

`load.coax everglade2` placed 20 bot clients (WebSocket) in two rooms of
16 and 4, straight to a local room for 60 s. net-proxy holds 6 socket
pairs (the shared socket table has 16 slots), so this run had no injected
loss.

- All 20 opened and stayed open. None were blind, and no state frame was
  dropped. Every bot heard state at 30 Hz.
- Input to acknowledgement: p50 18 ms, p95 32 ms (room of 16) and 27 ms
  (room of 4).
- Bytes per player: 15.3 KB/s in the room of 16, and 5.2 KB/s in the room
  of 4.
- Room tick: p50 500 µs, p95 600 µs.
- The room refused 43 frames (`r`); the bots heard 197 of their own casts back.
- The tool's own verdict printed "fails". Its summary doesn't say which
  per-bot bar a bot missed; the figures above are all inside its stated
  bars.

## The pipeline's check

The build hosts have no Chrome, so the browser matrix runs on a Mac:

```
coax/web/tool.sh coax/web/room/net-matrix.coax --page http://127.0.0.1:PAGE --room ROOM \
  --hash HEX --out FILE --quick --check docs/audits/2026-11-11-room-loss-matrix.jsonl
```

`--check` fails a cell when its remote-arrow p95 is over 1.5 times this
file's figure plus 50 ms, or fewer than half its arrows were seen.

The pipeline itself runs the model check in `room-wt.coax`
("webtransport under loss"). It runs with the room's checks
(`coax/web/room/room.coax`) on every build host, and drives the room's
real frames through a seeded model of both transports. It fails when, at
2% or 10% loss with 120 ms and ±30 ms, a WebTransport page's state is
2 RTT old or more at the 95th percentile, or older than a WebSocket
page's.

## Bugs the runs found

- **Streams starved** (`wt.c`). Fixed in 6133b87c69.
- **The proxy reordered datagrams** (`net-proxy.coax`). Jitter was applied
  to each datagram on its own, so datagrams overtook each other and QUIC
  counted spurious loss. TCP through the same proxy keeps order. Fixed in
  6133b87c69.
- **#13185:** a local dev room stalls its checkpoints and its pages hear
  nothing. The matrix ran each round trip on a fresh room to stay clear
  of it.
- **#13187:** the WebTransport defects above.
