# Chat load benchmark

This page is the baseline for how fast the OpenAgents phone apps get a person
into a Coder chat and back an answer, phase by phase, and a ranked list of
where the time goes. It covers Coder chats only: the phone no longer lists
Claude Code, Codex, OpenCode, or Devin chats
([#9920](https://github.com/OpenAgentsInc/openagents/issues/9920)).

It measures three things:

- Opening the Coder tab to a composer you can type in, on a cold and a warm
  launch.
- Tapping Send to the first visible words of the reply and to its end, for
  the basic Coder (the hosted chat, NIP-CJ) and for Coder on a computer
  (NIP-HOST, auto-start, and the observer).
- Showing the list of earlier Coder chats and opening one, through the relay
  and directly over the tailnet.

[Coder runtime index](README.md)

## Run it

The harness is `crates/chat-load-bench`. It drives the real
`coder-connect` client and relay link, the observer host
(`coder_connect::host::Host`) under its real relay loop
(`coder_connect::cli::serve_observer`), the direct transport, the
`coder-history` reader, and the `rust-native` transcript layout.
`crates/openagents-mobile` is its own Cargo workspace, so the phone's read
loops are mirrored in `crates/chat-load-bench/src/phone.rs`, constant for
constant, with the source they follow named at the top of that file.

The deterministic run needs no network and reads nothing private: 80
synthetic Coder task transcripts, a loopback NIP-42 relay, and a loopback
direct listener.

```sh
cargo run -p chat-load-bench --release            # 5 runs, 5 chats opened per run
cargo test -p chat-load-bench --release           # the same path once, as a check
```

Other sources and paths:

```sh
# This machine's own Coder task transcripts (~/.openagents/tasks).
cargo run -p chat-load-bench --release -- --source coder

# The production relay, with a new temporary host key. Repeat for more samples.
cargo run -p chat-load-bench --release -- --source coder \
  --relay wss://relay.openagents.com --runs 1 --chats 3 --no-direct --no-cold

# The basic Coder: five messages to the OpenAgents chat worker, each from a
# new device key, timed leg by leg.
cargo run -p chat-load-bench --release -- --basic-coder 5 --no-relay --no-direct

# The same path against a local coder-worker on its stub door (no model).
cargo build -p coder --bin coder-worker --release
cargo run -p chat-load-bench --release -- --basic-coder-local 10 --no-relay --no-direct
```

The phone's own side, through `App::call` as the iOS and Android hosts drive
it (`crates/openagents-mobile/src/speed_tests.rs`):

```sh
# Launch to a ready composer, cold and warm. No network; runs with the other tests.
cargo test --release --manifest-path crates/openagents-mobile/Cargo.toml \
  coder_tab_to_composer_timings -- --nocapture

# Send to first words and to done. Network. The computer-backed test needs a
# host with tailnet admission and auto-start on, and archives its task.
OPENAGENTS_TEST_ADMISSION=<host tailnet IP> cargo test --release \
  --manifest-path crates/openagents-mobile/Cargo.toml speed_tests \
  -- --include-ignored --nocapture --test-threads 1
```

A production-relay run pairs with a new, temporary observer host, so the
running `coder host serve` never sees its reads. Titles, paths, and message
text are never printed.

## Baseline

Measured on 2026-09-28 on the owner's Mac (Apple silicon), at `cd999566cd`
unless a row says "before". "Before" is `38aeec3b19`, a few hours earlier,
ahead of `6dc829f3ef` (faster observer reads) and `6038a5ea48` (the Coder tab
opens on a new chat). The machine was running many other agents, so disk and
p95 values vary. The phone and the host were the same machine; the round trip
to `relay.openagents.com` was about 30 ms (ICMP 28 to 39 ms). "Direct" is the
loopback listener, so a real tailnet adds its round trip to each direct read.

### What the person waits for

| Wait | Path | Median | Spread (n) | Before | Most of the time |
| --- | --- | ---: | --- | ---: | --- |
| Launch to a ready composer, new install | phone | 9.5 ms | 8.4 to 11.9 ms (30) | 133 ms, up to 390 ms under disk load | creating encrypted stores, each with `fsync` |
| Launch to a ready composer, warm | phone | 1.3 ms | p95 8 to 14 ms (30) | 2.6 ms | nothing |
| Coder tab to composer in a running app | phone | 0 ms | the first packet shows it | one tap to New chat | nothing |
| Send to first words, basic Coder | NIP-CJ, production | never | 10 of 10 silent | never | no chat worker is running (bottleneck 1) |
| Send to done, basic Coder, stub worker | NIP-CJ, loopback | 0.9 ms | p95 3.7 ms (10) | | the model and the relay are the cost |
| Send to Working, Coder on a computer | NIP-HOST + auto-start | 2.6 s | 1.7 to 4.8 s (6) | | the engine's session opening |
| Send to first reply row, Coder on a computer | NIP-HOST + observer | 10.0 s | 8.1 to 12.6 s (6) | | the engine's whole turn; nothing streams |
| Send to Done, Coder on a computer | | 11.3 s | 9.0 to 13.4 s (6) | | |
| Earlier Coder chats list, relay link already warm | relay, production | 459 ms | 417 to 473 ms, 2 pages (3) | 846 ms | relay transit |
| Earlier Coder chats list, new client | relay, production | 762 ms | 693 to 911 ms, 2 pages (3) | 846 ms | opening the relay link |
| Earlier Coder chats list | direct, loopback | 2.5 ms | p95 2.6 ms, 1 page (5) | 11 ms | |
| Open an earlier Coder chat, first rows | relay, production | 210 ms | 191 to 267 ms (9) | 294 ms | relay transit |
| Open an earlier Coder chat, not kept on the phone | relay, production | 868 ms | 794 to 1,350 ms, 4 to 6 pages (9) | 1.23 s | page count times relay transit |
| Open an earlier Coder chat, not kept on the phone | direct, loopback | 8 ms | p95 13 ms, 1 or 2 pages (25) | 27 ms | |
| Reopen a chat the phone kept | phone | first view | 0 round trips | | |

The Coder tab paints its list of earlier chats from its kept copy (the
`coder-list` store) in the first packet. The catalog read supplies each chat's
title and the transcript an opened chat reads, so it decides how soon a chat
the phone did not keep can show its rows. The app opens each computer's
relay link and direct connection (`Client::warm`) when it comes to the
foreground and when the Coder tab shows, so the "link already warm" row is the
usual case.

### Relay exchange, leg by leg

One observer read on a relay link with its standing reply subscription,
through `relay.openagents.com` (3 runs, 3 exchanges each):

| Leg | Median | Before |
| --- | ---: | ---: |
| Link: TLS, WebSocket, and NIP-42 AUTH | 239 to 262 ms | 212 to 227 ms per connection |
| Link: standing reply subscription, REQ to EOSE, once | 59 to 65 ms | 52 to 63 ms for every read |
| Request to the relay's OK | 86 to 95 ms | 77 to 85 ms |
| Request to its reply: relay in, host answers, relay out | 195 to 255 ms | 274 to 312 ms |
| Of which the host's answer | about 2 ms | 9 to 19 ms |

Before, a hop into the host took 83 to 86 ms and a hop back 99 to 118 ms, each
about three round trips.

### Basic Coder, leg by leg

The production chat worker answered none of 10 jobs on 2026-09-28, and the
repository's own `live_basic_coder_streams_a_reply` fails with `Silent` after
its 30-second contact wait. `oa-coder-worker-1` runs only
`coder-worker.service`, the executor worker; the chat worker's unit
(`coder-worker-chat` in [the chat worker runbook](../../deployment/chat-worker.md))
is not installed. The legs before the worker, measured against production:

| Leg | Median |
| --- | ---: |
| Seal: NIP-44 encrypt and sign the `25900` request | 0.1 ms |
| Relay connection: TLS, WebSocket, and NIP-42 AUTH, new for every message | 210 to 270 ms |
| Reply subscription: REQ to EOSE | about 55 ms |
| Request to the relay's OK | about 80 ms |

With the local worker on its stub door, every leg together takes 0.9 ms. On
production, the first words would arrive after about 350 ms of transport
before the request reaches the worker, one relay hop back (about 100 ms), the
worker's first answer (since `95c7eda2e3`, a Jev opener as partial `seq` 0),
and up to 250 ms until the iOS host's next poll.

### Coder on a computer, from the host's own records

Each run's times from the phone's Send, lined up with the task's ATIF
records and the host's `autostart.jsonl` (runs 1 and 2 before, 4 to 6 on
`cd999566cd`; run 3's records were not read):

| Step | Run 1 | Run 2 | Run 4 | Run 5 | Run 6 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Auto-start starts the task (1-second resolution) | 0 s | 0 s | 0 s | 0 s | 0 s |
| The task's ATIF session opens | 1.18 s | 3.08 s | 2.76 s | 1.41 s | 1.57 s |
| Jev decision answered (`decision_response`) | 1.96 s | 3.92 s | 3.60 s | 2.15 s | 2.37 s |
| The engine's turn (`effect`) ends | 7.99 s | 10.53 s | 10.13 s | 8.11 s | 9.64 s |
| Phone shows Working | 1.69 s | 3.53 s | 3.29 s | 1.79 s | 1.91 s |
| Phone shows the first reply row | 8.09 s | 10.67 s | 10.21 s | 8.20 s | 9.74 s |
| Phone shows Done | 8.99 s | 12.50 s | 11.69 s | 9.00 s | 10.82 s |

The engine's turn takes 5.9 to 6.6 s, and the phone shows the reply about
0.1 s after it ends, over the direct connection's change notifications. The
prompt was "Reply with only the word ready."

### Phone and host CPU

| Phase | Median |
| --- | ---: |
| Seal a request (sign + NIP-44) | 0.2 ms |
| Open a reply (NIP-44 decrypt + verify) | 0.8 ms |
| Chat list view: validate + JSON | 0.1 ms |
| First layout of an opened Coder chat (bundled-font shaper) | 0.5 ms |
| Host history read for one catalog or transcript page (Coder source) | 0.2 to 0.5 ms |
| One new encrypted phone store and its first write | 7.5 ms (92 ms under disk load) |

## Where the time goes, ranked

Each item names the code responsible and an estimate of what fixing it would
save. The estimates come from the measurements above, not from prototypes.

1. **The basic Coder has no worker.** Send never shows words; the phone shows
   a failure after the 30-second `CONTACT` wait
   (`crates/openagents-mobile/src/basic_coder.rs:49`). The chat worker's unit
   is not installed on `oa-coder-worker-1`. Installing it is an owner step
   (its secret and gateway key live only in its environment file), and it
   turns "never" into about 0.5 s plus the worker's first answer.
2. **Coder on a computer shows nothing until the engine's turn ends.** The
   first reply row appears when the turn's `effect_result` is written, 5.9 to
   6.6 s after the turn starts. The turn starts 1.4 to 3.1 s after auto-start
   picks the task up (the session opening), after a Jev decision of 0.25 to
   0.5 s (`crates/microcoder/src/repository/native.rs:66`). Writing the
   engine's reply into the transcript as it streams, and keeping an engine
   warm so the session opens at once, would move first words from about
   10 s to the model's first token plus about 1 s: an estimated 3 to 5 times
   sooner. This is the longest wait a person sees.
3. **Every relay hop costs about three round trips.** Observer requests and
   replies are stored kind `3188` events, and the relay fans a stored event
   out only after its Postgres admission transaction, with its `NOTIFY`,
   commits (`crates/nostr-relay/src/store/mod.rs:349`, `2875`). The request
   reaches the relay's OK in 86 to 95 ms at a 30 ms round trip. Fanning out
   from memory as soon as the event is admitted, or carrying observer reads
   as ephemeral events, would bring each hop to about one round trip: an
   exchange from about 230 ms to about 80 ms, for every relay read on this
   page.

   A local relay on Postgres behind a delaying proxy
   (`crates/nostr-relay/tests/exchange_latency_postgres.rs`) showed the hop's
   cost was the admission's sequential database statements, about 23 round
   trips to the `OK` and 2.5 more before delivery, not where the fan-out sits.
   The relay now pipelines them (about 6 round trips) and delivers its own
   commits from memory. At a 4 ms database round trip an exchange went from
   215 ms to 53 ms (`OK` 97 to 27 ms, delivery 108 to 27 ms); at about 8 ms,
   from 418 ms to 99 ms. Production reaches `OK` about 60 ms after its
   30 ms network round trip, about 2.5 ms per Cloud SQL statement, which
   predicts an exchange near 100 to 150 ms once deployed. Not yet measured
   on production.
4. **Opening a Coder chat reads until it finds 10 messages.** A Coder task's
   transcript is about 88% `System` adapter records that show no row (on this
   Mac, 969 of 1,118 records), so the first batch rarely finds
   `BATCH_MESSAGES` (10) and reads backward pages up to `BATCH_PAGES` (12),
   16 KiB at a time through the relay
   (`crates/openagents-mobile/src/conversation.rs:37`, `46`, `51`). Real
   chats took 4 to 6 pages; synthetic ones took up to 12. A Coder-specific
   stop (the task's turns, not 10 messages), larger relay pages, or a
   host-side page that skips records with no row would make most opens one or
   two reads: about 0.25 to 0.45 s instead of 0.8 to 1.35 s. Direct reads
   already take 1 or 2 pages.
5. **The iOS host asks for results on a timer.** Rust has a read's result
   before the host asks for it: the Coder tab polls every 1 s while a chat is
   live and every 3 s otherwise
   (`bins/openagents-ios/host/App/OpenAgentsApp.swift:288`), and every 250 ms
   while a basic reply streams (`OpenAgentsApp.swift:296`). A callback from
   Rust when the packet changes would save 0.5 s on average (up to 3 s) after
   each read, and 125 ms on average before the first streamed words.
6. **The basic Coder opens a new relay connection for every message.**
   `Relay::run` connects, authenticates, and subscribes for each turn
   (`crates/openagents-mobile/src/basic_coder.rs:371`): 265 to 325 ms before
   the request leaves the phone. A kept connection with a standing reply
   subscription, as the observer's `Link` now has, would save nearly all of
   it from the second message on, and all of it if warmed with the Coder tab.
7. **The relay link opens on the first read when nothing warmed it.** A new
   client's first list took 693 to 911 ms against 417 to 473 ms after
   `Client::warm`: the 300 to 330 ms link opening
   (`crates/coder-connect/src/transport.rs:188`). The app warms clients on
   foreground and when the Coder tab shows; a read that races that warm-up
   pays the difference.
8. **A first launch creates its stores one `fsync` at a time.** On a new
   install `App::open` took 8 to 12 ms on a quiet disk and 133 to 390 ms
   while other processes loaded it, mostly creating encrypted stores, each
   first write syncing the file and the directory
   (`crates/coder-computers/src/cache.rs:117`, `120`). Creating stores on
   first use would make it steady near the warm 1.3 ms. It happens once per
   install.

Phone CPU is not a bottleneck: sealing, opening, the list view, the packet,
and a Coder chat's layout each take under 1 ms.

### Fixed since the first baseline

`6dc829f3ef` fixed four bottlenecks this benchmark found at `38aeec3b19`:

- **A new relay subscription for every read.** `Session::exchange_event`
  sent `REQ` and waited for `EOSE` before each request, 52 to 63 ms each; the
  client now keeps one standing link per computer
  (`crates/coder-connect/src/transport.rs:177`).
- **The host's relay loop re-answered old requests.** Its subscription had a
  256-frame budget, so it renewed after about 127 reads and was sent the last
  minute's requests again, which it answered and published again: 175
  requests were answered 6,370 times, and 300 were answered 22,550 times. It
  now has a 4,096-frame lease and skips requests it already took
  (`crates/coder-connect/src/cli.rs`, `serve`).
- **Every read rewrote and synced the observer book**, 8 of the host's 9 ms
  per read, with every retained relay reply in it (2.4 MB at peak). Reads now
  go to an append-only request log with no sync per read; the host's answer
  is about 2 ms.
- **The host answered relay reads one at a time on its event loop.** It now
  answers up to eight at once off the loop.

Together they took a loopback relay exchange from 9.8 ms to 1.8 ms and a
direct list from 11 ms to 2.5 ms, and on production an exchange from 274 to
312 ms to 195 to 255 ms.

### Fixed waits in the load path

| Wait | Value | Where |
| --- | --- | --- |
| Coder tab poll while live, and otherwise | 1 s, 3 s | `bins/openagents-ios/host/App/OpenAgentsApp.swift:288` |
| Poll while a basic reply streams | 250 ms | `bins/openagents-ios/host/App/OpenAgentsApp.swift:296` |
| Open chat's newest-chats read while its task runs | every 3 s, or 30 s with a direct connection | `crates/openagents-mobile/src/coder_tab.rs` (`HEAD_EVERY`, `HEAD_NUDGED_EVERY`) |
| Keep an open chat's transcript on the phone | at most every 5 s | `crates/openagents-mobile/src/coder_tab.rs` (`KEEP_EVERY`) |
| One relay read, then a failed read | 8 s | `crates/coder-connect/src/client.rs:28` |
| After a direct read fails, reads use the relay | 30 s | `crates/coder-connect/src/client.rs:32` |
| One observer read at most | 20 s | `crates/openagents-mobile/src/conversation.rs:32` |
| Basic Coder waits for the worker to answer at all | 30 s | `crates/openagents-mobile/src/basic_coder.rs:49` |
| Host task summary loop checks the task store | every 250 ms | `crates/coder-host/src/serve/mod.rs` (`STAMP_EVERY`) |

No sleep sits on the load path itself; each wait above is a poll interval or
a deadline.

## Limitations

- The phone was simulated on the Mac. The phone-side CPU phases are small
  enough that a slower phone CPU would not change the ranking; cellular round
  trips would make every relay leg longer.
- The direct path was measured over loopback. A real tailnet read adds one
  round trip per page.
- The machine was busy, so disk-bound phases (`fsync`, first launch) varied
  by more than 10 times between runs.
- The basic Coder's worker and model latency were not measured, because no
  chat worker was running.
- The computer-backed path is six runs of a one-word reply on one host with
  auto-start and one engine lane, against the host build already running
  there; a longer reply widens the gap in bottleneck 2.
- The harness mirrors the phone's read loops, not its code. When
  `chats.rs`, `conversation.rs`, or `basic_coder.rs` change their constants
  or read order, change `crates/chat-load-bench/src/phone.rs` and
  `crates/chat-load-bench/src/cj.rs` too.
