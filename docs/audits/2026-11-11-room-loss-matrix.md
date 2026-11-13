# The room under loss: WebSocket against WebTransport (#13177)

Date: 2026-11-11. Every cell's JSON is in
`docs/audits/2026-11-11-room-loss-matrix.jsonl`, with a `phase` field.

## Summary

- After #13187 (below) WebTransport meets the bar, 16 of 16 arrows at a
  p95 of 190 ms, and is the default. The rest of this summary is the
  first run.
- WebTransport stayed behind its switch after the first run.
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

## After #13187: the full matrix again

Date: 2026-11-11, the same Mac, proxy, seed (13177), jitter (±30 ms) and
fresh room per cell, phase `wt2-13187` in the `.jsonl`. Three changes made
the difference:

- **Resends were garbage** (`wt.c`). The room's QUIC writer kept each
  stream's unacknowledged bytes in one buffer that it moved on every
  acknowledgement and grew with `realloc`. ngtcp2 resends lost stream data
  from the pointers it was given, so after a loss it resent whatever lay
  there by then. Frames on the reliable streams arrived corrupt or not at
  all: casts went unanswered, arrows went unseen, sessions went silent.
  The buffer is now a chain of chunks that never move, each freed once
  every byte in it is acknowledged.
- **Casts and launches don't wait for a resend.** A `C` or `F` goes on the
  combat stream and in the input datagrams under the next input's number
  until the room acknowledges it. The room takes each once by its sequence
  number, whichever way it came (`net_seen_fresh`). An arrow loosed (`a`)
  or ended (`d`) goes on the stream and as a datagram now and next tick;
  the page takes each once by the arrow's id.
- **Redundancy is bounded.** An input datagram repeats the newest 6
  unacknowledged inputs at most (`NET_OWED`), and an input that is the next
  one's but for its number and last word is one byte (`~`). Page A's out
  rate fell from 2.6 to 5.9 KB/s to 1.4 to 2.1 KB/s; a WebSocket page
  sends 0.8 KB/s.

| Transport | Loss | RTT ms | Arrows seen | Remote arrow p50 / p95 | Walk p50 / p95 | Bytes/s in / out | Tick p50 / p95 | Keys |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| WS | 0% | 50 | 12/12 | 90 / 124 | 8 / 16 | 1736 / 817 | 200 / 400 | 3 |
| WT | 0% | 50 | 12/12 | 86 / 119 | 11 / 15 | 1826 / 1442 | 200 / 400 | 2 |
| WS | 2% | 50 | 12/12 | 80 / 130 | 12 / 41 | 1685 / 816 | 200 / 300 | 3 |
| WT | 2% | 50 | 12/12 | 90 / 118 | 9 / 15 | 1809 / 1419 | 200 / 300 | 3 |
| WS | 5% | 50 | 12/12 | 137 / 272 | 11 / 15 | 1635 / 816 | 200 / 400 | 4 |
| WT | 5% | 50 | 12/12 | 99 / 135 | 7 / 14 | 1765 / 1442 | 300 / 500 | 4 |
| WS | 10% | 50 | 12/12 | 271 / 445 | 11 / 15 | 1688 / 820 | 200 / 400 | 5 |
| WT | 10% | 50 | 12/12 | 89 / 109 | 9 / 16 | 1664 / 1465 | 200 / 300 | 4 |
| WS | 0% | 100 | 12/12 | 136 / 171 | 11 / 15 | 1750 / 820 | 200 / 400 | 3 |
| WT | 0% | 100 | 12/12 | 152 / 168 | 9 / 15 | 1847 / 1760 | 200 / 300 | 5 |
| WS | 2% | 100 | 12/12 | 153 / 305 | 9 / 16 | 1721 / 821 | 200 / 400 | 3 |
| WT | 2% | 100 | 12/12 | 140 / 187 | 6 / 12 | 1810 / 1824 | 300 / 400 | 6 |
| WS | 5% | 100 | 12/12 | 147 / 247 | 9 / 15 | 1679 / 821 | 200 / 300 | 3 |
| WT | 5% | 100 | 12/12 | 141 / 158 | 12 / 24 | 1748 / 1839 | 300 / 400 | 5 |
| WS | 10% | 100 | 12/12 | 285 / 477 | 12 / 16 | 1705 / 820 | 200 / 300 | 5 |
| WT | 10% | 100 | 12/12 | 155 / 188 | 10 / 16 | 1651 / 1820 | 200 / 400 | 6 |
| WS | 0% | 150 | 12/12 | 175 / 197 | 11 / 16 | 1754 / 820 | 200 / 300 | 5 |
| WT | 0% | 150 | 12/12 | 197 / 240 | 12 / 16 | 1851 / 2048 | 300 / 400 | 6 |
| WS | 2% | 150 | 12/12 | 202 / 291 | 9 / 13 | 1716 / 820 | 300 / 400 | 5 |
| WT | 2% | 150 | 12/12 | 183 / 231 | 11 / 16 | 1829 / 2060 | 300 / 400 | 7 |
| WS | 5% | 150 | 12/12 | 263 / 713 | 9 / 16 | 2092 / 821 | 300 / 500 | 7 |
| WT | 5% | 150 | 12/12 | 200 / 221 | 11 / 17 | 1755 / 2069 | 300 / 400 | 5 |
| WS | 10% | 150 | 12/12 | 345 / 694 | 9 / 16 | 1890 / 820 | 200 / 400 | 9 |
| WT | 10% | 150 | 12/12 | 193 / 230 | 9 / 16 | 1675 / 2084 | 300 / 400 | 5 |

The pass bar, 16 trials each:

| Transport | Loss | RTT ms | Arrows seen | Remote arrow p50 / p95 | Bar (2 RTT) | Bytes/s in / out | Keys |
| --- | --- | --- | --- | --- | --- | --- | --- |
| WS | 2% | 120 | 16/16 | 146 / 344 | 240 | 1610 / 811 | 6 |
| WT | 2% | 120 | 16/16 | 166 / 190 | 240 | 1703 / 1813 | 6 |

- WebTransport meets the bar: 16 of 16 arrows, p95 190 ms against 240 ms.
  WebSocket misses it (344 ms).
- Every WT cell saw every arrow. Under loss WT's p95 stays within about
  1.6 RTT; WebSocket's grows with TCP's resends to 713 ms.
- The one cell where WT trailed, 0% at 150 ms (240 against 197 ms), ran
  again: WT 231 ms, WebSocket 216 ms. That is under a tick apart (noise,
  per the trials note above), and both are inside 2 RTT.
- `E2N_WT_DEFAULT` is now on, for a shard's own endpoint
  (`docs/metaverse/webtransport-server.md`).

## What the first table said

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
