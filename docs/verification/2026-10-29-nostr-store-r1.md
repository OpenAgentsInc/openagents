# Nostr event store, R1 part: in-memory verified store for the Coax relay

Tasks: nostr-event-store (#8eb5, R1 part) and nostr-store-verified-dedup
(#f539, the store's verified-ID dedup). Worker: claude-store, 2026-10-29.
**Not complete:** the store is in memory only. There is no durable file, no
NIP-09 deletion or tombstones, and no trusted recovery (#966b). A restart
forgets every event. Neither task is marked done.

## What landed

| File | What | Checks | Mutation |
| --- | --- | --- | --- |
| `coax/lib/nostr/store_core.coax` (`#authority`) | Every decision. It admits only when the caller's verification result says the signature over this exact text passed (`verified`). It keeps one record per event id and the lexicographically lowest exact original (`ns_original_before`, from `store_dedup_policy.coax`). A lower original no longer than the stored one is written in place. A longer one is appended and the old record retired, or refused (the old one kept) when it does not fit. A duplicate never takes another live record, and the byte count moves by the size difference. Replaceable kinds (0, 3, 10000–19999) are keyed by kind and author, addressable kinds (30000–39999) by kind, author and d tag. On replacement, the newer event wins, and on the same second the lowest id wins. NIP-40: an event is visible while `now` < expiration. Ephemeral kinds are accepted and not stored. Fixed bounds: a record table and a byte arena the caller sizes. When either is full, compaction drops replaced records and expired regular events. Expired replaceable and addressable heads stay so that older versions cannot come back. Then it refuses with "error: ..." (RS_FULL). Query order: newest first, then lowest id, cut at the limit and at the output's size. It loads neither `event.coax` nor the verifier | 46 (14 store, 32 from the loaded store_policy, dedup policy, snapshot and layout units) | **581/581 killed**, 0 survivors, no `// mutant: equivalent` |
| `coax/lib/nostr/store.coax` | The relay face. It implements the relay double's interface (`RsdStore`, `RsdRec`, `RSD_MAX`, `RSD_ARENA`, `rsd_admit`, `rsd_query`, `rsd_text`) by copying fields. It turns an `RsAdmit` into a record, and its verified flag is `reason == RS_ACCEPT && e.error == NE_OK && e.verified && e.original == text`. It turns a record back into a `NostrEvent` for `filter.coax`'s `nf_match` (ids, authors, kinds, since, until, every tag filter, including `#a #d #e #E #p #q`), with `rs_limit` for the limit. It is **not** `#authority`: loading `relay_server.coax` would put the relay's 6,087-mutant graph under every mutation run. All of its decisions are calls into store_core | 303 in the graph (3 its own: reason codes agree, verified-only admission, field mapping, filters over stored originals, order and limits) | not mutated (see above) |
| `coax/tests/nostr/store_verified_checks.coax` | Real BIP-340 verification. One signed event (`NS_VALID_TEST`) in three exact encodings: as signed, with the fields reordered, and with `h` escaped as `h`. All have the same id and signature. Every arrival order converges on the lowest original (the reordered one). A forged signature and a wrong signer never reach the store (`RS_BAD_SIG`, nothing written). The relay's own `nostr_event_verify_vartime` admits through the store, and the same verified result does not admit other bytes | 6 new (351 in the graph), with `--steps 400000000`: about 38M steps for each test with two fast verifies, and 79M for the relay verifier | n/a |

The type names keep the double's `rsd_`/`Rsd` prefix on purpose, so the relay
swaps stores by changing one line in `coax/relay/relay.coax`:

```
#load "../tests/nostr/relay_store_double.coax";   ->   #load "../lib/nostr/store.coax";
```

This change was not made here, because claude-relay-r1b owns `coax/relay/` and
`relay_server.coax`.

## Checks run (compiler coax0 0.23.0, `coax/bin/coax` SHA-256 `03498a6c…ab93`)

- `coax test coax/lib/nostr/store_core.coax`: 46/46.
- `coax mutate coax/lib/nostr/store_core.coax --jobs 3 --shard I/4`, then
  `--merge`: 581 mutants, 581 killed. The first full run (585 mutants) had 7
  survivors. Four of the tests started every record at arena offset 0, and
  one redundant reload in the duplicate path could not be observed (now
  removed). The tests were sharpened, and the rerun on the final file
  (SHA-256 `6a75b81d…c4d1`) killed every mutant. The shard records and the
  merge report are in
  [data/nostr-store-r1-2026-10-29/](data/nostr-store-r1-2026-10-29/).
- `coax test coax/lib/nostr/store.coax`: 303/303.
- `coax test coax/tests/nostr/store_verified_checks.coax --steps 400000000`:
  351/351.
- Swap check: a scratch copy of `coax/relay/` was made with the one `#load`
  line changed to `store.coax`, and `coax/tests/nostr/relay_session.py` ran
  against its ASan+UBSan build (127.0.0.1). It printed `relay_session: ok`
  with no sanitizer report: publish, duplicate, bad signature, replaceable
  and addressable ordering, NIP-40 expiry and serving, limits, CLOSE and
  closes, all through this store. The scratch copy was deleted and is not
  committed. As with the double, a restart serves 0 events.

## The prior handoff and the reindex gate

The stopped worker (codex-nostr-profiles) wrote that the reindex step's first
full mutation run had two survivors, and that its corrected tests were never
rerun as a full gate. That reindex source, its uncommitted work and
`docs/verification/2026-10-29-nostr-store-handoff.md` are not on this machine
or on `origin/main` (only on the Mac). So the gate could not be rerun. This
store was rebuilt from `3a038c87ca` (ingress) and the board notes instead.
`store_admit.coax`'s `ns_admit_one` and the frame/snapshot slices are
unchanged. The in-memory store does not use the NSLG frame. The durable file
should reuse it.

## What is missing

- **Trusted recovery (#966b).** Nothing here survives a restart. A durable
  file replayed with re-verification would still let an older, individually
  valid file resurrect deleted or replaced events, as the earlier prototype
  showed. Restore stays incomplete until #966b supplies a trusted
  monotonic checkpoint.
- **Durable append-only file.** Decisions in Coax, file effects in C,
  re-verification of every original on replay.
- **NIP-09 deletion and tombstones** (R2, and #f539's acceptance item on
  "later deletion ... preserve tombstones").
- **#f539 acceptance not met in full:** checked and release native and Wasm
  fixtures for the verified dedup path, ASan/UBSan fixtures of the store
  itself (the relay sessions above are the only sanitizer evidence), and an
  independently signed duplicate with a *different* signature (the encodings
  here share one signature; no test signer was added). The store's quota is
  its arena (`RSD_ARENA`, 4 MiB), not `store_dedup_policy`'s 1 MiB log quota,
  so the core uses `ns_original_before` directly rather than
  `ns_duplicate_decide`.
- `store.coax` is glue checked by tests, not by mutation, for the reason
  above.
