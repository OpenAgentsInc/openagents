# New-land's Coax Nostr migration — 2026-10-30

Owner request: start with everything feeding the new-land landing page,
replace the foreign Nostr path with shared Coax, document the work and push
after focused checks. Full unchanged mutation runs are deliberately excluded.

## Baseline and scope

`/new-land` is a development-only captured view (`new-land/DEV-ONLY`). Its
generator `coax/web/project/landing.ts` previously read six projects and Pylon beacons
through the TypeScript project tools and `nostr-effect`. The Pylon reader
checks beacon signatures but omits the content x digest and most shared
beacon admission rules; activity counters trust relay-delivered events.
The renderer ages captured "online" values without expiring them.

This work keeps the development-only route. Generic browser/Bun WebSocket,
file and git effects may remain in the host; shared Coax owns Nostr message
reading, event/hash/signature checks, query bounds/pagination, project folding,
NIP-19 and Pylon beacon/freshness/activity decisions. No keys or signing are
required. Simulated people remain explicitly simulated, separate from Pylons.

## Implemented

`coax/web/project/landing-nostr.coax` uses the shared original-event reader,
public variable-time Schnorr verifier, Nostr message/filter readers and writers,
NIP-19 encoder, and Psionic's `pb_read` / `pb_fresh`. The real content bytes
are decoded and SHA-256 hashed before checking the beacon's x digest. Event
signatures, actual filter matching, query pagination and freshness execute in
Coax. `landing-reader.ts` moves WebSocket bytes and invokes the emitted Wasm;
the landing import graph contains no `nostr-effect`, `nostr-tools`, noble or
scure imports. Host hashing remains for build artifacts and explicitly
simulated names.

`landing-fold.coax` reuses the existing project fold in a separate translation
unit because the bootstrap compiler has a flat procedure namespace. The
private C bridge calls the Coax verifier on the same immutable original span
immediately before copying it into the fold's owned storage. The public Wasm
entry accepts original bytes, never a caller-supplied verification bit.
Known issue roots survive refreshes in `LAND_KNOWN_ISSUES`; missing roots get
exact-ID repair queries, and a final missing root or project refuses the fold.
No store overflow silently truncates the displayed capture.

The generator atomically renames its output only after all project and Pylon
reads complete. Development builds refresh it and retain the last complete
capture on refusal. Release builds continue to exclude `/new-land`.

Provider activity uses signed 26910 answers and 27010 status updates within
the capture's preceding 24 hours. Status updates can include progress; they
are no longer labelled failed jobs. Reply delay requires an independently
verified kind-25910 request addressed to that provider. Missing requests
produce unknown timing, rather than a fabricated zero. These observations
are provider-wide, so two machines sharing a provider can share counts.

The Coax renderer identifies the capture and simulated people. It uses the
generic host clock and elapsed time across pauses to expire real beacons;
expired advertisements stop contributing online slots. It no longer wraps
the observed age every minute. Cards distinguish signed observations from
completed work and show unknown delay explicitly.

## Focused verification

Baseline: `aca6a4ccfd` (main at implementation start). Source/artifact hashes,
the capture log and renderer results are in
[the evidence file](data/newland-coax-nostr-2026-10-30/results.json).

| Check | Result |
| --- | --- |
| `coax test coax/web/project/landing-nostr.coax --library --steps 250000000` | 292 / 292 passed |
| `bun test coax/web/tests/newland_reader.test.ts` | 7 tests, 579 assertions passed against the actual emitted Wasm |
| `coax test apps/openagents-web/newland.coax --steps 400000000` | 30 / 30 passed |
| `node coax/web/tests/newland_wasm.cjs apps/openagents-web/build/newland.wasm` | 9 frames, 3 viewports, finite geometry, intact guard bytes; maximum 50,718 of 200,000 vertices; unknown-clock, positive freshness and paused-page expiry checked |
| Real relay capture | Six projects and one real Pylon, zero demo Pylons; committed capture at 2026-10-30T14:26Z |
| Headless muted WebGPU Chrome | Reviewed 1440×900 desktop/Pylon card and 390×844 phone; capture labels, expired availability and unknown timing render correctly |
| Shell syntax, visual rules and `git diff --check` | Passed |

Reader checks include independently signed public fixtures (public throwaway
scalars 3 and 7), changed content/signatures, signed invalid beacon digests and
contracts, provider scoping, missing timing, duplicate events, CLOSED refusal,
multiple history pages, ambiguous one-record relay caps, frozen project-row
comparison (including parent/rank/commit/status time), missing known roots,
and a real failed socket read preserving the previous output. The negative
capture test requires the network error, so a parser/startup failure cannot
satisfy it.

An additional direct interpreter run of `landing-fold.coax` passed 22 of 23
checks, including both new small checks. The inherited full transcript test
in `project-fold.coax:1124` exceeds the evaluator's aggregate storage limit
before folding. Increasing a step budget cannot fix that separate limit.
The emitted Wasm comparison above executes that transcript's events and
compares their projected fields with the frozen independent TypeScript table;
the interpreter limitation remains open. This is not an all-green interpreter
gate. No compiler limits or inherited checks were weakened.

Per the owner's request, no full mutation/release gate was run. The unchanged
public verifier's earlier vector, native/Wasm differential and zero-survivor
mutation evidence remains in [the Schnorr record](2026-10-29-schnorr-fast.md),
introduced by `9e0480f61850f4c700c34853d4ca763ddcb50849`.

## Bounds and remaining work

The public reader has a 1 MiB input ABI, 8 KiB filters, a 200-event requested
page size, at most 100 pages per query, 8,192 distinct events per query,
and a per-connection admitted-event budget of 8,192 events / 8 MiB. Connection
and page timeouts are 10 and 15 seconds. The existing fold remains bounded by
8,192 events, 4 MiB decoded storage and 256 issue roots; activity projection
accepts at most 512 unique activity events and 512 request records. Capacity,
malformed input, ambiguous pagination and missing known roots refuse the
capture and preserve its predecessor. Raising these limits needs a measured
follow-up, rather than truncation.

EOSE's parsed NIP-67 finish hint is a relay declaration, not proof that the
relay disclosed all history. Known-root repair protects previously captured
roots; it cannot prove disclosure of unknown events or all updates. This is
a bounded public observation, not consensus or an authenticated completeness
claim. Optional beacon owner auth signatures are not verified here, and
project-authored worker labels establish no independently authenticated worker
identity. No keys, publishing, grants, spending or execution were added.

Next migration priorities beyond this development page are:

1. Independently verify events before the other live project page's trusted
   fold admission; replace the remaining SDK-backed snapshot/list readers.
   Investigate its repeated full-history reconnects separately.
2. Move NIP-98 admission and replay reservation together, then signing and
   sessions. The C API's inference path currently lacks the same replay
   reservation used by several other handlers. Keep signer/secret-source
   review gates, including issue 4828, intact.
3. Cut over unlock's NIP-44/NIP-59/key custody paths with their reviewed
   boundaries. The existing custom phone challenge flow is not a NIP-46 bunker.
4. Qualify Coax relay capacity and feature parity before replacing the
   production relay. Coax R1 already has TLS, NIP-42 and durable store replay;
   its 2,048-record / 4 MiB / 16-connection configuration and missing production
   features are still separate deployment work.

The new-land capture does not complete those broader migrations and is not
published as a live production landing page.
