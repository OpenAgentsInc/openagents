# Coax Nostr epic: handoff

## Stop handoff — 2026-10-29

The owner asked for a stopping point and handoff. The previous execution
update below is historical: the Nostr workers have stopped and released their
claims. Use the live Nostr queue before taking work; it contains reopened tasks
and signed notes that supersede older completion claims. Do not add sats.
The phone sign-in worker owns iOS key custody, the oa-api sign-in challenge and
phone-link endpoints, `/signin`, and its TestFlight build. Do not take those.
The QR dependency #503d is on main (`78e2095bc1`); the phone fast path and the
full NIP-46 work remain distinct.

### Active validator issue #161a

#161a remains claimed while a full-reference event mutation gate is running.
The prior final run failed: 4,301/4,302 mutants killed; the surviving change
made `N_SignedEvent_decode` require minimal wire encoding. A new test covers
a valid non-minimal envelope: 195/195 checks pass and an isolated diagnostic
kills that mutant, but that diagnostic is not a full mutation pass. A fresh
full-reference run is required on the changed test source. Do not mark it done
until that gate, independent review and the other acceptance requirements pass.
The project publisher and final completion release must follow the normal queue process.

The changed runtime rejects negative internal depth (`7ed6d17885`), and the
generator removes only a per-message limit redundant with the shared 1 MiB
cap (`8179746a5d`). The latter's Python-free bootstrap checks and fingerprints
cap parity, and its scope, independent review and tests are documented in
`docs/verification/2026-10-29-proto-generated-validator-review.md`. The full
final FI reference run killed 3,162/3,162 mutants; final event/FI fixtures passed
checked/release native, Wasm, ASan/UBSan and iOS Simulator, including protoc and
independent signature controls. Projections compare borrowed original bytes and
do not prove signatures, membership, identity, freshness or authorization.
They require live, stable, physically valid borrowed storage. The event adapter
has narrower JSON/tag limits than the generic descriptor; the FI adapter has
narrower header, payload and audience limits. Nostr kind 0..65535 is enforced
by the original event parser in this adapter, not by the generic uint32 DTO.

Uncommitted projection sources and reproducible test fixtures are in
`/tmp/codex-event-final` (base `8179746a5d`) and in the verified local archive
`/Users/christopherdavid/.codex/handoffs/nostr-validator-161a-20261029/projections-envelope-check.tar.gz`
(SHA-256 `fc663357726b43cf5ce568ccc459a5aa0150febfb6468ce0595ab92395b6e2f8`).
The adjacent README and manifest include recovery instructions and per-file
hashes; this is not a remote backup. Key source/compiler hashes are
the older `/tmp/coax-event-final-key-hashes.txt` and `/tmp/coax-fi-final-key-hashes.txt`;
the updated event source SHA-256 is
`1946d02e0cd1cbf2e816ad3b2e7e0e773a3118c1a93e7686da6bec9ce4ce8c29`.
Logs: `/tmp/coax-event-envelope-reference.log` (running),
`/tmp/coax-event-final-reference.log` (failed), `/tmp/coax-fi-final-reference.log`,
`/tmp/coax-event-final-interop.log` and `/tmp/coax-fi-final-interop.log`. The
first event run failed with 130 survivors in
`/tmp/coax-required-policy-event-full-mutation.log`; do not present it as a
pass. #ef04 tracks versioned numeric-range semantics; #5869 tracks ordinal
error precedence (`LIMIT | WIRE` can be misreported as `DUPLICATE`). Both are
separate from signature or authorization and remain open.

### Store and project page

Single-event public verified ingress is on main (`3a038c87ca`), and the scoped
one-representation dedup policy is on main (`162749d294`). Neither is a full
store. #8eb5 remains open and depends on #f539 verified-ID dedup and #966b
trusted recovery. Two simultaneous real verifications exceeded the unchanged
20M evaluator budget; never replace verification with a caller-supplied flag.
Restore remains unavailable without a validated real rollback-resistant
checkpoint and a cross-process publication fence. The unconnected recovery
model is on main (`c1fe34e160`), not a real provider.

#8eb5's original reindex mutation run failed 3,945/3,947; corrected test-only
work has not passed a full rerun. Four other long gates were stopped on the
owner's request, without results. The incomplete source/evidence is preserved
in `/Users/christopherdavid/.codex/handoffs/nostr-store-8eb5-20261029-chwcgz2b/`
(`nostr-store-8eb5.tar.gz` SHA-256
`1fa1d74bee160bd3c9b27d09d243ef7cb5d91ffa7d8d2753a520f74384ae4aa8`);
the adjacent README/manifest give recovery and per-file hashes. Do not assume
the archive is merged, tested as a complete store, or remotely backed up.

#0845 remains open: the project runner has a local WIP checkpoint
`3d66c2c0dcd1de22a35e6cb3b5ac9bd1653aac90` and durable local bundle, source,
logs and recovery instructions under
`/Users/christopherdavid/.codex/handoffs/nostr-runner-0845-20261029`.
Its handoff is `docs/handoffs/2026-10-29-project-runner-0845.md` in the
preserved runner worktree. The original-order v3 mutation run was stopped at
7,151/7,513 with 1,228 survivors; v6 was stopped at 125/7,691 with one
survivor. V6's 209 checks and raw Fold native/Wasm matrix passed, but the
real-worker overlay matrix had a harness error and its saved fix was not
retested. Unicode formatting, slug validation, missing foreign roots and
independent root caps have isolated fixes; do not claim they are shipped or
mutation-qualified.

### Audit and releases

The secret-auditor's checked-cast and checked-arithmetic false acceptances
were repaired conservatively on main (`582948bdac`, `c1d2f01448`) and their
reference mutation evidence recorded (`124bf63997`, `2acd95f5a2`). Old
aggregate and scalar secret-source acceptances remain suspended; affected
tasks were reopened. The experimental signer uses a variable-time verifier on
secret-derived candidate data before an independently justified publication
boundary; #89db tracks that separate issue. No signer approval, key recovery,
target timing or production-secret conclusion is established.

An automatic cybersecurity review stopped the signer source-model investigation
as a potential cybersecurity risk. #4828 was noted and released; do not treat
that investigation as complete or bypass the restriction.

The normal release queue was stopped after its in-flight release completed:
`af1e47acdf` passed staging and production, log
`/tmp/coax-held-51f4-claim-release.log`. `91aa0186d8` gates the Haunted House
walk on a completed nonempty scene frame, and `238f697bec` updates the galaxy
reference for the intentionally removed Coding tag; the earlier intermittent
0.3%-lit production frame was not reproduced or proven. Both previous failures
rolled back or stopped at staging, and thresholds were not loosened. No release
process from this queue remains running. The remaining held claim/done release
labels are in `/tmp/codex-nostr-release-queue.sh` after `51f4-claim`, with
additional `966b-claim` and `f539-claim` in
`/tmp/codex-nostr-extra-releases.txt`. Those are obligations, not completed
releases. At this host each full release took about 15 minutes, not 15 seconds.
Before resuming, verify the current main's compiler provenance; release one at
a time and stop on the first failure. Never rerun unrelated code checks merely
because another main commit arrived.



On 2026-10-28 the owner moved the whole Coax Nostr epic (the `nostr`
initiative on https://openagents.com/projects/coax) to the Codex chat that
reviewed it (`codex://threads/01a1841b-45df-7ab0-ba74-2b5b3fc3d900`). That
chat owns the plan, the work file and the queue order from here. The Claude
queue worker has stopped. The live-page worker (`claude-live-1`) finishes
#c29a and then stops.

## Historical execution update

`codex-nostr` owns the taskmaster and root worker loop. Other active workers are
`codex-nostr-codecs`, `codex-nostr-profiles` and `codex-nostr-vectors`; use the live
queue, not the historical lists below, to avoid duplicate claims. The shared
checkout stays untouched; isolated worktrees rebase/push only their own commits.

Since the original handoff: ct_view now reachesE0409 safely (`cfc36f3ab7`);
page capacity is256; TS pagination and Coax bounded catch-up no longer mistake
short capped responses for complete history. Vectors, native vector runner,
profiles, byte helpers, wrapping intrinsics, SHA-256, hex/base64, Bech32/NIP-19,
entropy, random import and byte storage have shipped in scoped slices. Structural
JSON admission is shipped (`a165c9bc26`); decoder mutation closure is complete
in `2bf22a148e` (971/971 mutants). Coax0.16 adds `mul_wide`; field multiplication,
inverse/root, scalars, HMAC/HKDF, PBKDF2, secure wiping and integer-profile JCS
retain separate correctness evidence. The public timing harness records actual
million-observation Mac/Linux/browser controls without approving secret use.
Actual signed queue comments/plans/statuses and
`coax/nostr.md` record evidence and remaining gates.

Read-only event verification is still distinct from signing. Full profiles,
production secret approval, independent review, native/iOS/Wasm target evidence
and domain authority integration remain open until their own checks pass. No
amounts or rewards have been added. Test only affected changes; an unrelated
remote commit does not justify repeating the compiler/app suites.

## Phone sign-in fast path landed (2026-10-28, claude-signin)

The owner approved a fast path for m9 instead of NIP-46 first: the iOS app
holds a Nostr key (Keychain, device-only) and approves a one-time QR from
https://openagents.com/signin with a NIP-98 signed request; the page then
gets a site session bound to that key. Members link a phone to a private
project with LINK A PHONE or by opening an invite link on the phone. See
`docs/phone-sign-in.md`.

- Taken: #bc0b ios-key-custody (claimed by claude-signin). Signing uses
  bitcoin-core libsecp256k1 compiled into the app for now; swap in the Coax
  secp256k1/Schnorr when it lands, against the same BIP-340 vectors.
- Used: #503d coax-qr-encoder (Codex) draws /signin's and LINK A PHONE's QR.
- Left waiting: the NIP-46 tasks (nip46-signer, nip46-session-lifecycle,
  nip46-client-page, ios-scan-to-sign-in), server-nip98-session's Coax
  verifier and server-session-lifecycle, web-sign-in-page and
  phone-sign-in-e2e as written. The fast path covers their user-visible
  outcome with the C verifier in oa-api; the tasks stay for the Coax and
  NIP-46 versions.

## Where things are

- Design: `coax/nostr.md`. Protocol for projects and tasks:
  `docs/protocol/2026-10-28-projects.md`.
- Work file: `apps/openagents-web/projects/coax/work/nostr.json` (schema in
  that directory's `README.md`). The Coax roadmap (R0–R12) is a separate
  initiative in `coax.json` and stays with the `codex-coax` worker.
- Tools: `coax/web/project/` (`publish.ts`, `snapshot.ts`, `details.ts`).
  `QUEUE.md` there is the worker loop. Every verb takes a slug or alias.
- Project key: `~/work/.secrets/openagents-project-coax.key`. Never print or
  commit it.
- Releasing: `coax/web/release.sh` from the repo root (about 15 s). It
  builds a clean worktree of `origin/main`, re-reads the relay, deploys
  staging, smokes it, promotes and smokes production, and rolls back on its
  own if production fails. From a worktree, copy `coax/bin/coax` in first.

Release now also walks the real pages against approved headless goldens. It
can take longer than the original fifteen-second estimate, especially behind
another release. Stop on a failed gate, inspect the owner change, and retry
only after its cause is fixed; never bypass or silently update another owner's
golden. Independent work can continue while a valid release holds the lock.

## Phone fast path coordination

The owner assigned a Claude agent the temporary NIP-98 phone sign-in fast path:
one-use `/signin` challenge, an iOS signature, the page session and member phone
key linking. That agent exclusively owns `ios-key-custody`, the challenge/link
API endpoints, `/signin` and TestFlight. Do not take those tasks or edit their
implementation. Full NIP-46 and its lifecycle remain in the Nostr epic; the fast
path does not satisfy those later contracts.

QR source shipped in `78e2095bc1`. Load `coax/lib/qr/qr-memory.coax` and call
`qr_encode(input,out)`. A70-byte URL yields size37/count1369; a5929-byte caller
buffer covers every supported version. Output is row-major0/1; render a light
four-module quiet zone and dark Coax quads without smoothing. Native/Wasm exact
reference grids and the actual iOS Vision rendered-frame scanner pass. Physical
camera optics remain part of the device sign-in owner step, not an inferred
result. See `coax/lib/qr/README.md` for the API and scoped evidence.

## Original handoff snapshot

These lists are historical. The live queue and current execution update above
supersede their status; retain them as the starting evidence.

### Done

| Task | Commit |
|---|---|
| #47b1 host-socket-pipe: WebSocket byte pipe | df008f96a4 |
| #d223 nostr-json: bounded JSON pull parser and writer | 8f23a77f35 |
| #a7e7 compound assignment through a view emits valid C | e2f8432ea2 |
| #b17d value_barrier for constant-time code | b9a5f216f0 |
| Queue tooling: claim, release, note, show, done, plan | fdd3a57565, e0af56a0a3 |
| Click a task for its detail; `#alias` deep links | 5225d1aa5b |

#b17d checked: `coax/build.sh` and `tests/constant_time.py` (3 tests) pass on
main. The checker's `#test`/`#forall` 554 passed, and mutation killed
3777/3777.

### In flight

- **#c29a project-page-live** (`claude-live-1`): the page reads its
  project, tasks and order from the relay. The WebSocket fold has landed.
  The worker finishes this task, releases its claims and stops.
- **WORK_DETAILS from the live fold:** today the detail text comes from the
  build-time snapshot. When the live fold refills the tables it has to fill
  `w[0].details` too. The escaping is in `coax/web/project/details.ts`.

### Ready next, by queue order

These Nostr-side tasks are ready: #993f crypto-bytes, #ef11
crypto-vectors-pinned, #5c3a nostr-hex-base64, #2265 secp-libsecp-harness,
#5923 nostr-jcs, #d0d7 host-random-import, #44fe host-storage-pipe and #503d
coax-qr-encoder. The intrinsics #0ad5 (wrapping ops) and #cead (mul_wide) gate
the secp field work. The rest waits on these. `bun publish.ts list` is the
truth.

### Known problems

- **`coax/tests/run.sh`: 88 pass, 1 fails.** `tests/fail/ct_view.coax`
  expects E0409, but E0383 (returned borrow of a local array, added in
  9d0f246ba1) now fires first. The test needs a case that still reaches E0409
  without a local borrow. Don't just change the expectation.
- **Page item limit:** the queue's combined task count passed 128 items.
  Raise the bound with a test that nothing is cut off, as the Codex review
  planned.

## Lessons from the queue worker

- **Runtime helpers stale the seeds:** generated C carries the whole
  runtime, so a new runtime helper makes every bootstrap seed stale. Emit
  helpers only into programs that use them.
- **Checker seed refresh:** refreshing it runs mutation (about 6 min). New
  checker code needs its own `#forall`.
- **Shared checkout:** other agents use the main checkout. Never stash, reset
  or autostash their files. Push and run the tools from a worktree of
  `origin/main`, and copy `state/coax.json` in when the tools need it.

## Rules that still hold

- **No sats on tasks:** no rewards or amounts on tasks.
- **Pages are Coax:** everything on a Coaxial page is drawn in Coax. A page's
  hidden `<main>` is just the name and one link, and no comments ship to
  browsers.
- **Bitcoin-only rails:** no WorkOS. Sign-in is Nostr with phone QR (m9).
- **No hand edits to order:** never renumber or reorder the work file by
  hand. Use `publish.ts move` and `respread`.
- **Done means checked:** mark a task done only when its acceptance checks
  pass. Otherwise note why and leave it open.
