# Nostr consumers: project-page regression, consumer profiles, application evidence (2026-11-03)

Issue #12630. Everything here ran on coderos-4080 (Linux x86_64) against
local processes, with **test keys** or public events only. No production key,
signer, payment or deployment was touched; nothing here approves one.

Compiler: the checked release build with input fingerprint
`727901f6a2be…` from `~/.cache/coax-release` (the build of this tree's
fingerprint, `0ef5b97d2e65…`, was still running on the machine). Base commit
`57c3f6e63a`.

## 1. Project-page regression after the NIP-22 refusal

Chapter 52 added `N22_KINDS` (a comment whose parent kind is neither 1111 nor
the root's refuses) to `coax/lib/nostr/nip22.coax`. The project page loads it
through `project-fold.coax`; its checks were not run because they need the
generated `apps/openagents-web/projects/baked.coax`.

Generated inputs: `openagents project snapshot --project SLUG` for all ten
projects with a page (read-only from the public relay; `coax`: 321 items,
1,330 activity), then `openagents project bake` (10 projects). The generated
files are git-ignored and not committed.

| Suite | Result |
| --- | --- |
| `apps/openagents-web/project-checks.coax --library --steps 2000000000` | 88 of 88, 62 s |
| `project-live-checks.coax` (native, with `project-capacity.coax`) | 5 of 5 |
| `project-frames.coax` (native, with `project-capacity.coax`) | 10 of 10 |
| `apps/openagents-web/nip-guide-observe-test.coax --steps 80000000` (also loads `nip22.coax`) | 458 of 458 |

The native suites were built as `coax/web/suites.coax` builds them (`coax lib`
for the suite and its unit, linked with `coax/web/native-check.c`), by hand,
because `suites` refuses a compiler not built from this exact tree.

Relay differential for the project page's five REQs
(`coax/tests/nostr/relay_differential.py`, ASan/UBSan relay): 237 recorded
events published, each OK; `project` 2, `issues` 105, `comments` 121,
`status` 8, `repo` 1, each the same set in the same order as the Rust relay's
recording; no sanitizer report. One difference is noted, not failed: the Rust
relay's EOSE carries the NIP-67 `["finish"]` hint and the Coax relay's
doesn't. The Rust-relay differential for COUNT, search, NIP-11 and the newer
wiring was refreshed by #12631 in `57c3f6e63a` (163 observations, none
unexplained) and was not repeated here.

## 2. Consumer profiles

`coax/lib/nostr/consumers.coax` (`#authority`) names ten real consumers, each
on its target, and four axes for each: sign, verify, key storage, transport.
Each used axis names its file, a symbol that must stay in it, and the record
that shows it running there. The rule: a profile is complete only when every
used axis is in Coax with a record, or, for key storage and transport only,
done by the host platform with a record. Checks: 9 of 9; `coax mutate`: 92
of 92 killed.

`coax/tests/nostr/consumer_report.coax` reads the registry against the tree:
sources and symbols present, SHA-256 of each source, whether each named
record exists and names its source, the compiler's provenance record and the
base commit. It wrote `nostr/book/reports/consumers-2026-11-03.md` and
`.json`. Result: **0 of 10 profiles complete**:

- `cli`, `pylon`, `coder-host`: key storage blocks (a 0600 file; a refused
  `--key-file`; a mode 600 file).
- `api-nip98`, `homepage`: no record for the host transport.
- `project-page`: live events are relay-trusted; no verification.
- `web-signin`: no Wasm record for signing; entropy kept in browser storage.
- `hosted-runner`, `unlock-opener`, `ios-app`: libsecp256k1 or nostr-effect
  sign outside Coax.

## 3. Applications, with test keys

### The CLI (native, `apps/openagents-cli`)

The relay `coax/relay/build.sh` built, on `ws://localhost:18741`. Two
identities, each SHA-256 of a fixed public label, passed only through
`OPENAGENTS_NSEC` with an empty `HOME`; nothing was written to a real
profile. Signing is `nostr_event_sign_step` (`nostr.coax`); every event read
back passes `rl_read` (`relay.coax`) before it is shown; the socket is
`relay_conn.coax`.

- `nostr publish` from A: accepted, NIP-42 AUTH 1, 21 ms.
- `nostr req` from B for A's notes: 1 event, verified, to EOSE, 21 ms.
- B publishes; A reads both: 2 events, verified.
- `nostr auth` writes a 546-byte `Nostr …` NIP-98 header.
- An independent BIP-340 check (Python, after BIP-340's reference code; not
  committed) recomputed both ids and verified both signatures, and refused
  each with one signature bit flipped.

### The Pylon: provider, job and attempt binding

`python3 -I psionic/tests/pylon/pylon_e2e.py` (ASan/UBSan relay and Pylon,
`psionic-serve --toy`, the TEST-ONLY buyer): 52 checks passed, including
"outcome answered for request r-1, attempt 1" and "service door
pylon:ch19-local, provider the Pylon's key". The Pylon (`run.coax`) signs
in Coax with a test key, authenticates with NIP-42, seals with
`nip44_encrypt_checked`, verifies with `rl_read`, survives a relay restart,
re-authenticates, resubscribes from the newest job and republishes its beacon.
No sanitizer report. The buyer is `nostr-effect` and `@noble` from an existing
local `node_modules`, used as the independent client.

### Two-client Verse presence

`coax/tests/nostr/verse_presence_pair.coax`: two processes, each a client
with its own test key, against the local Coax relay. Each signs NIP-MV pose
frames (kind 23300, world `MV_PLAZA`), publishes on one connection and
subscribes to the other's on another; `rl_read` verifies each frame,
`mv_frame_read` admits it for the world, `mv_order` drops replays. Latency is
the receiver's wall clock minus the sender's `t`.

| Run | A received | B received | Median | Max | Budget |
| --- | --- | --- | --- | --- | --- |
| 40 frames, 250 ms apart, default relay | 40/40 | 40/40 | 5 ms | 15 / 17 ms | 100 ms |
| 200 frames, 100 ms apart, default relay | 60/200 | 60/200 | 6 ms | 30 ms | refused |
| 200 frames, 100 ms apart, `--rate 1200,120,1200` | 200/200 | 200/200 | 5 ms | 26 / 10 ms | 100 ms |

Finding: the relay's default of 60 events a key a minute (the Rust relay's
too) refuses presence above one frame a second. The policy is an owner
decision (NEEDS_OWNER.md, Nostr book follow-ups, item 5). The relay is
#12631's and was not changed. This is loopback on one machine; it doesn't
measure the internet or a browser realm.

### Wallet

No regtest or Spark test environment exists on this machine, and Breez's
local regtest is third-party `docker compose` code that
`docs/lightning/testing.md` sends to a cloud host. No wallet interaction ran
and no payment was made. Recorded as an owner action (item 6).

### Agent runtime carriage: the coder host

`coax/web/tool.sh coder/tui/host_smoke.coax`: `coder host` (`host.coax`) on
a local Coax relay in a fresh `CODER_HOME`, a stand-in browser key, the echo
engine. 26 of 26 checks: pairing asked and waiting, a thread refused before
it is allowed, allowed, the grant read, `thread.run` on the host's engine,
`thread.read` to finished, `thread.stop`, an unpaired key refused, a
forgotten key refused as revoked, key and `state.json` mode 600. Signing is
`nostr_event_sign_step` in Coax; transport is `relay_conn.coax` and the
CLI's `relay.coax` client with `rl_read`. The host key is a generated
throwaway file.

## Not covered here

- The hosted runner's signer migration and the unlock opener: production
  identities and rollback (owner, and #12630's migration task).
- iOS and browser targets: nothing ran on them for this record.
- Settlement expiry, the NIP-KEYS pin, real Pylon keys: unchanged owner
  decisions in NEEDS_OWNER.md.
