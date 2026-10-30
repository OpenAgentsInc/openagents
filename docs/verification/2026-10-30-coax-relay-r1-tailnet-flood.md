# Coax relay R1: the Pylon on the tailnet, and a flood (2026-10-30)

Plan: [coax/nostr.md section 17](../../coax/nostr.md). R1 record:
[2026-10-29-coax-relay-r1.md](2026-10-29-coax-relay-r1.md). This closes two
items of its "Missing for R1": the client relay machine and the chapter 19
Pylon talking to the Coax relay on the tailnet, and bounded memory under a
flood. The flood found that 16 silent or trickling sockets could hold every
connection slot forever. That is fixed here with checks and zero-survivor
mutation.

Every run used test keys only. The Pylon used its `--test-key-label` key
(`--key-file` stays refused until audit #4828). No public relay was
contacted. relay.openagents.com and its database were not touched. The only
relays were the ones these scripts started, with `--store` in a fresh
scratch directory.

## The tailnet address

This box's tailnet address is `100.74.238.61`. Its MagicDNS name is
`coderos-4080.tailaeab8f.ts.net`, which resolves to that address. The relay
binds the literal (`--listen 100.74.238.61`). The client relay machine
(`relay_uri`, and so `relay_conn` and the Pylon) refuses IP literals by
design. So the Pylon's runs name the relay
`ws://coderos-4080.tailaeab8f.ts.net:PORT`, and that is also the relay's
`--url`, because NIP-42 AUTH events must name the URL the client used. The
flood driver is plain Python, so it uses `--url ws://100.74.238.61:PORT`.

## 1. The chapter 19 Pylon against the Coax relay on the tailnet

The Pylon is the program that runs the client relay machine: `relay_conn`
(transport), `relay.coax` (subscriptions, reconnects, generations),
`relay_read` (frames through message.coax, ids and signatures verified) and
`nip42.coax` (AUTH). So this run is also the client relay machine's run
against the Coax relay.

`psionic/tests/pylon/pylon_e2e.py` gains `--listen ADDRESS --host NAME`. The
relay binds ADDRESS instead of 127.0.0.1. The Pylon, the buyer and the NIP-42
URL use `ws://NAME:PORT`. The script refuses to run when NAME does not
resolve to ADDRESS. Without the flags nothing changes (127.0.0.1 and
localhost). The relay is still the Coax relay (`coax/relay/build.sh
--sanitize`, `--store`), started by the script.

```
python3 -I psionic/tests/pylon/pylon_e2e.py --listen 100.74.238.61 \
    --host coderos-4080.tailaeab8f.ts.net OUT_DIR
```

| Run | Relay | Result |
| --- | --- | --- |
| 1 | relay.coax as on main at `26d2e0d9c1` | **52/52**, `pylon e2e (coderos-4080.tailaeab8f.ts.net at 100.74.238.61): ok`. Build 199 s |
| 2 | with this change's deadlines and buffer (below) | **52/52**, the same line |

Both runs included the following:

- The Pylon's self-check and beacon.
- NIP-42 for the Pylon and the buyer.
- Jobs r-1 to r-8: answered, version-refused, cancelled, and answered after
  a relay restart on the same port and `--store`.
- The Pylon reconnects, re-authenticates once per new challenge (2 AUTHs),
  resubscribes with `since` moved forward and republishes its beacon.
- The NIP-65 list names `ws://coderos-4080.tailaeab8f.ts.net:PORT`.
- The relay accepts every event the Pylon publishes, and no input is
  forged.
- No sanitizer report from the Pylon or the relay.

## 2. The flood

`coax/tests/nostr/relay_flood.py` (new, standard library) starts the relay
with `--store` in a scratch directory. It samples `VmRSS` and `VmHWM` from
`/proc/PID/status` every 0.2 s and drives the relay past every limit the R1
record names. BIP-340 signing is in the script, with a test-only key: the
sha256 of a fixed label. The cases:

- **Connections:** 16 WebSockets, each with its own challenge. Then 200 more
  handshakes, and a burst of 100 raw sockets held open past the C host's 32
  handles.
- **Subscriptions:** 32 on each of the 16 connections, 512 in all. Each is
  two filters of 60 ids (about 8 KiB). Sub `s0` is live on kind 1 for the
  fan-out. The 33rd is refused. Then 17 filters, filters over 8192 bytes,
  65 ids in one filter, 8 open filters (query cost), a 69 KiB REQ of 16×64
  ids, and a 65-character sub id. After a CLOSE, the slot takes a new
  subscription.
- **Frames:** a 131074-byte text frame, a header claiming 2^40 bytes,
  fragments adding up past 131072 bytes, and an unfinished 131000-byte
  frame held while other connections are served.
- **Events:** AUTH, then valid signed kind-1 events until the store is full,
  each fanned out live to the 14 other open connections.
- **Slowloris:** 3 silent sockets, 4 that trickle an HTTP head and 8
  WebSockets that trickle a frame, one byte every 0.25 s. One idle
  WebSocket stays open beside them. A fresh client tries at 4 s and 13 s.
  Then two full-size frames and a REQ are pipelined in one write.

```
python3 -I coax/tests/nostr/relay_flood.py --listen 100.74.238.61 --events 2100 --extra 200 [--relay BIN] [--out DIR]
python3 -I coax/tests/nostr/relay_flood.py --listen 100.74.238.61 --events 100 --big --extra 200 --sanitize
```

### Results (final files, tailnet address)

| | Release (`build.sh`, -O1, no sanitizers) | ASan+UBSan (`build.sh --sanitize`) |
| --- | --- | --- |
| Checks | **31/31**, `relay flood: ok` | **31/31**, `relay flood: ok`, no sanitizer report |
| Connections 17 to 216 | 200 of 200 closed without a 101 | 200 of 200 |
| 100 raw sockets past the limit | 100 of 100 closed by the relay; the 16 accepted stay up | same |
| Subscriptions | 512 of 512 EOSE (0.9 s); 16 of 16 33rd refused "restricted: too many active subscriptions" | 512 of 512 (4.7 s); 16 of 16 |
| Filter refusals | "restricted: too many filters", "restricted: subscription filters exceed 8192 bytes" (8616-byte and 69824-byte REQs), "invalid: malformed filter" (65 ids), "restricted: query cost exceeds the configured limit", NOTICE "invalid: subscription id must contain 1 to 64 characters" | same |
| Frames | close 1009 for each of the three oversized cases; the unfinished frame is held, others are served, its last byte completes it | same |
| Events | 2100 of 577 bytes in 303.2 s (144.4 ms each, nearly all of it verification): **2048 stored, 52 refused "relay storage is full"**, first refusal at #2048, 0 other | `--big`, 100 of 60377 bytes in 130.5 s (1305 ms each): **69 stored (the 4 MiB arena), 31 refused "relay storage is full"**, first at #69 |
| Slowloris | at 4 s a fresh client is refused (all 16 slots held); silent sockets and trickled heads dropped at 10.0 s; at 13 s a fresh client is served; the 8 trickled frames get close 1008 at 30 s; the idle WebSocket is kept for 37 s; the pipelined pair and REQ are read and answered with EOSE | same (heads dropped at 10.3 s) |
| Run time | 359 s | 189 s |

### Memory

| | Idle (after start) | Peak (VmHWM) | End |
| --- | --- | --- | --- |
| Release | 27.5 MiB | **28.5 MiB** | 28.5 MiB |
| ASan+UBSan | 36.0 MiB | 79.9 MiB | 78.9 MiB |
| Release, rebased onto main (server TLS linked, store recovery) | 29.6 MiB | **31.1 MiB** | 31.1 MiB |

The tables above are from the files before the rebase. After the rebase
onto main (server-side TLS in `relay-host.c`, #966b recovery and
`rr_req_filters`), the release flood was run once more on the merged files:
31/31, `relay flood: ok`. 2100 events took 116.9 s (55.7 ms each, at a lower
host load): 2048 stored and 52 refused "relay storage is full", the first at
#2048. VmRSS was 30288 KiB idle and 31864 KiB from the first 200 events to
the end. The extra ~2 MiB is the linked TLS library. The ASan flood was not
repeated on the merged files.

Release VmRSS by phase, in KiB (min..max):

| Phase | VmRSS |
| --- | --- |
| idle | 28208 |
| connections | 28344 |
| subscriptions | 28424..28760 |
| frames | 28748..28800 |
| events | 29172..29240 |
| each 200-event mark, 200 through 2000 | 29240 |
| slowloris | 29176..29244 |

The resident set grows by about 1 MiB from start to the full store. It does
not move while the 2048 events are stored or while 52 more are refused.
Under ASan the store's 4 MiB arena shows as a 5 MiB step (75.6 to 80.6 MiB),
and the subscriptions phase adds about 30 MiB of shadow and quarantine. Both
stay flat after their phase. The samples are in each run's `rss.tsv`, in the
scratch output directory (not committed).

The flood does not reach one bound, the C host's output buffer:
`socket-server-native.c` mallocs `CXSRV_MAX_OUT` (4 MiB) per accepted
handle. Only the pages written count, and a client that never reads makes
`cxsrv_flush` fail after `RY_IO_MS` (2 s), which closes it. So the worst
case is 16 × 4 MiB of pending output on top of the figures above. That
figure is computed from the code, not measured.

## What the flood found, and the fix

**No deadline on a connection's input.** Before this change, a socket that
connected and sent nothing, or trickled an HTTP head, or trickled a frame,
held its slot forever. Sixteen of them locked every client out. This was
seen in a first run on the relay as it was on main. A fresh client was
refused for the whole 20 s, and a frame trickled for 36 s was never closed,
on the binary built from `26d2e0d9c1`. The Rust relay drops a handshake
after 10 s (`HANDSHAKE_TIMEOUT`, `gateway/socket.rs` at `0b401561a0^`).

- `coax/lib/nostr/relay_route.coax` (`#authority`) gains `rr_deadline(ws,
  used, since, now)`. The check came first.
  - An HTTP head that is not complete `RR_HEAD_S` (10 s) after accept is
    dropped.
  - A WebSocket that holds part of a frame and has taken no complete frame
    for `RR_FRAME_S` (30 s) is closed with 1008 (`RR_CLOSE_SLOW`).
  - An idle WebSocket with nothing buffered is kept, because a Pylon waits
    for jobs that way.
  - A clock that goes backwards drops no one.
- `coax/relay/relay.coax` keeps `since` per connection. It is set at
  accept, at the 101, and each time a complete frame is taken. Every serve
  pass (at most 1 s apart) calls `rr_deadline`.

**The input buffer is now one full frame plus one read.** `RY_IN` was 147456
bytes. A partial frame can be up to 131086 bytes and one transport read up
to 65536 bytes (`CXSRV_MAX_IN`), so a pipelining client could in principle
be closed with 1009 mid-stream. `RY_IN` is now `RS_MAX_MESSAGE + 14 + 65536`,
the bound `relay_conn` uses on the client side. The cut was not reproduced
on loopback, because read sizes there never lined up, so the change rests on
the arithmetic. The flood's pipelining check passes on both builds.

The driver's own first-run failures were harness bugs, and each is fixed:

- Filters of 120 ids are "malformed": `filter_read` takes at most 64 ids
  per filter.
- A Python socket closed while another thread reads it sends no FIN. The
  driver now shuts it down first.
- A 10 s socket timeout in the reader was taken as a close.

### Checks

| Check | Result |
| --- | --- |
| `coax test coax/lib/nostr/relay_route.coax --library` | 12/12 after the rebase (10 before, plus the deadline check and the parallel `rr_req_filters` check) |
| `coax mutate coax/lib/nostr/relay_route.coax --library --jobs 4` | Before the rebase: 265 of 265 killed. The rebase merged the Rust-differential agent's `rr_req_filters` into the same file, so it was run again on the merged file (sha256 `b182429b88738c09…`): **270 mutants, 270 killed, 0 survived**, no `// mutant: equivalent` lines |
| `python3 coax/tests/nostr/relay_session.py` (ASan+UBSan) | 72/72, `relay_session: ok`, both before the rebase and on the merged files (with server TLS and store recovery) |
| `python3 coax/tests/nostr/relay_differential.py` (ASan+UBSan) | 13/13, `relay_differential: ok`, before the rebase |
| Pylon e2e on the tailnet | 52/52 before and after the change (above) |
| Flood, release and ASan+UBSan | 31/31 each (above); release again on the merged files, 31/31 |

## Not done

- **Per-IP and per-pubkey limits.** Sixteen idle but valid WebSockets still
  hold every slot. That is a legitimate state (a Pylon is idle that way), so
  no deadline can cover it. The Rust relay's `max_connections_per_ip` (20)
  and its rate limits are R2, as the R1 record says.
- **The output-buffer bound** is computed, not measured (above).
- **Throughput.** Release verification is about 145 ms per event on this
  host at load around 32 on 28 cores, so a valid-event flood is bounded by
  verification at about 7 events per second. Refused events cost the same
  verification before "relay storage is full", because the store is asked
  only after the signature checks out.
- **wss://.** The flood and the tailnet Pylon run used ws://. Server-side
  TLS landed in parallel ([2026-10-30-coax-relay-r1-tls.md](2026-10-30-coax-relay-r1-tls.md)),
  and its sessions cover wss://.

## Rerun on the final tree

At `8bc425508a`, with every R1 change merged (server TLS, store recovery,
the Rust-differential fix, the deadlines):

| Check | Result |
| --- | --- |
| `relay_session.py` (ASan/UBSan, ws://) | ok |
| `coax/tests/net/tls.sh` (client cases and `relay_session.py --tls --client`) | all local checks pass |
| `relay_differential.py` (the relay.openagents.com recording) | ok |
| `rust_relay_differential.sh` | 114 observations, 0 unexplained |
| `relay_flood.py --listen 100.74.238.61` (release, 2100 events) | ok; VmRSS 29.7 MiB idle, 30.5 MiB peak |
| `relay_flood.py --sanitize --listen 100.74.238.61 --events 300` | ok; 36.9 MiB idle, 80.8 MiB peak |

The full 2100-event ASan flood exceeded the 15-minute command limit (about
1.8 events/s under ASan) and was stopped at 1400 events with VmRSS flat at
80.6 MiB; the 300-event run covers the same phases.
