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
3. **Possible compiler aliasing difference.** A `cast(string)` of a slice is
   copied under `coax test` but aliases the buffer under `coax run`
   (chapter 17).
4. **Flat namespace collisions.** Procedure names and local names share one
   namespace. Chapter 98 renamed a local in `secp_field.coax` (`fold` to
   `reduced`) and parameters in `schnorr.coax` (`event_id` to `nostr_id`)
   so the project fold and the verifier load together.
5. **NIP-49 scrypt needs a 64 MiB stack.** Coax has no heap, so log_n 16
   puts a 64 MiB array on the stack. It crashes at the default 8 MiB and
   can't run on macOS, whose stack limit is 64 MiB (chapter 47).
6. **Relay modules not wired.** The running relay calls none of these:
   `store_index`, `store_delete`, `relay_rate`, `nip45`, `nip50`
   (`filter_read_search`), `nip70`, `nip77`, `nip13`, `nip86`, `nip43`,
   `nip29`. Chapter 103 lists the wiring each needs. The Coax store also
   holds at most 2,048 events, which blocks a cutover of the production data.
7. **Hashing and phrase limits.**
   - The one-shot SHA-256 stops at 1 MiB, so `nip94.coax` refuses larger
     files.
   - `nip06_key` derives a key from a phrase with a bad checksum.
   - `bip39_entropy` reads 12-word phrases only.
8. **Website checks not run.** `apps/openagents-web/project-checks.coax`
   loads `nip22.coax`, which gained the `N22_KINDS` refusal (chapter 52).
   It wasn't run, because it needs the generated `projects/baked.coax`.
9. **Pinned NIP digest is stale.** Commit `b01de252f7` added `NIP-KEYS.md`,
   which changed the OpenAgents NIP folder digest. `profiles.py`'s pin test
   now fails (now `fb7563e7…`, pinned `2fb1f51f…`). The new text needs a
   review before it is re-pinned.

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

- **libsecp256k1 comparisons:** chapters 11 and 14.
- **10,000-signature run (chapter 13):** checked only by the Python oracle.
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
