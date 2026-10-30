# Coax Nostr epic: handoff

## API deployment integrity handoff — 2026-10-30

A separately scoped update hardens the existing C-image deployment. It captures
and tests committed API sources in a detached worktree, obtains an immutable
image digest from the successful Cloud Build result, and verifies project,
region, service UID, ready revision, resolved image, exclusive traffic and
routed health identity before promotion. Staging and production receive the
same digest. API health now reports configured build/release plus Cloud Run
service/revision; it is still a database health endpoint. Existing phone and
NIP-98 behavior and the C Dockerfile remain unchanged.

The full native API suite passed 954 service checks under ASan/UBSan, alongside
standalone bounded-JSON, fake-gcloud/curl rollout and real-Git detached-source
fixtures. A build-only Cloud Build of the exact C source context succeeded; no
Cloud Run staging or production deployment was performed. Full evidence,
source/fixture limits, build ID and digest are in
`docs/verification/2026-10-30-api-deploy-identity.md`. A verified archive of
the build context, source manifest and sanitized build result is local at
`~/.codex/handoffs/nostr-api-c-deploy-20261030/`, not a remote backup. The first
full-suite attempt was invalid because its shell script was edited while it ran;
the recorded final rerun used frozen inputs and passed.

This does **not** establish that a real digest-based Cloud Run rollout works,
authenticate a source/build attestation, or authenticate a Coax compiler
artifact. #f96d remains open: the separate Coax-linked API prototype is in
`~/.codex/handoffs/nostr-api-bridge-20261029/`, and its Linux bootstrap/artifact
limits are in `~/.codex/handoffs/nostr-api-linux-bootstrap-20261029/`. The
source-only Coax image previously timed out at the build limit; do not replace
the working C Dockerfile with it absent a demonstrated, reviewed image flow.
Take a fresh worktree, verify the live queue, and continue from those artifacts;
do not infer Coax Nostr authorization or production secret approval.

## Restart — claude-nostr, 2026-10-29 (Linux host)

The owner restarted the epic with one worker, `claude-nostr`, on the Linux
machine (worktree `~/oa-coax-nostr`, branch `coax-nostr`, from
`origin/main`). The project key is on this host, so claims, plans, notes and
completions are signed on the relay as QUEUE.md describes. The live queue is
the source of truth.

The `codex-nostr-profiles` and `codex-nostr-codecs` claims still show on the
relay (#79d5, #f07b, #71e7, #0f43, #6dc6, #3653, #12bd, #6652, #46bd, #d235,
#bd06). This worker leaves them alone until the owner or taskmaster releases
them.

### Host facts

- **Wasm on Linux.** `~/.cache/coax-wasm-toolchain` holds nix clang 21,
  `wasm-ld` and wasi-libc 27, with compiler-rt builtins. To use it, set
  `COAXIAL_CLANG=$T/bin/clang COAXIAL_SYSROOT=$T/sysroot
  COAXIAL_RESOURCES=$T/resource PATH=$T/bin:$PATH`, where `$T` is that
  directory. Node WASI runs the result. Not every fixture script reads these
  variables yet. Some still hardcode `/opt/homebrew`.
- **No iOS here.** There is no Xcode or simulator on this host. Any acceptance
  check that needs an iOS target stays open for the Mac.
- **No site releases from here.** `coax/web/release.sh` builds the pages with
  the Mac toolchain paths, and its goldens are Mac renders. A release attempted
  after #0af2 was stopped during its compiler bootstrap, before any build or
  upload. The page catches up at the next release from the Mac. See
  `NEEDS_OWNER.md`.

### Running record

| Task | State | Commit | Evidence |
| --- | --- | --- | --- |
| #0af2 crypto-secret-lint | done | `fc59b9c7a1` | Scalar policy revalidated on the rebuilt compiler: 41/41 checks; reference mutation 749/749 (8,891 checks, nothing skipped); `test_secret.py` 7/7; new `test_revalidate.py`: 400 generated programs, 257 accepted, emitted-C guard scan and checked/-O2 execution, 12 refused controls. Finding: the corrected policy refuses `cb_mask` (checked multiply on a secret), and `test_secret.py` had still asserted acceptance. `docs/verification/2026-10-29-secret-scalar-revalidation.md` |
| #0af1 server-nip98-session | skipped (signed note) | — | Overlaps the phone sign-in worker's oa-api endpoints. Needs a taskmaster or owner decision |
| #2ff9 nostr-proto-adapters | done | `5bb765142a` | The event and FI projections from #161a meet the round-trip, mismatch and forged-authority criteria. On Linux: Coax checks 195/195 and 210/210; protoc, checked/release native, ASan/UBSan and Wasm fixtures pass; FI reference mutation 3,162/3,162. The event mutation rerun was stopped at 546/4,306, which is not a pass; #161a's 4,306/4,306 on the same source stands. HOST stays with #3d81. `docs/verification/2026-10-29-proto-adapters.md` |
| #092b crypto-byte-load32-total | open, released | `5768ad2d86` | `cb_load_be32/le32` now use a fixed four-byte `u32` accumulator with no checked cast. ABI unchanged. Evidence: checks 13/13; reference mutation 146/146; z3 30 unsat proofs and 8 sat controls; 6,216 vectors on each of checked/release native and Wasm; an independent review; Coax checks of all 16 direct callers. **Open:** the callers' Python target suites (SHA-256, ChaCha/XChaCha, Poly1305, PBKDF2, scrypt, ECDH, Schnorr) need the Mac (`xcrun`, Homebrew LLVM, the fuzz build cache). `docs/verification/2026-10-29-load32-total.md` |

### Why the other ready tasks are not taken (2026-10-29)

- **#4828:** stopped by the automatic cybersecurity review. Not resumed.
- **#0af1:** overlaps the phone sign-in worker. It needs a taskmaster or owner
  decision.
- **#0845:** depends on the unshipped #c969 draft (`project-verified.coax`,
  `project-verify-queue.coax`, `project-scope.coax`) and the runner WIP. Both
  exist only in the Mac archives, not on main or on this host.
- **#f539:** acceptance needs deletion and tombstones from the store owner,
  #8eb5. #8eb5 waits on #f539 and #966b, and its WIP is archived on the Mac.
  That is a dependency loop for the taskmaster to break. One option is to
  land the store skeleton first.
- **#966b:** acceptance needs an actual, independently trusted
  rollback-resistant checkpoint provider. Choosing and provisioning it is an
  owner decision. Only the mock-provider half could be done here.
- **#89db:** acceptance needs source-bound secret-flow evidence for the
  signer's self-verifier. That is the kind of signer source-model analysis the
  review stopped under #4828. It needs an owner decision before anyone takes
  it.
- **#ef04, #5869:** both change the generated protobuf importer. Their
  acceptance needs the affected iOS consumers built and run
  (`verse/home.coax`'s generated configuration codecs), and this host has no
  Xcode. Their designs (`docs/research/nostr-proto-*-design.md`) are drafts,
  not accepted API decisions. They are best done on the Mac, or after the
  owner accepts a Linux-only partial.

## Stop handoff — 2026-10-29

The owner asked for a stopping point and handoff. The previous execution
update below is historical: the Nostr workers have stopped and released their
claims. Use the live Nostr queue before taking work; it contains reopened tasks
and signed notes that supersede older completion claims. Do not add sats.
The phone sign-in worker owns iOS key custody, the oa-api sign-in challenge and
phone-link endpoints, `/signin`, and its TestFlight build. Do not take those.
The QR dependency #503d is on main (`78e2095bc1`); the phone fast path and the
full NIP-46 work remain distinct.

### Generated protobuf validator issue #161a — done

#161a is resolved on the relay and on main at
`d42b3c1c2ff45fd04fb18e77bd4dd4aa38ac3b76`. The actual generated consumer
closures passed full-reference mutation without waivers: event 4,306/4,306
and FI 3,162/3,162. Their Coax checks passed 195/195 and 210/210. The tested
inputs, source identities, commands, independent review and native, Wasm,
ASan/UBSan, protoc and iOS Simulator fixture results are recorded in
`docs/verification/2026-10-29-proto-consumers.md` and its adjacent input
manifest. The compiler bootstrap remains Python-free.

The prior event reference run failed 4,301/4,302: a non-minimal envelope
was not tested through the ordinary decoder. The final tests cover acceptance
by the ordinary decoder and rejection by the minimal decoder; only the full
rerun above is the acceptance gate. Earlier partial failure logs remain in
`/tmp/coax-event-final-reference.log` and
`/tmp/coax-required-policy-event-full-mutation.log`. The final complete traces
are `/tmp/coax-event-envelope-reference.log` and
`/tmp/coax-fi-final-reference.log`; they and the fixture logs are also in
`/Users/christopherdavid/.codex/handoffs/nostr-validator-161a-20261029/`.
That directory is local, not a remote backup. The sources and verification
summary are committed on main.

The runtime rejects negative internal depth (`7ed6d17885`), and the generator
removes only a per-message limit redundant with the shared 1 MiB cap
(`8179746a5d`); bootstrap checks and fingerprints enforce cap parity. The
projections compare borrowed original bytes and do not prove signatures,
membership, identity, freshness or authorization. They require live, stable,
physically valid borrowed storage. Event JSON/tag and FI header/payload/audience
limits are narrower than their generic descriptors. Nostr kind 0..65535 is
checked by the original event parser, not the generic uint32 DTO.

#ef04 tracks explicit schema numeric-range semantics; #5869 tracks ordinal
error precedence (`LIMIT | WIRE` can be misreported as `DUPLICATE`). Both
remain open and are separate from signature or authorization. The projections
are experimental and unverified, and they do not complete the rest of the
Nostr epic or authorize secret handling.

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

The previous serialized release queue was stopped after its in-flight release:
`af1e47acdf` passed staging and production, log
`/tmp/coax-held-51f4-claim-release.log`. After #161a completed, a new serialized
release of `ced1bc1552` passed staging and production; the published project
index confirms #161a done in `d42b3c1c2f`. Its log is
`/tmp/coax-nostr-161a-done-release.log` and rollback snapshot is
`20261029T162422Z`. No release or validator process from this effort remains
running. The newer release includes the live relay's latest events; the older
individual queued snapshots were not replayed. `91aa0186d8` gates the Haunted
House walk on a completed nonempty scene frame, and `238f697bec` updates the galaxy
reference for the intentionally removed Coding tag; the earlier intermittent
0.3%-lit production frame was not reproduced or proven. Both previous failures
rolled back or stopped at staging, and thresholds were not loosened. The
historical held claim/done release labels are in `/tmp/codex-nostr-release-queue.sh` after `51f4-claim`, with
additional `966b-claim` and `f539-claim` in
`/tmp/codex-nostr-extra-releases.txt`. Those individual releases were not run;
check current relay and site state before deciding what, if anything, remains to publish. Release one at a time
and stop on the first failure. Never rerun unrelated code checks merely
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
