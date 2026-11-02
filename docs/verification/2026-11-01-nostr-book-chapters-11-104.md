# Nostr book, chapters 11 to 104 (2026-11-01)

On 2026-11-01 every chapter of *Nostr From Scratch* from 11 to 104 was
written as a teaching chapter, one commit per chapter on `main`. This record
lists what was checked at the end, what each chapter postponed, and what the
work found outside the book.

## Status at the end

- **Implemented:** 102 of 104 chapters (1 to 102).
- **Partly implemented:** 2 chapters.
  - **Chapter 103:** the importer, the equality check and the shadow
    comparer exist and are checked. The cutover and the rollback rehearsal
    need owner sign-off (`NEEDS_OWNER.md`).
  - **Chapter 104:** NIP-KEYS signing (ECDSA/BIP-143, BIP-341, BIP-322) is
    not built. Root-only mutation of `coax/lib/nostr/nipkeys.coax` finished
    later at `a9ce49d714`: 1963 of 1963 mutants killed across 40 shards.
- `python3 psionic/tests/book.py --book nostr` (the structure check) passes.

## Final example run

All 102 examples on `origin/main` at `784cbdb9bf` ran in debug and release:
204 runs, 0 failures. Each run's exit status, stderr and stdout matched
`examples.json` and its fixture.

- **Compiler:** the cached release build `bcefdc7fa553…` (coax0 0.46.0,
  built 2026-11-01 10:18 −0600).
- **How it ran:** the same `run_one` procedure as `book.py`, in 8 slices,
  because one whole-book run takes longer than 15 minutes on this machine.
- **Not covered:**
  - Chapters 103 and 104 landed after `784cbdb9bf`. Their agents ran their
    examples in both modes.
  - Compiler commits after 10:18 (`#3696`: `fe6d83a83e`, `4b16dd812e`,
    `acf753674a`) were not covered. `coax/build.sh` timed out twice at
    14.5 minutes in the stage-0 bootstrap.

## Findings outside the book

1. **x402 settlement has no expiry check.** The live route settles in SQL
   (`inference.topup_settle`) and checks no invoice expiry at settlement.
   `coax/lib/x402/settle.coax` (chapter 86, `#authority`, 189 of 189
   mutants killed) checks expiry plus a 60-second grace, but the route
   doesn't call it.
2. **Compiler budget regression.** Cached build `63dbbb21aaac` reports
   `E0381: type checker storage or nesting budget exceeded` on
   `psionic/lib/mesh/totals.coax` (for example `psionic/examples/mesh-totals.coax`).
   That file is unchanged since 2026-10-30, and older compilers accept it.
   Fixed in the compiler (0.47 follow-up): its result holds 224 strings,
   and the borrow leaves gave each array element its own leaf past the
   64-leaf budget; a wide array is now one leaf. Regression:
   `coax/tests/pass/wide_borrow_result.coax`.
3. **Possible compiler aliasing difference.** A `cast(string)` of a slice is
   copied under `coax test` but aliases the buffer under `coax run`
   (chapter 17). Aliasing is correct (spec: `cast(string)` borrows); the
   compile-time evaluator copied. Fixed in the compiler (0.47 follow-up).
   Regression: `coax/tests/pass/cast_string_borrows.coax`.
4. **Flat namespace collisions.** Procedure names and local names share one
   namespace. Chapter 98 renamed a local in `secp_field.coax` (`fold` to
   `reduced`) and parameters in `schnorr.coax` (`event_id` to `nostr_id`)
   so the project fold and the verifier load together.
5. **NIP-49 scrypt needs a 64 MiB workspace. Fixed for hosts.** Coax has
   no heap, so a Coax program that runs log_n 16 puts a 64 MiB array on
   the stack, which crashes at the default 8 MiB and can't run on macOS
   (stack limit 64 MiB). The library already takes the workspace from its
   caller (`nip49_scrypt_start`/`step` take `V` as a `[] u8`, and refuse a
   view shorter than `128 · r · N` bytes), so a host owns it.
   `coax/tests/nostr/ncryptsec_vector.sh` is that host: C that `malloc`s
   64 MiB, derives the published vector's key at log_n 16 and opens it to
   the published private key, and checks the one-byte-short workspace, the
   log_n 17 and the short-step refusals, in checked and release builds
   under ASan/UBSan (about 31 s each). The book's example still takes
   `KEY_NOSTR` from `hashlib.scrypt`, because `book.py` runs plain Coax
   programs with no host to supply a heap buffer (chapter 47).
6. **Relay modules not wired. Partly fixed.** When this record was written
   the running relay called none of these: `store_index`, `store_delete`,
   `relay_rate`, `nip45`, `nip50` (`filter_read_search`), `nip70`,
   `nip77`, `nip13`, `nip86`, `nip43`, `nip29`. Chapter 103's table says
   which are wired now and what the rest need.
   - Wired in `0a8fe18801`..`54f00c2e64`: `store_index`, `store_delete`
     (NIP-09, NIP-62), `relay_rate`, `nip45`, `nip70` and `nip13`.
   - The served NIP-11 `supported_nips` lists them: `[1,9,11,40,42,45,62,70]`,
     and 13 with `--pow` (`rr_nip11_pow`). Root-only mutation:
     `relay_route.coax` 438 of 438, `relay_server.coax` 344 of 344.
   - `nip50` wired: `rs_req_search` (relay_server.coax, root-only 347 of
     347), `rd_req`/`rd_req_filter`/`rd_sub_matches` in `relay_store.coax`
     over `n50_scan`; NIP-11 then lists 50. Searches scan the store; the
     word index is not used (it is rebuilt whole after any write).
   - Still not wired: `nip77`, `nip86`, `nip43`, `nip29`.
   - The Coax store holds at most 2,048 events, which blocks a cutover of
     the production data.
   - Not rerun: the Rust differential (`relay_rust_differential.py`) needs
     the Rust sandbox; its expected COUNT reply and its `supported_nips`
     note are out of date.
7. **Hashing and phrase limits. Fixed.**
   - Fixed in `ad37e4eb61`: the one-shot SHA-256 stopped at 1 MiB, so
     `nip94.coax` refused larger files. `n94_check` now streams through
     `sha256_init`/`update`/`final` in 1 MiB pieces (root-only mutation:
     343 of 343 killed).
   - Fixed after `ad37e4eb61`: Blossom's `nb7_fetch` and `nb7_upload`
     (`nipb7.coax`) used the same one-shot hash and refused bodies over
     1 MiB as `NB7_LIMIT`. Both now stream through `n94_digest`, checked
     with a 1,100,000-byte body against Python's `hashlib` (root-only
     mutation, three shards merged: 214 of 214 killed; chapter 57).
   - Fixed in `8cf39a8851`: `nip06_key` derived a key from a phrase with a
     bad checksum. It now refuses and zeroes the key.
   - Fixed in `8cf39a8851`: `bip39_entropy` read 12-word phrases only. It
     now reads 12, 15, 18, 21 and 24 words (`bip39_entropy_len` returns
     the byte count), checked against the Trezor 18- and 24-word vectors.
     No BIP-39 vectors are pinned under `coax/tests/vectors`, so the
     vectors are in the test itself.
8. **Website checks not run.** `apps/openagents-web/project-checks.coax`
   loads `nip22.coax`, which gained the `N22_KINDS` refusal (chapter 52).
   It wasn't run, because it needs the generated `projects/baked.coax`.
9. **Pinned NIP digest is stale.** Commit `b01de252f7` added `NIP-KEYS.md`,
   which changed the OpenAgents NIP folder digest. `profiles.py`'s pin test
   now fails (now `fb7563e7…`, pinned `2fb1f51f…`). The new text needs a
   review before it is re-pinned.

## Fixed after this record

- **`rs_sub_open`** (chapter 96) opened a subscription from a REQ array with
  no closing bracket. It now refuses any message `ms_read` refused as
  `-RS_FRAME` (`62880c1cbf`; root-only mutation: 344 of 344 killed).
- **Example timeouts:** `scalar-mult.coax` (chapter 10) and `ecdh.coax`
  (chapter 14) timed out at the default 60 s under parallel load. Both now
  have a 240 s timeout (`e306f403b4`).
- Finding 7 above, the hashing and phrase limits.

## Library changes the chapters made to existing code

- **`relay.coax`:** an `auth-required:` publish now waits as `RP_AUTH` and
  is sent again after AUTH (chapter 31; 578 of 578 mutants killed).
- **`relay_server.coax`:** its NIP-11 copy now equals the document the relay
  serves (chapter 37).
- **`nip22.coax`:** a comment whose parent kind is neither 1111 nor the
  root's kind refuses (chapter 52).
- **`nip19-core.coax`:** gained `nip21_decode_checked` (chapter 16).
- **`filter_read.coax`:** gained `filter_read_search` (chapter 41).
- **`store_index.coax`:** gained `nsi_query_limit` (chapter 40; 588 of 588
  mutants killed after the change).
- **`psionic/tests/book.py`:**
  - optional per-example `run_steps`, `timeout` and `stack_kib`;
  - the ecash name check (chapter 74).

## Published NIP examples that don't check out

Each of these is recorded in the chapter that found it:

- **Wrong event ids:** NIP-26, NIP-27, NIP-47 (the appendix info event),
  NIP-51, NIP-53 (the live stream), NIP-70, NIP-88 (the response), NIP-98,
  NIP-B0 and NIP-B7 (kind 10063).
- **NIP-26:** its signature doesn't verify, and its `created_at` is past the
  delegation's limit.
- **NIP-57:** the receipt's description hash doesn't match its invoice.
- **Not valid JSON:** NIP-35, NIP-59 and NIP-66 (kind 10166).
- **Placeholders and mismatches:**
  - NIP-CC has a placeholder signature and bad bech32 checksums.
  - NIP-64's PGN starts with non-PGN lines.
  - NIP-F4 names kind 10164 in its text and 10064 in its example.
- **Possible reference bug:** in the frame-limit case, `hoytech/negentropy`
  fingerprints the remainder from the wrong item (chapter 43). Not yet
  confirmed against a running implementation.

## Checks postponed

These earlier records are cited, not rerun:

- **libsecp256k1 comparisons (chapters 11 and 14):** done later on
  2026-11-01, with no mismatch
  ([2026-11-01-libsecp-crosscheck.md](2026-11-01-libsecp-crosscheck.md)).
- **10,000-signature run (chapter 13):** libsecp256k1 later verified 10,000
  Coax signatures and 300 events. Each was also equal to libsecp256k1's own
  signature
  ([2026-11-01-libsecp-crosscheck.md](2026-11-01-libsecp-crosscheck.md)).
- **Secret probe (chapter 13):** the memcheck gate exempts three procedures.
- **Entropy (chapter 12):** the Mac, iOS and Chrome runs.
- **Mutation, sanitizers and the 10,000-entity run:** chapters 15 and 16.
- **Full mutation (`COAX_MUTATION_FULL=1`):** not run on any new file. Each
  new `#authority` file had root-only mutation with zero survivors,
  `nipkeys.coax` included (`a9ce49d714`).

## Plan lines rewritten

Chapters 14, 47, 101 and 102 changed their "Done means" line in
`nostr/book/PLAN.md`:

- **Chapter 14:** an invalid Wycheproof key whose only fault is its y can't
  be refused after x-only reduction.
- **Chapter 47:** the log_n cap is 18, not 20.
- **Chapter 101:** "offline, on a fixed clock". The presence latency uses an
  assumed 40 ms per hop.
- **Chapter 102:** the offline agent path. Moving the hosted runner to this
  stack in staging is not built.

The owner should confirm the rewrites for chapters 101 and 102.
