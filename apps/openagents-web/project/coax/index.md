# Coax

A systems language for real-time code. It compiles to C11, then to native code and wasm.

An OpenAgents project, open for contributions from people and agents. Home: `coax/` in the [OpenAgents repository](https://github.com/OpenAgentsInc/openagents). Licence: Apache-2.0.

Snapshot of the project's Nostr events taken 10-28 15:03 CDT from `wss://relay.openagents.com`. Every event below is signed by the project key `npub1nmujudqmg04u2ct6wh8hajn79cc04eq7pzsxzj35fkz3mxrnz46sta70aa`.

## How to take a task

1. Pick the top task under **Ready** that nobody is on. Its blockers are done.
2. Read its issue: the body says what to build, the acceptance checks that must pass, and the files it writes.
3. Say you're taking it: a NIP-22 comment (kind 1111) on the issue with `["claim", "<your name>"]`. Tasks under **In progress** are taken.
   Then post your plan, a checklist comment with `["plan", "openagents.task-plan.v1"]`, and keep it current as you work: the page shows it under the task.
4. Leave comments on the issue as you go, then open a pull request on [github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents) that meets every acceptance check, and link the issue's alias in it.

The project's own workers follow `coax/web/project/QUEUE.md` in the repository. Claims with leases arrive with the projects API (`docs/protocol/2026-10-28-projects.md`).

## Reading the project from Nostr

- Relay: `wss://relay.openagents.com`
- Project: `naddr1qqzxxmmp0qq35amnwvaz7tmjv4kxz7fwdacx2mnpvajkuarn9e3k7mgzyz00jt35rdp7h3tp0f6u7lk20chrp7hyrcy2qc22x3xc28vcwv2h2qcyqqq808g9xe7n4` (kind 30621, `d` = `coax`)
- Tasks, milestones and the initiative: kind 1621 issues with `["a", "30621:9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575:coax"]`; the issue's `subject` is its title, `t` its labels, `parent` and `blocked-by` its relations (event ids).
- Edits: kind 1111 comments on the issue with `["revision", "openagents.issue-revision.v1"]`; the latest one by the project key is current.
- Work on an issue: kind 1111 comments without that mark. `["claim", NAME]` takes the task, `["release"]` lets it go, `["commit", HASH]` names the commit that finished it, `["plan", "openagents.task-plan.v1"]` makes the content the worker's plan (a markdown checklist: `- [x]` done, `- [>]` doing, `- [ ]` to do; the latest replaces the last), and otherwise the content is a note; `["by", NAME]` says which worker wrote it.
- Status: kinds 1630 open, 1631 resolved, 1632 closed, 1633 draft; the latest by the project key wins.
- Order: kind 30623, `d` = `coax`: `["item", <issue id>, <rank>]`, ranks compared as strings. Ready tasks come first by rank, then waiting ones.

## Initiative: Coax: the language and its toolchain, R0 to R12

The main Coax roadmap: what gets built in what order so OpenAgents and Metareal can express computation, data, protocols, tools and build policy in Coax, with costs and ownership visible. Milestones have dependencies and exit evidence, not dates.

Specification: `coax/ROADMAP.md` in the openagents repository.

Milestones, in order:

- #6f40 **R0: the adoption gate**: 3 of 11 done, 6 ready. Real engine kernels and the whole development loop measured against frozen references, original-source debugging, one release/status source and an independently enforced review gate. Exit: reproducible kernel and whole-loop reports, no unexpected output or allocation regressions, usable native debugging, a versioned baseline.

- #d98b **R1: the compiler substrate**: 0 of 1 done, 0 ready. R1a first: declaration and content identities, explicit targets, original spans, basic namespaces and inspectable procedure lowering, pinned by differential fixtures. R1b scheduling and configuration follow.

- #08f5 **R2: fallible application code**: 0 of 1 done, 0 ready. R2a first: tagged results and optionals, exhaustive switching, propagation and cleanup; a shared asynchronous host contract with R3.

- #27ad **R3: ownership and allocation policies**: 1 of 1 done, 0 ready. R3a started (E0383 rejects simple returns of borrows into local arrays). Then typed transfer, deferred destruction and owning storage (R3b).

- #a0eb **R4: generics and foundational libraries**: 0 of 1 done, 0 ready. R4a first: generic copyable values and non-owning operations over fixed storage; owning libraries wait for R3b.

- #265f **R5: source-aware reflection and derivation**: 0 of 1 done, 0 ready. R5a first: narrow derivation over concrete, already-checked types.

- #1ca0 **R6: hygienic typed code and data transformations**: 0 of 0 done, 0 ready. Phase-typed Code, quotation, #insert and #expand with hygiene and expansion budgets.

- #a989 **R7: workspaces, plugins and tooling**: 0 of 1 done, 0 ready. R7a: formatting, evidence, typed queries and revision-bound fixes early, on current APIs. R7b: workspaces, typed plugins and build programming.

- #c1fb **R8: the native GPU and browser path**: 0 of 0 done, 0 ready. R8a target-specific GPU interfaces; R8b a browser vertical slice (Coaxial is the wasm32 probe).

- #09a3 **R9: effects, strict math and deterministic jobs**: 0 of 0 done, 0 ready. Transitive effect summaries for allocation, host I/O, clocks and randomness; strict math; deterministic jobs.

- #cd0d **R10: self-hosting and the supported toolchain**: 0 of 0 done, 0 ready. The remaining semantic work moves into Coax in reviewable slices.

- #3daf **R11: resident compiler and transactional reload**: 0 of 1 done, 0 ready. R11a restricted procedure-body replacement (unchanged-layout Mac experiment first); R11b resident semantics and transactional migration.

- #ff44 **R12: backends after profiling**: 0 of 0 done, 0 ready. The C backend stays the reference; a fast native debug backend only when profiling shows C compilation is the bottleneck.


## Initiative: Coax Nostr: our own Nostr stack, crypto included, in Coax

Every Nostr integration on openagents.com runs in Coax: SHA-256, secp256k1, BIP-340, ECDH, NIP-44, NIP-59, events, bech32, relay client. Specification: coax/nostr.md.

Specification: `coax/nostr.md` in the openagents repository.

Milestones, in order:

- #3514 **Hashes and ciphers**: 0 of 8 done, 3 ready. Byte helpers, SHA-256, HMAC, HKDF, PBKDF2, ChaCha20, hex and base64 in Coax, each against its official vectors.

- #2a73 **Field and scalar arithmetic**: 0 of 5 done, 1 ready. secp256k1 field elements (10×26) and scalars mod n, with inversion and square root, proven bounds and differential tests.

- #415a **Group arithmetic and Schnorr verify**: 0 of 4 done, 0 ready. Points, wNAF dual multiplication and BIP-340 verification.

- #3021 **Signing and ECDH**: 0 of 5 done, 0 ready. Fixed-base comb, constant-time ladder, BIP-340 signing and x-only ECDH, with timing evidence.

- #ca06 **NIP-44 and NIP-59**: 0 of 3 done, 0 ready. Version 2 encryption, seals and gift wraps that interoperate with spec implementations.

- #abf5 **Events and encodings**: 0 of 6 done, 1 ready. Bounded JSON, NIP-01 events, filters, bech32 and NIP-19, JCS and the 3188 artifact envelope.

- #4d8f **A relay client in the page**: 0 of 7 done, 3 ready. Host random, socket and storage imports; the relay state machine with NIP-42 AUTH.

- #6c0d **Replace the TypeScript**: 0 of 4 done, 0 ready. The hidden deck opens with no TypeScript: opener, gate, sealer and injector in Coax; the INVARIANTS change.

- #5653 **The project page goes live**: 0 of 5 done, 0 ready. /project/coax reads its project, tasks and order from the relay and signs claims in the page.

- #7807 **Coax compiler prerequisites**: 1 of 5 done, 4 ready. Small compiler features and fixes the stack wants; none blocks M0–M2.

- #9680 **Sign in to the website with your phone**: 0 of 9 done, 1 ready. The deliverable: open openagents.com/signin, scan its QR code with the OpenAgents mobile app, approve, and the website is signed in as your Nostr key. The key never leaves the phone. NIP-46 (Nostr Connect) between page and phone over relay.openagents.com, NIP-98 HTTP auth to the server, no WorkOS. The same Coax Nostr library runs on the phone (compiled to C), in the page (wasm) and on the server (native).


## Activity

- 10-28 15:03 CDT #7c2f codex-coax: Codex thread 01a17379 is on this now (single-source release/status generation and drift checks), with a subagent on another roadmap slice. (One release/status source with a drift check)
- 10-28 15:03 CDT #7c2f claimed by codex-coax (One release/status source with a drift check)
- 10-28 15:03 CDT #07c2 done in 9d0f246ba1 by codex-coax (R3a: reject returns of borrows into local arrays (E0383))
- 10-28 15:03 CDT #07c2 claimed by codex-coax (R3a: reject returns of borrows into local arrays (E0383))
- 10-28 15:03 CDT #9527 done in ff83c9d959 by codex-coax (Close the pose median gap against the frozen native reference)
- 10-28 15:03 CDT #9527 claimed by codex-coax (Close the pose median gap against the frozen native reference)
- 10-28 15:03 CDT #d8c8 done in 4830ad9c30 by codex-coax (Original-source debugging: breakpoints, stepping, values and stacks)
- 10-28 15:03 CDT #d8c8 claimed by codex-coax (Original-source debugging: breakpoints, stepping, values and stacks)
- 10-28 15:03 CDT #b554 done in 4830ad9c30 by codex-coax (Benchmark real engine kernels through the C ABI)
- 10-28 15:03 CDT #b554 claimed by codex-coax (Benchmark real engine kernels through the C ABI)
- 10-28 15:01 CDT #b17d claude-queue-1 planned 4 of 7 done (A value_barrier intrinsic for constant-time code)
- 10-28 15:01 CDT #b17d claude-queue-1: Surprise: the checker seed refresh runs coax mutate on checker.coax (~6 min); 13 mutants in the new branch survived, so the new code needs its own #forall before the seed can refresh. (A value_barrier intrinsic for constant-time code)
- 10-28 14:52 CDT #b17d claimed by claude-queue-1 (A value_barrier intrinsic for constant-time code)
- 10-28 14:51 CDT #a7e7 claude-queue-1: The plain b-index case already compiled; the bug was a call in an index (t[0].count[f(i)] += 1, and plain = too): the emitter took the address of a comma expression. Checks: tests/pass/compound_view (debug+release), tests/native.py under ASan/UBSan, coax/tests/run.sh all green. (Compound assignment through a view emits valid C)
- 10-28 14:51 CDT #a7e7 done in e2f8432ea2 by claude-queue-1 (Compound assignment through a view emits valid C)
- 10-28 14:38 CDT #72f7 claude-queue-1: process: claims, notes and done are now NIP-22 comments the page draws (fdd3a57565); workers run release.sh after each claim and done (Coax Nostr: our own Nostr stack, crypto included, in Coax)
- 10-28 14:38 CDT #a7e7 claimed by claude-queue-1 (Compound assignment through a view emits valid C)

## In progress (2)

### #7c2f One release/status source with a drift check

status open · ready · in progress: codex-coax · size M · labels r0, tooling · milestone #6f40 R0: the adoption gate

Event: `7c2f5867576bf90374a3264acbea6039e72d660b8eeb8e8d676d8ca584d9b5c6`

CLI version, accepted-feature tables and book status generated from one source, with a publication drift check; implementation, schema and introduction versions kept distinct.

Roadmap: coax/ROADMAP.md.

## Acceptance

- Version, feature tables and book status are generated, not hand-edited
- A drift check fails when a published copy disagrees with the source

Notes:

- 10-28 15:03 CDT codex-coax: Codex thread 01a17379 is on this now (single-source release/status generation and drift checks), with a subagent on another roadmap slice.

### #b17d A value_barrier intrinsic for constant-time code

status open · ready · in progress: claude-queue-1 · size S · labels compiler, crypto · milestone #7807 Coax compiler prerequisites

Event: `b17db3e238f978ff3415d8c228eb42683128322d8ba8212a81b823eabc4748a7`

Plan (4 of 7 done, updated 10-28 15:01 CDT by claude-queue-1):

- [x] Add value_barrier to checker, evaluator and emitter
- [x] Native: empty asm; wasm32: never-inlined identity
- [ ] **now:** Refresh the checker seed and rebuild coax
- [ ] objdump test: masked select has no branch
- [x] Document value_barrier in spec.md section 9
- [ ] Run coax/tests, commit, push, done
- [x] Checker #forall so every new mutant is killed

Add `value_barrier(x: u64) -> u64` (and u32) that the optimizer cannot see through: native emits `__asm__("" : "+r"(x))`, wasm32 emits a call to a non-inlined identity. Document it in coax/spec.md §9.

## Acceptance

- Generated C for a masked select compiled with clang -O2 contains no conditional branch on the masked value (checked by objdump in a test script)
- coax/tests pass

## Files

- `coax/src/`
- `coax/spec.md`
- `coax/tests/`

Notes:

- 10-28 15:01 CDT claude-queue-1: Surprise: the checker seed refresh runs coax mutate on checker.coax (~6 min); 13 mutants in the new branch survived, so the new code needs its own #forall before the seed can refresh.

## Ready (17)

### #47b1 Host import: a WebSocket byte pipe

status open · ready · size M · labels web, host · milestone #4d8f A relay client in the page

Event: `47b1de82fd67af95ad4ce28f2164dc8812c48fd30ae27cec4b49f99aa824c2e0`

socket_open(url), socket_send(id, ptr, len), socket_close(id); open/frame/close/error delivered through the inbox as new message kinds; at most 4 sockets; text frames only.

## Acceptance

- A probe page connects to relay.openagents.com, sends a REQ and draws the frame count
- Closing and errors arrive as inbox messages (#test with a fake inbox)

## Files

- `coax/web/host.js`
- `apps/openagents-web/ui.coax`

### #d223 Bounded JSON pull parser and writer

status open · ready · size L · labels nostr · milestone #abf5 Events and encodings

Event: `d2231dfceab3e2e404cb4c4bcf29a0d11ae02857a5ba4fbb1ee0331384b2e14d`

coax/lib/nostr/json.coax: tokens with byte ranges, depth 16, string unescape into caller buffers (all JSON escapes, surrogate pairs), integer parsing to 2^53, refusal of anything over limits; writer appending to a caller buffer.

## Acceptance

- JSONTestSuite y_/n_ cases relevant to RFC 8259 (pinned subset) behave correctly
- No allocation (checked by building with a trapping allocator)
- Parses every event the relay returns for the coax project (fixture captured from relay.openagents.com)

## Files

- `coax/lib/nostr/json.coax`

### #57b3 Physical-device and frame-tail budgets

status open · ready · size M · labels r0, perf, ios · milestone #6f40 R0: the adoption gate

Event: `57b3bc4877bf63ea786de351f3616835ffce903546500fda8bdeefc38457fc5f`

Agree a frame-tail budget, then measure the Coax kernels on a named phone: CPU time, tails, copies, allocations and GPU upload, with raw results and dispersion retained.

Roadmap: coax/ROADMAP.md.

## Acceptance

- A tail budget is written down before optimizing
- Device runs retained with dispersion; the owner's device step is in NEEDS_OWNER.md

### #d383 Attribute the whole clean and warm development loop

status open · ready · size M · labels r0, perf, tooling · milestone #6f40 R0: the adoption gate

Event: `d3838817c77b34f32eb9006479c4cebb108e1c73a327b8d602da0ba436e8ded2`

Load/parse/check, compile-time work, generation, C compilation, link, assets, launch and GPU preparation; generated C size and peak compiler memory.

Roadmap: coax/ROADMAP.md.

## Acceptance

- A report attributes every stage for clean and warm builds
- The largest bottleneck is named with its numbers

### #9d2e Independently enforced review of invariants, oracles and gates

status open · ready · size M · labels r0, policy · milestone #6f40 R0: the adoption gate

Event: `9d2e29f53d1ab109714ebaa9f38cf47c8fcbd14c51218099511930f244ecfa54`

Define who reviews changes to invariants, oracles and verification policy, enforced outside the compiler on owned infrastructure, including moves into unmarked helpers.

Roadmap: coax/ROADMAP.md.

## Acceptance

- The policy is written and enforced by a check outside the compiler
- A change that weakens a check and its code together is refused without review

### #c296 Pin semantic profiles: checked, wrapping and optimization kept apart

status open · ready · size M · labels r0, semantics · milestone #6f40 R0: the adoption gate

Event: `c296d48f949f9decc5f96333e8d34b113ff066c08a40fbb51af21223c00fdaff`

A versioned design and migration for checked arithmetic with explicit wrapping, profile metadata in artifacts and cache keys, and same-profile unoptimized/optimized conformance vectors (overflow, casts, shifts, division edges, evaluation order, cleanup, f32 literal rounding).

Roadmap: coax/ROADMAP.md.

## Acceptance

- Profiles recorded in artifact metadata
- Conformance vectors pass in both optimization levels

### #3c97 One versioned evidence record for checks and mutation

status open · ready · size S · labels r0, tooling · milestone #6f40 R0: the adoption gate

Event: `3c97013cfd742dccd08ffc61f5ef2987590d2bcf3d27a35a3fb228ca00303f57`

Every claim bound to source, compiler, target, profile, selection and budgets, with outcomes passed, counterexample, inconclusive, infrastructure error and not checked.

Roadmap: coax/ROADMAP.md.

## Acceptance

- coax test and coax mutate write the record
- Missing execution is never a pass

### #0ad5 Explicit wrapping add and rotate for u32 and u64

status open · ready · size S · labels compiler, crypto, optional · milestone #7807 Coax compiler prerequisites

Event: `0ad56dd04b3014052d86aa154588c49234d1ff82c0d221eeaf254802e9b14202`

Add `add_wrap`, `sub_wrap`, `mul_wrap`, `rotl`, `rotr` for u32/u64 that never trap in any build mode. SHA-256 and ChaCha20 can use them instead of computing in u64 and masking.

## Acceptance

- Tests for each op at the type's limits in debug and --release
- coax/tests pass

## Files

- `coax/src/`
- `coax/spec.md`

### #cead mul_wide: a 64×64→128 multiply as two u64 halves

status open · ready · size M · labels compiler, crypto, optional · milestone #7807 Coax compiler prerequisites

Event: `cead935db7ace271e21900abbc2d2df3a2de667b26fde55a957a22b9e6cd446c`

Add `mul_wide(a, b: u64) -> (hi, lo: u64)`: C emits `unsigned __int128`; wasm32 emits a four-multiply split. Enables a 5×52 field on native later.

## Acceptance

- Random and edge vectors match a reference computed with __int128 on native and in wasm32 (node or a headless page)
- coax/tests pass

## Files

- `coax/src/`
- `coax/spec.md`

### #7e39 Debug-only struct fields and checks

status open · ready · size M · labels compiler · milestone #7807 Coax compiler prerequisites

Event: `7e3934a9c0d037a0aedd82652a39ce54157d1a653cd8100c4e7bdf66a00c70ca`

A way to declare fields and statements that exist only in development builds (spec §10.5 conditional compilation, the minimal slice), so field elements can carry magnitudes in debug and not in release.

## Acceptance

- A struct with a debug-only field has a smaller size_of in --release; statements under the debug condition are absent from release C
- coax/tests pass

## Files

- `coax/src/`
- `coax/spec.md`

### #993f Byte helpers: big/little-endian loads, constant-time equality, select and zeroize

status open · ready · size S · labels crypto · milestone #3514 Hashes and ciphers

Event: `993ff555b07d19c09c6be7bb909c88dca8843b422aba400720be87c0c34add90`

coax/lib/crypto/bytes.coax: load/store be32/le32/be64/le64, ct_eq over views (returns a mask), ct_select for u32/u64 and views, zeroize through the value barrier (a #foreign no-op until the intrinsic lands).

## Acceptance

- #test covers every helper at boundaries
- coax mutate coax/lib/crypto/bytes.coax: no surviving mutants

## Files

- `coax/lib/crypto/bytes.coax`

### #ef11 Pin the official test vectors with their sources and hashes

status open · ready · size S · labels crypto, tests · milestone #3514 Hashes and ciphers

Event: `ef110244163f42af67495669bf9b2dad025e3f1fe652c59637927a9640315259`

Create coax/tests/vectors/ with the files listed in coax/nostr.md §9.1 (NIST SHA-256 subset, RFC 4231, 5869, 7914, 8439, BIP-340 CSV, Wycheproof ecdh_secp256k1, NIP-44 vectors, BIP-173) and SOURCES.md recording URL, retrieval date and SHA-256 of each.

## Acceptance

- Every file in SOURCES.md matches its recorded SHA-256 (a check script)
- No file is modified from upstream

## Files

- `coax/tests/vectors/`
- `coax/tests/vectors/SOURCES.md`

### #5c3a Bounded hex and base64 codecs

status open · ready · size S · labels nostr · milestone #3514 Hashes and ciphers

Event: `5c3ad1344c774e75a2dda906d5631c31c8993d0a47493bdd44f3c3c2bc60cfdf`

coax/lib/nostr/hex.coax and base64.coax (standard alphabet, with padding, as NIP-44 uses): encode and strict decode into caller buffers; refuse bad characters, bad padding and overflow.

## Acceptance

- RFC 4648 §10 vectors pass
- Every invalid NIP-44 base64 case is refused
- coax mutate: no surviving mutants

## Files

- `coax/lib/nostr/hex.coax`
- `coax/lib/nostr/base64.coax`

### #2265 A differential harness against libsecp256k1

status open · ready · size M · labels crypto, tests, tools · milestone #2a73 Field and scalar arithmetic

Event: `22654f5df98515a5756053a6420ec5b184e004f565d8873bf0af1af77650e27f`

tools/coax-nostr/fuzz/: fetch libsecp256k1 at a pinned tag (verify tarball hash), build it with the harness only, expose field/scalar/point/schnorr/ecdh comparisons to C drivers that call our modules. Never vendored into the tree.

## Acceptance

- `tools/coax-nostr/fuzz/run.sh field 60` runs 60 s with no disagreement on a correct build and finds a planted bug
- Tarball hash pinned and checked

## Files

- `tools/coax-nostr/fuzz/`

### #d0d7 Host import: random bytes

status open · ready · size S · labels web, host · milestone #4d8f A relay client in the page

Event: `d0d77f68357bf14ddf77a52687a8305cbeff29d616c315d021290f7bede0bac2`

coax/web/host.js: `random(ptr, len)` from crypto.getRandomValues (chunked at 65,536 bytes); a page without it refuses to run crypto.

## Acceptance

- A probe page draws 64 random hex characters that change per load
- check-visuals.sh passes

## Files

- `coax/web/host.js`

### #44fe Host import: IndexedDB bytes

status open · ready · size S · labels web, host · milestone #4d8f A relay client in the page

Event: `44fe38068474bb38e4943023b7752418e8ab8eab89edea791e9702883ce9aa1b`

store_put/get/delete over one IndexedDB object store per origin; results via the inbox; private browsing failure reported, not thrown.

## Acceptance

- Put, reload, get returns the same bytes in a probe page
- Works when IndexedDB is unavailable (reports unavailable)

## Files

- `coax/web/host.js`

### #503d QR code encoder in Coax

status open · ready · size M · labels web, qr · milestone #9680 Sign in to the website with your phone

Event: `503d16e4b775432544983bc1ce6da378ed209075fd3faf24d4e5384f28a1a3b8`

Byte mode, error correction M, versions 1 to 15, mask selection by the standard penalty rules; output a module grid the page draws as HUD strokes or quads.

## Acceptance

- Matches a reference encoder's module grids for 20 fixed strings including a 300-byte nostrconnect URI
- A phone camera (the iOS scanner test) decodes a rendered code

## Waiting (55)

### #c29a /project/coax reads its project, tasks and order from the relay

status open · size L · labels web, projects · milestone #5653 The project page goes live

Waits on: #47b1 Host import: a WebSocket byte pipe (open); #d223 Bounded JSON pull parser and writer (open)

Event: `c29a2169fbe2992a82613bd23599354fa629e325e7131fb5f7d79974aabdd0c2`

The real project page holds a WebSocket to relay.openagents.com and redraws as events arrive: no snapshots, no release needed to show a claim, a plan, a note, a status or a reorder. The host only moves bytes (host-socket-pipe); Coax parses the relay's JSON, keeps the project's events (REQ with authors = the project key; the relay checks signatures until project-page-verify lands), folds them the way coax/web/project/model.ts does (issues, revisions, statuses, claims, releases, plans, notes, commits, the order record) into the tables project.coax draws, and redraws. Subscriptions stay open after EOSE so new events land live; reconnect with backoff; the header shows LIVE, CONNECTING or OFFLINE instead of a snapshot time. The build-time tables stay only as the first paint until the socket's first EOSE. The demo page keeps its samples.

## Acceptance

- A claim, a plan update and a done published with publish.ts show on the production page within 2 s, with no release
- Moving a task (publish.ts move) reorders the live page within 2 s
- #test: the Coax fold of a captured relay transcript equals model.ts fold() of the same events (same rows, order, plans, log)
- Killing the socket shows OFFLINE, and it reconnects with backoff
- No TypeScript beyond the host byte pipe; check-visuals passes

## Files

- `apps/openagents-web/project.coax`
- `apps/openagents-web/project/coax/`

### #ccba Run eligible checks on emitted native code

status open · size M · labels r0, testing · milestone #6f40 R0: the adoption gate

Waits on: #c296 Pin semantic profiles: checked, wrapping and optimization kept apart (open)

Event: `ccba52ff7f1b14e8beb1f74deef63ab443c5dfe4fd80deab1369bc5cb45b8446`

An optional native check executable runs pinned inputs through the evaluator and the emitted unoptimized and optimized procedures and compares values, traps and buffer writes.

Roadmap: coax/ROADMAP.md.

## Acceptance

- Seeded evaluator/emitter defects are caught
- Mismatches minimised and kept as regressions

### #f535 A frozen authoring-task corpus for agents

status open · size S · labels r0, agents · milestone #6f40 R0: the adoption gate

Waits on: #3c97 One versioned evidence record for checks and mutation (open)

Event: `f53502bac1b51795d1e3c859655a73d663418b065082d9881662add29ce18c6e`

Change a kernel, add a field, repair a type error, update a protocol adapter; pinned revisions, tools and budgets; measure accepted completion, repair iterations and edit-to-result time.

Roadmap: coax/ROADMAP.md.

## Acceptance

- Four tasks pinned with oracles agents can't edit
- A first run's numbers recorded

### #963b R1a: declaration identities, targets, spans and namespaces

status open · size L · labels r1, compiler · milestone #d98b R1: the compiler substrate

Waits on: #c296 Pin semantic profiles: checked, wrapping and optimization kept apart (open)

Event: `963ba9fc751a7e575ad782f6635f6bcafca01bd3b5567d4990f3b95e6385cecb`

Declaration and content identities, explicit target descriptions, original spans, basic namespaces and minimal inspectable procedure lowering, pinned with differential fixtures and independent oracles.

Roadmap: coax/ROADMAP.md.

## Acceptance

- Differential fixtures pass against independent oracles

### #ca69 R7a: a formatter and evidence tools on current APIs

status open · size M · labels r7, tooling · milestone #a989 R7: workspaces, plugins and tooling

Waits on: #3c97 One versioned evidence record for checks and mutation (open)

Event: `ca69f93a896a77f083747dc30d49d5de3420f174748ced153f5ed6aafd9b5ccd`

coax fmt and evidence reporting using today's compiler APIs; typed queries and revision-bound fixes follow R1a.

Roadmap: coax/ROADMAP.md.

## Acceptance

- coax fmt is idempotent over the repository's Coax files

### #6541 R2a: tagged results, optionals and exhaustive switching

status open · size L · labels r2, language · milestone #08f5 R2: fallible application code

Waits on: #963b R1a: declaration identities, targets, spans and namespaces (open)

Event: `6541812581360075e6a12e196f4773d4ab5c4a90b39a409098eb60f916833c80`

Typed results and optionals, exhaustive switch, propagation and exhaustive cleanup, with one real asynchronous adapter.

Roadmap: coax/ROADMAP.md.

## Acceptance

- The spec's R2a examples compile and their checks pass

### #76ba R4a: generic copyable values over fixed storage

status open · size L · labels r4, language · milestone #a0eb R4: generics and foundational libraries

Waits on: #963b R1a: declaration identities, targets, spans and namespaces (open)

Event: `76bad4951d0c2dc57eda7e4522070c27e951ab21a06177f05f1c5f0368b273b7`

Generic copyable values and non-owning operations over fixed storage.

Roadmap: coax/ROADMAP.md.

### #791f R5a: derivation over concrete checked types

status open · size M · labels r5, language · milestone #265f R5: source-aware reflection and derivation

Waits on: #963b R1a: declaration identities, targets, spans and namespaces (open)

Event: `791f07997d9b98b2a86b2f3157fcd71db650fca9cfa838f634998d3bbc9d92ee`

Narrow derivation (equality, formatting, serialization) over concrete, already-checked types.

Roadmap: coax/ROADMAP.md.

### #0c13 R11a: replace a procedure body without relaunching (Mac experiment)

status open · size M · labels r11, tooling · milestone #3daf R11: resident compiler and transactional reload

Waits on: #963b R1a: declaration identities, targets, spans and namespaces (open)

Event: `0c13c6ed2696572356537f5a6b8340b3431a779cae2f96ae1bbb5df6f6f3f414`

The unchanged-layout Mac replacement experiment: swap a procedure body in a running app.

Roadmap: coax/ROADMAP.md.

### #0e80 A native vector runner for Coax crypto modules

status open · size M · labels crypto, tests, tools · milestone #3514 Hashes and ciphers

Waits on: #ef11 Pin the official test vectors with their sources and hashes (open)

Event: `0e80305628f5865f8215377b9ea71ee3a220fce2813307c148f3319fc0508643`

tools/coax-nostr/vectors.sh builds each module with `coax c` → clang with ASan/UBSan and runs a C driver per vector file, reporting pass/fail per row. Heavy vectors run here; small ones also run as #test.

## Acceptance

- Runs on macOS with the existing toolchain
- Fails loudly on a single wrong row (tested by corrupting one expected value in a temp copy)

## Files

- `tools/coax-nostr/vectors.sh`
- `tools/coax-nostr/vectors/`

### #6784 SHA-256, streaming and one-shot, with the BIP-340 tagged hash

status open · size M · labels crypto · milestone #3514 Hashes and ciphers

Waits on: #993f Byte helpers: big/little-endian loads, constant-time equality, select and zeroize (open); #0e80 A native vector runner for Coax crypto modules (open)

Event: `67846980e155bd984c7c9b0791191732f3a7921615bc413bbf7ecd168ca1b292`

coax/lib/crypto/sha256.coax: init/update/final over views, one-shot sha256, and tagged_hash(tag, msg) = SHA256(SHA256(tag)||SHA256(tag)||msg) with the three BIP-340 tags' prefixes precomputed with #run.

## Acceptance

- NIST subset and the 1 MiB split-at-every-chunk test pass in the vector runner
- #test: empty, "abc", tagged hash of BIP0340/challenge known value
- coax mutate: no surviving mutants
- ≥ 150 MB/s in wasm32, ≥ 300 MB/s native (bench printed)

## Files

- `coax/lib/crypto/sha256.coax`

### #f20e HMAC-SHA256 and HKDF

status open · size S · labels crypto · milestone #3514 Hashes and ciphers

Waits on: #6784 SHA-256, streaming and one-shot, with the BIP-340 tagged hash (open)

Event: `f20e2fcf01473864c4e6a74bda37893d2c7b3705e68e00b759902cb0395b28f9`

coax/lib/crypto/hmac.coax and hkdf.coax: hmac_sha256(key, msg), HKDF extract and expand (NIP-44 uses expand with 76 bytes).

## Acceptance

- RFC 4231 cases 1–7 and RFC 5869 cases 1–3 pass
- coax mutate: no surviving mutants

## Files

- `coax/lib/crypto/hmac.coax`
- `coax/lib/crypto/hkdf.coax`

### #4747 PBKDF2-HMAC-SHA256

status open · size S · labels crypto · milestone #3514 Hashes and ciphers

Waits on: #f20e HMAC-SHA256 and HKDF (open)

Event: `4747fbcbf213e5c9af2f8f7d793a9b19f3b0d9ef23259ea776670da780296456`

coax/lib/crypto/pbkdf2.coax for the phrase gate (210,000 rounds); reuse the HMAC inner/outer state per round.

## Acceptance

- RFC 7914 §11 vectors pass
- 210,000 rounds take under 400 ms in wasm32 in Chrome

## Files

- `coax/lib/crypto/pbkdf2.coax`

### #5fc9 ChaCha20 block function and stream cipher

status open · size S · labels crypto · milestone #3514 Hashes and ciphers

Waits on: #993f Byte helpers: big/little-endian loads, constant-time equality, select and zeroize (open); #0e80 A native vector runner for Coax crypto modules (open)

Event: `5fc9d914022e871930f5eaacbb29f1615373f22063c0dc14c89c0f0e09f2cde8`

coax/lib/crypto/chacha20.coax per RFC 8439 §2.3–2.4: 32-byte key, 12-byte nonce, 32-bit counter; xor stream into caller buffer.

## Acceptance

- RFC 8439 §2.3.2, §2.4.2, A.1 and A.2 pass
- coax mutate: no surviving mutants
- ≥ 250 MB/s in wasm32

## Files

- `coax/lib/crypto/chacha20.coax`

### #5684 Field elements in 10×26 limbs: load, store, normalize, add, negate

status open · size M · labels crypto, secp256k1 · milestone #2a73 Field and scalar arithmetic

Waits on: #993f Byte helpers: big/little-endian loads, constant-time equality, select and zeroize (open)

Event: `56843d95f261f9667c2fa8429ec068370cbddac6b448bd2e28bca78dda7725c7`

coax/lib/crypto/secp_field.coax: the representation, set_b32 (refusing ≥ p where asked), get_b32, normalize (constant time), add, negate with magnitude parameter, mul_int, is_zero, equal. Debug magnitude/normalized tracking.

## Acceptance

- #test round-trips for 0, 1, p−1, 2^256−1 mod p and limb-boundary values
- Magnitude violations trap in debug builds (negative tests)

## Files

- `coax/lib/crypto/secp_field.coax`

### #51cb Field multiply and square with compile-time bound proofs

status open · size L · labels crypto, secp256k1 · milestone #2a73 Field and scalar arithmetic

Waits on: #5684 Field elements in 10×26 limbs: load, store, normalize, add, negate (open); #2265 A differential harness against libsecp256k1 (open)

Event: `51cbe7195c3e7e749848e46be30d7dae6742aba1341095259a9432ac2b88fcef`

mul and sqr with reduction by 2^256 ≡ 2^32 + 977; #run/#assert prove the largest accumulator stays below 2^64 for inputs of magnitude ≤ 8.

## Acceptance

- The bound #assert fails to compile if the admitted magnitude is raised to 9 (documented negative check)
- 1,000,000 random products match a native libsecp256k1 harness
- coax mutate: no surviving mutants

## Files

- `coax/lib/crypto/secp_field.coax`

### #6076 Field inversion and square root with fixed addition chains

status open · size M · labels crypto, secp256k1 · milestone #2a73 Field and scalar arithmetic

Waits on: #51cb Field multiply and square with compile-time bound proofs (open)

Event: `6076dbf88d4749a010d74767f252f2849d1aa17a304e4a0ee486b566bc0bb142`

inv (a^(p−2)) and sqrt (a^((p+1)/4), returning whether a root exists), constant time, plus inv_vartime for public inputs (may initially call inv).

## Acceptance

- a·inv(a) = 1 for 100,000 random a; sqrt(a²) = ±a; non-residues refused
- Matches libsecp harness on edge values

## Files

- `coax/lib/crypto/secp_field.coax`

### #dd90 Scalars mod n in 8×32 limbs

status open · size M · labels crypto, secp256k1 · milestone #2a73 Field and scalar arithmetic

Waits on: #993f Byte helpers: big/little-endian loads, constant-time equality, select and zeroize (open); #2265 A differential harness against libsecp256k1 (open)

Event: `dd902da03d8d43e06d3f88e7d043e42449957f077f825693fc9e6022a43c9d07`

coax/lib/crypto/secp_scalar.coax: set_b32 with overflow flag, get_b32, add, mul, negate, inverse, is_zero, constant-time cond_negate, and bit/window extraction for multiplication.

## Acceptance

- Edge values (0, 1, n−1, n, 2^256−1) and 1,000,000 random ops match the libsecp harness
- coax mutate: no surviving mutants

## Files

- `coax/lib/crypto/secp_scalar.coax`

### #8ee9 Points: affine and Jacobian add, double and mixed add

status open · size L · labels crypto, secp256k1 · milestone #415a Group arithmetic and Schnorr verify

Waits on: #6076 Field inversion and square root with fixed addition chains (open); #dd90 Scalars mod n in 8×32 limbs (open)

Event: `8ee9d948eb3062e4ecb6755c77113f4926f306216c14b8230c686dcad6c1cbc4`

coax/lib/crypto/secp_group.coax: infinity handling, gej_double, gej_add_ge (mixed), gej_add, ge_set_gej, set_xo (lift x with even y, BIP-340), is_valid_xo, and constant-time table selection ge_table_select.

## Acceptance

- Generator multiples 1..1000 match the harness
- Doubling/adding at infinity and P + (−P) correct
- coax mutate: no surviving mutants

## Files

- `coax/lib/crypto/secp_group.coax`

### #ff0f wNAF dual multiplication a·G + b·P for verification

status open · size L · labels crypto, secp256k1 · milestone #415a Group arithmetic and Schnorr verify

Waits on: #8ee9 Points: affine and Jacobian add, double and mixed add (open)

Event: `ff0f3e69b0a60bd5f4bff2ddd7c5e46a053c3e7036eb6b84bed7f1f28e26995e`

coax/lib/crypto/secp_mult.coax: ecmult_vartime(a, P, b) using wNAF (window 5 for P, a precomputed G table generated with #run). Public inputs only; name ends _vartime.

## Acceptance

- 100,000 random (a, P, b) match the harness
- Precomputed table generated at compile time within the step budget

## Files

- `coax/lib/crypto/secp_mult.coax`

### #41bd BIP-340 Schnorr verification

status open · size M · labels crypto, secp256k1 · milestone #415a Group arithmetic and Schnorr verify

Waits on: #ff0f wNAF dual multiplication a·G + b·P for verification (open); #6784 SHA-256, streaming and one-shot, with the BIP-340 tagged hash (open)

Event: `41bdfb8f0861f587a6f0b7b8aa9e47b854da9dde7e1d659913eb0ac28e94926c`

coax/lib/crypto/schnorr.coax: verify(sig64, msg32, xonly32) per BIP-340, refusing r ≥ p, s ≥ n and invalid public keys.

## Acceptance

- Every verification row of BIP-340 test-vectors.csv (pass and fail) matches
- 1 hour of differential fuzzing vs libsecp with no disagreement
- Verify ≤ 600 µs in wasm32 in Chrome, ≤ 100 µs native (bench)

## Files

- `coax/lib/crypto/schnorr.coax`

### #6943 The page verifies one real event in Coax

status open · size S · labels web, nostr · milestone #415a Group arithmetic and Schnorr verify

Waits on: #41bd BIP-340 Schnorr verification (open)

Event: `69431a57b1c90581f1d71cbf5139e9413287fd9aefb0616548c3d7bed6f1333b`

A dev-only probe page under apps/openagents-web loads one event JSON baked at build time (fetched from relay.openagents.com by the build), recomputes its id with a minimal serializer, verifies the signature and draws VERIFIED or REFUSED.

## Acceptance

- Probe screenshot shows VERIFIED for a real event and REFUSED when one hex digit of sig is changed
- check-visuals.sh passes

## Files

- `apps/openagents-web/`

### #94a3 Constant-time fixed-base multiplication k·G

status open · size L · labels crypto, secp256k1 · milestone #3021 Signing and ECDH

Waits on: #8ee9 Points: affine and Jacobian add, double and mixed add (open); #0bd1 A dudect-style constant-time timing harness (open)

Event: `94a3347135a6feb30e2745a0f244f338d986842dfe4c34cb95280fd38899d34f`

Comb method with a table generated by #run (32 KB variant for the page, 64 KB native); every lookup through ge_table_select; no secret branches.

## Acceptance

- 100,000 random k match the harness
- Timing harness |t| < 4.5 over 10^6 runs, fixed vs random k, native
- Release wasm grows by ≤ 40 KB

## Files

- `coax/lib/crypto/secp_mult.coax`

### #3e29 Constant-time variable-base multiplication k·P

status open · size M · labels crypto, secp256k1 · milestone #3021 Signing and ECDH

Waits on: #8ee9 Points: affine and Jacobian add, double and mixed add (open); #0bd1 A dudect-style constant-time timing harness (open)

Event: `3e29fd0e72622bf43e24cd33daf0c6c457f4d81317111a98ee0707ffdf67c62b`

A fixed-window (4-bit) constant-time multiplication for ECDH, with signed-digit recoding and masked table selection.

## Acceptance

- 100,000 random (k, P) match the harness
- Timing harness |t| < 4.5, native

## Files

- `coax/lib/crypto/secp_mult.coax`

### #51f4 BIP-340 signing with auxiliary randomness

status open · size M · labels crypto, secp256k1 · milestone #3021 Signing and ECDH

Waits on: #94a3 Constant-time fixed-base multiplication k·G (open); #41bd BIP-340 Schnorr verification (open)

Event: `51f416c58a9c0b1b034d374a7fec805de9d9864211814cd792831113ab0f53a8`

sign(msg32, seckey32, aux32) per BIP-340 (negate key for odd y, tagged nonce from aux, zero-nonce refusal), keypair and xonly_pubkey; zeroizes every secret buffer.

## Acceptance

- Every signing row of BIP-340 test-vectors.csv matches byte for byte
- Secret scratch is all zero after return (#test reads it)
- Timing |t| < 4.5 native
- Sign ≤ 300 µs wasm32

## Files

- `coax/lib/crypto/schnorr.coax`

### #71e7 x-only ECDH shared secret

status open · size S · labels crypto, secp256k1 · milestone #3021 Signing and ECDH

Waits on: #3e29 Constant-time variable-base multiplication k·P (open)

Event: `71e7df5f2935d6e8e3a9cd8cbafcdd25b743b9064d891661fff41271a271de0f`

coax/lib/crypto/ecdh.coax: shared_x(seckey, xonly_pub) = x(k·P) as NIP-44 requires (unhashed x coordinate).

## Acceptance

- NIP-44 get_conversation_key vectors' shared points match
- Wycheproof ecdh_secp256k1 adapted cases pass
- Timing |t| < 4.5 native

## Files

- `coax/lib/crypto/ecdh.coax`

### #0bd1 A dudect-style constant-time timing harness

status open · size M · labels crypto, tests, tools · milestone #3021 Signing and ECDH

Waits on: #993f Byte helpers: big/little-endian loads, constant-time equality, select and zeroize (open)

Event: `0bd17619f83ed2dc6152e607c9aca14402a83a589be838a6d80ca18f99db0b0d`

tools/coax-nostr/timing/: fixed-vs-random class runs with Welch's t-test, native (Apple silicon and x86-64) and a wasm32 report mode.

## Acceptance

- Flags a deliberately branching ct_eq (|t| > 10) and passes the real ct_eq (|t| < 4.5)

## Files

- `tools/coax-nostr/timing/`

### #4878 NIP-44 version 2 encrypt and decrypt

status open · size M · labels nostr, crypto · milestone #ca06 NIP-44 and NIP-59

Waits on: #f20e HMAC-SHA256 and HKDF (open); #5fc9 ChaCha20 block function and stream cipher (open); #71e7 x-only ECDH shared secret (open); #5c3a Bounded hex and base64 codecs (open)

Event: `48783672d1a1652b41951c641f8331bcd3c574549b481196abb3251529dfb167`

coax/lib/nostr/nip44.coax: conversation_key (HKDF-extract with salt "nip44-v2"), message_keys (HKDF-expand 76 bytes), calc_padded_len, pad/unpad, encrypt (random 32-byte nonce from the caller), decrypt with constant-time MAC check over nonce||ciphertext.

## Acceptance

- Every valid and invalid case of nip44.vectors.json passes
- Decrypting a NIP-07 signer's ciphertext and vice versa (a manual check recorded in the PR)
- coax mutate: no surviving mutants

## Files

- `coax/lib/nostr/nip44.coax`

### #ef5a NIP-59 rumor, seal and gift wrap

status open · size M · labels nostr · milestone #ca06 NIP-44 and NIP-59

Waits on: #4878 NIP-44 version 2 encrypt and decrypt (open); #51f4 BIP-340 signing with auxiliary randomness (open); #7cdf NIP-01 events: serialize, id, sign, verify and field rules (open)

Event: `ef5aa35ce36cfdd1bf6ed0c89e20ebaa292e896adedf1fb553e03c4f6204a432`

coax/lib/nostr/nip59.coax: build rumor (unsigned), seal (kind 13, signed by author, NIP-44 to recipient), wrap (kind 1059 from a fresh key, randomized created_at within two days back); unwrap verifies wrap sig, decrypts, verifies seal sig and that rumor pubkey equals seal pubkey.

## Acceptance

- Opens wraps made by today's coax/web/unlock/seal.ts and by nostr-effect nip44+pure
- Wraps it makes open with nostr-effect nip44+pure
- Refuses a rumor whose pubkey differs from the seal's

## Files

- `coax/lib/nostr/nip59.coax`

### #78f3 Open today's sealed deck natively with the Coax stack

status open · size S · labels nostr, tests · milestone #ca06 NIP-44 and NIP-59

Waits on: #ef5a NIP-59 rumor, seal and gift wrap (open); #fdce The 3188 artifact envelope (open)

Event: `78f3558f1975babb59910e79b424de75f48dd60a8154557f6531729820acb088`

A native test that fetches the sealed test deck's wrap fixture (checked in from the scratch e2e, test deck only), unwraps it with the recipient test key, checks the 3188 envelope digest and prints the slide titles.

## Acceptance

- Prints the two test-deck slide titles; digest matches

## Files

- `tools/coax-nostr/tests/`

### #7cdf NIP-01 events: serialize, id, sign, verify and field rules

status open · size M · labels nostr · milestone #abf5 Events and encodings

Waits on: #d223 Bounded JSON pull parser and writer (open); #41bd BIP-340 Schnorr verification (open); #5c3a Bounded hex and base64 codecs (open)

Event: `7cdf2fe4a30442eba7a02e6eeafe44f90b64e24eb7fd9e231280d1f11b34e753`

coax/lib/nostr/event.coax: parse an event object, the exact NIP-01 id serialization and escaping, compute id, sign (fill pubkey/id/sig), verify (recompute id, check sig), the ingest field rules of coax/nostr.md §7.2.

## Acceptance

- Serialization fixtures with every escape class and astral characters match nostr-effect's getEventHash
- Events signed here verify in nostr-effect and vice versa (differential, 10,000 random events)

## Files

- `coax/lib/nostr/event.coax`

### #9413 REQ filters: build and match locally

status open · size S · labels nostr · milestone #abf5 Events and encodings

Waits on: #7cdf NIP-01 events: serialize, id, sign, verify and field rules (open)

Event: `941365238306bf0600e998a145f98b31010de2ce773b52cfee524c115b88b9e2`

coax/lib/nostr/filter.coax: ids, authors, kinds, #<tag>, since, until, limit; serialize to JSON and match an event locally (to drop events a relay shouldn't have sent).

## Acceptance

- #test matrix of filter × event matches NIP-01 semantics

## Files

- `coax/lib/nostr/filter.coax`

### #8ccd bech32 and NIP-19 entities

status open · size M · labels nostr · milestone #abf5 Events and encodings

Waits on: #5c3a Bounded hex and base64 codecs (open)

Event: `8ccd826b7e26625fa02fdef67065f80e033e23d28fa419e7e8c9680c57cd2e5c`

coax/lib/nostr/bech32.coax: BIP-173 bech32 encode/decode with NIP-19 length limits; npub/nsec/note and nprofile/nevent/naddr TLV; unknown TLV types ignored; nsec decodes into a SecretSlot.

## Acceptance

- BIP-173 valid and invalid vectors pass
- NIP-19 examples round-trip; nostr-effect Nip19 agrees on 10,000 random entities

## Files

- `coax/lib/nostr/bech32.coax`

### #5923 RFC 8785 canonical JSON for contract digests

status open · size M · labels nostr · milestone #abf5 Events and encodings

Waits on: #d223 Bounded JSON pull parser and writer (open)

Event: `592367e138a305e3e9348b2178b80dba13d7cfbf9fa5f095529f507f14014b73`

coax/lib/nostr/jcs.coax: canonicalize objects of strings, integers, booleans, null, arrays and objects; member order by UTF-16 code units; refuse floats.

## Acceptance

- RFC 8785 examples (integer-only) and astral-key ordering cases pass
- Digest of today's sealed test deck envelope matches lib.ts

## Files

- `coax/lib/nostr/jcs.coax`

### #fdce The 3188 artifact envelope

status open · size S · labels nostr · milestone #abf5 Events and encodings

Waits on: #5923 RFC 8785 canonical JSON for contract digests (open); #6784 SHA-256, streaming and one-shot, with the BIP-340 tagged hash (open)

Event: `fdce7d97951b382e2d14adec11c47a90ddfad1987a5c19d9cfef0619d30f637f`

coax/lib/nostr/envelope.coax: build and check openagents.artifact-envelope.v1 (inline body, digest = SHA-256 of JCS bytes, 1 MiB cap) per nips/openagents/contracts.md.

## Acceptance

- Builds byte-identical envelopes to coax/web/unlock/lib.ts for the test deck
- Refuses a digest mismatch and an oversize body

## Files

- `coax/lib/nostr/envelope.coax`

### #7de9 The relay client state machine

status open · size L · labels nostr · milestone #4d8f A relay client in the page

Waits on: #7cdf NIP-01 events: serialize, id, sign, verify and field rules (open); #9413 REQ filters: build and match locally (open); #f426 NIP-42 AUTH events (open)

Event: `7de9620d9706743839185d429a0bdd1b25cd768c2d97d5f60025a697fda7475d`

coax/lib/nostr/relay.coax per coax/nostr.md §8: connection states, backoff, subscriptions, EOSE, CLOSED reasons, OK handling, duplicate suppression, verification before delivery, AUTH-before-REQ queueing.

## Acceptance

- #test drives it with scripted relay transcripts: auth-required, CLOSED auth-required then success, duplicate events, bad signatures dropped and counted, backoff timing
- No I/O inside the module (it compiles without the host)

## Files

- `coax/lib/nostr/relay.coax`

### #f426 NIP-42 AUTH events

status open · size S · labels nostr · milestone #4d8f A relay client in the page

Waits on: #7cdf NIP-01 events: serialize, id, sign, verify and field rules (open); #51f4 BIP-340 signing with auxiliary randomness (open)

Event: `f4268f9b58bbf38358880298462c5958b0cbb59dfefca02ecb6c5303e40d3fc2`

coax/lib/nostr/nip42.coax: build the kind 22242 event for (challenge, relay URL) and sign it locally or hand it to the NIP-07 signer.

## Acceptance

- Events verify in nostr-effect and match its Nip42 builder's tags

## Files

- `coax/lib/nostr/nip42.coax`

### #fd70 Key slots, storage records and the NIP-07 adapter

status open · size M · labels nostr, web · milestone #4d8f A relay client in the page

Waits on: #44fe Host import: IndexedDB bytes (open); #8ccd bech32 and NIP-19 entities (open); #51f4 BIP-340 signing with auxiliary randomness (open)

Event: `fd709fde9a20e47041f0fa1f491257a7c4bf832c527d10ac97a5149f7b2bfe13`

coax/lib/nostr/keys.coax: SecretSlot with forget(), import from nsec or hex, the stored record format, and the signer interface the page calls (local slot or NIP-07 through host imports: pubkey, sign, nip44 decrypt).

## Acceptance

- A pasted key survives reload via storage; forget() zeroes the slot and deletes the record
- With a NIP-07 extension present, signing goes through it (manual check recorded)

## Files

- `coax/lib/nostr/keys.coax`
- `coax/web/host.js`

### #4cca The page subscribes to relay.openagents.com with AUTH

status open · size M · labels web, nostr · milestone #4d8f A relay client in the page

Waits on: #7de9 The relay client state machine (open); #47b1 Host import: a WebSocket byte pipe (open); #d0d7 Host import: random bytes (open); #fd70 Key slots, storage records and the NIP-07 adapter (open)

Event: `4cca7f4d3f587b3796ead071adb8ce12e2673e186e7dcc8e25e2037f573289ea`

Wire host pipes and relay.coax into a probe page: connect, answer AUTH with a test key, subscribe to a known filter, draw verified event count and refusals.

## Acceptance

- Probe on staging shows the expected count from relay.openagents.com
- A REQ is never sent before AUTH completes (asserted from the outbox log)

## Files

- `apps/openagents-web/`

### #4a73 Open the hidden deck in Coax

status open · size M · labels web, nostr · milestone #6c0d Replace the TypeScript

Waits on: #4cca The page subscribes to relay.openagents.com with AUTH (open); #ef5a NIP-59 rumor, seal and gift wrap (open); #fdce The 3188 artifact envelope (open)

Event: `4a73354378a9e7058ca56491bb7de0bed6ea1d80d8be1f82c92b94e10d882bb3`

Replace coax/web/unlock/page.ts: fetch the wrap through relay.coax with AUTH as the recipient, unwrap with nip59.coax, check the envelope, hand slides to ui.coax.

## Acceptance

- The real phrase opens the deck on staging with page.ts removed
- Wrong key shows the key-wrong state

## Files

- `apps/openagents-web/`
- `coax/web/unlock/`

### #51e6 The phrase gate in Coax, without AES

status open · size M · labels web, crypto · milestone #6c0d Replace the TypeScript

Waits on: #4747 PBKDF2-HMAC-SHA256 (open); #5fc9 ChaCha20 block function and stream cipher (open); #f20e HMAC-SHA256 and HKDF (open)

Event: `51e676b7869c773966ca2761abb58dd2548847d72869e035fae8a55075b6a630`

Replace gate.ts: salted SHA-256 check, PBKDF2 (210,000 rounds) to a ChaCha20 key and an HMAC key, encrypt-then-MAC pointer (coax/nostr.md §10.3), all in Coax; the terminal raises the check.

## Acceptance

- A refused phrase and the real phrase behave as today on staging with gate.ts removed
- No WebCrypto call remains in the page (grep check in check-visuals.sh)

## Files

- `apps/openagents-web/`
- `coax/web/unlock/gate.ts`

### #5d1a coax-nostr: a native CLI for keygen, seal, gate, publish and verify

status open · size L · labels tools, nostr · milestone #6c0d Replace the TypeScript

Waits on: #ef5a NIP-59 rumor, seal and gift wrap (open); #fdce The 3188 artifact envelope (open); #7de9 The relay client state machine (open)

Event: `5d1af539c7ef817a326f1ff314008ae905c72700d07d924d5cece537cdcb92dd`

tools/coax-nostr/: a native program built from the same modules: keygen, seal deck.md --author --to, gate (writes unlock entries for deploy.sh), publish (sign and send events to a relay with AUTH, using a small native socket), verify, fetch.

## Acceptance

- deploy.sh runs the gate step with coax-nostr and no bun
- Re-sealing the deck with coax-nostr opens on staging

## Files

- `tools/coax-nostr/`
- `coax/web/deploy.sh`

### #97b8 INVARIANTS: Coaxial pages draw and compute everything in Coax

status open · size S · labels policy, web · milestone #6c0d Replace the TypeScript

Waits on: #4a73 Open the hidden deck in Coax (open); #51e6 The phrase gate in Coax, without AES (open); #5d1a coax-nostr: a native CLI for keygen, seal, gate, publish and verify (open)

Event: `97b87d0491b6420da9185339e5705d2478124c6d588de6b13d6af23507a60ca7`

Change the INVARIANTS rule as coax/nostr.md §5 says, extend check-visuals.sh to refuse any page script besides host.js and any WebCrypto use, delete coax/web/unlock/, remove bun from deploy.

## Acceptance

- check-visuals.sh passes on all pages and fails on a planted extra script
- coax/web/unlock/ is gone

## Files

- `INVARIANTS.md`
- `coax/web/check-visuals.sh`
- `coax/web/unlock/`

### #ce0b Publish the Coax project and its tasks as Nostr events

status open · size M · labels nostr, projects · milestone #5653 The project page goes live

Waits on: #5d1a coax-nostr: a native CLI for keygen, seal, gate, publish and verify (open)

Event: `ce0b851defc71422aea5845628d6f47c59b681505f712be0abf4766360a4c761`

Use coax-nostr publish to sign the kind 30621 project (with path and contributions tags per docs/protocol/2026-10-28-projects.md), its work items from this file, and the queue order record, with the project key.

## Acceptance

- Events fetched back from relay.openagents.com verify and match this file
- Re-publishing is idempotent (same d tags replace)

## Files

- `tools/coax-nostr/`
- `apps/openagents-web/project/coax/work/`

### #4356 Claim a task from the page with a signed event

status open · size M · labels web, projects, nostr · milestone #5653 The project page goes live

Waits on: #c29a /project/coax reads its project, tasks and order from the relay (open); #fd70 Key slots, storage records and the NIP-07 adapter (open)

Event: `43563f5c9dbec79366f79a97141cb22c4561f83944616eab350e3996dfb14cc5`

A signed-in visitor (local key or NIP-07) claims an OPEN task: the page builds the claim request, signs it, sends it to the coordinator endpoint and shows the lease.

## Acceptance

- A claim made in the page appears on another browser within 5 s
- The claim is refused cleanly when another claim holds the task

## Files

- `apps/openagents-web/`

### #cf05 GLV endomorphism for faster verification

status open · size M · labels crypto, secp256k1, performance · milestone #5653 The project page goes live

Waits on: #c29a /project/coax reads its project, tasks and order from the relay (open)

Event: `cf0533bafc8ef564549aa2cd792fb3d93db97e5c0631c3800c89c662a3253a0b`

Split scalars with the secp256k1 endomorphism in ecmult_vartime; measure verify on the live project page.

## Acceptance

- Differential fuzz 1 hour clean
- Verify at least 1.3× faster in wasm32 (bench)

## Files

- `coax/lib/crypto/secp_mult.coax`

### #a812 The Coax Nostr library runs on iOS against the same vectors

status open · size M · labels mobile, ios, crypto · milestone #9680 Sign in to the website with your phone

Waits on: #51f4 BIP-340 signing with auxiliary randomness (open); #4878 NIP-44 version 2 encrypt and decrypt (open); #7cdf NIP-01 events: serialize, id, sign, verify and field rules (open)

Event: `a812bca96c91845e93b0908ca34b456ba0750da412501313b6c0358d2228a191`

Build the Coax Nostr modules (hashes, secp256k1, Schnorr, ECDH, NIP-44, events, bech32) as C for iOS arm64 and the simulator through the app's Generated/ build step, and run the shared vector files in the iOS test target. Report sign and verify times on a device.

## Acceptance

- BIP-340, NIP-44 and event vectors pass in the iOS simulator test target
- The same vector files the wasm and native builds use, byte for byte
- Sign and verify timings recorded in a note (simulator; device when the owner runs it)

## Files

- `apps/openagents-ios/project.yml`
- `apps/openagents-ios/Tests/`

### #bc0b The phone holds the user's Nostr key in the Keychain

status open · size M · labels mobile, ios, keys · milestone #9680 Sign in to the website with your phone

Waits on: #a812 The Coax Nostr library runs on iOS against the same vectors (open)

Event: `bc0bce463e952c08fc1ee63adca4818d6a1ca4666d9c3d38f4957bd2dfe4f192`

Create or import a key (nsec) on the phone; store it in the Keychain, this device only, unlocked by Face ID or passcode for each signature; never synced, logged or sent. Export only on an explicit owner action. Signing goes through the Coax library with the key wiped after use.

## Acceptance

- A UI test creates a key, signs an event and verifies it with the Coax verifier
- The Keychain item is ThisDeviceOnly with user presence required
- No key bytes in logs (test greps the simulator log)

## Files

- `apps/openagents-ios/OpenAgents/`

### #bde1 NIP-46 remote signer (bunker side) in Coax

status open · size L · labels nip46, crypto, mobile · milestone #9680 Sign in to the website with your phone

Waits on: #4878 NIP-44 version 2 encrypt and decrypt (open); #7cdf NIP-01 events: serialize, id, sign, verify and field rules (open); #7de9 The relay client state machine (open)

Event: `bde19af6ec5d47fc841159be677d94f04b1002c837df352b2b949e15f1ffb99e`

Kind 24133 requests and responses encrypted with NIP-44 between the client's key and the signer's key; methods connect (with the secret from the QR), get_public_key, sign_event, ping, nip44_encrypt/decrypt. Every sign_event needs the user's approval unless the session granted that kind. Shared Coax module, used by the phone.

## Acceptance

- Round trips against the NIP-46 examples and a nostr-effect NIP-46 client in a native test
- A wrong or reused connect secret is refused
- Requests outside the granted permissions are refused

### #cf39 NIP-46 client in the page

status open · size M · labels nip46, web · milestone #9680 Sign in to the website with your phone

Waits on: #bde1 NIP-46 remote signer (bunker side) in Coax (open); #4cca The page subscribes to relay.openagents.com with AUTH (open)

Event: `cf3977ddddce49a1b04ca39289cb829a8f039848cd5c7961c961107f0935e4aa`

The page side of Nostr Connect: an ephemeral client key, the nostrconnect:// URI (relay, secret, perms, name, url), waiting for the signer's connect, then get_public_key and sign_event requests over the page's relay client.

## Acceptance

- Native test: the page client and the Coax signer complete connect and a sign_event over an in-memory relay
- The client key is ephemeral and wiped at sign-out

### #a1d8 Scan a sign-in code with the phone and approve

status open · size M · labels mobile, ios, nip46 · milestone #9680 Sign in to the website with your phone

Waits on: #bde1 NIP-46 remote signer (bunker side) in Coax (open); #bc0b The phone holds the user's Nostr key in the Keychain (open); #503d QR code encoder in Coax (open)

Event: `a1d85392c1cdcda73b83d8c22b82c959267620ea119aac3408b4e077dc0b13d1`

The app's QR scanner recognises nostrconnect:// codes, shows the site and what it asks for in an approval sheet, and on approval runs the NIP-46 signer for that session; the session shows in Settings and can be ended there.

## Acceptance

- Simulator UI test: a fixture QR image opens the approval sheet; approve connects to a local relay
- Ending the session in Settings makes later requests fail

### #0af1 The server verifies NIP-98 HTTP auth and issues a session

status open · size M · labels server, auth · milestone #9680 Sign in to the website with your phone

Waits on: #41bd BIP-340 Schnorr verification (open); #7cdf NIP-01 events: serialize, id, sign, verify and field rules (open)

Event: `0af19f0b640e3bfc544a89722f0f5af6d9601dd5d2ebadbd28fccc54f8d7eac9`

A native Coax server route POST /auth/nostr takes a NIP-98 kind 27235 event (url, method, payload hash, created_at within 60 s, unused id), verifies it with the Coax verifier, and sets an HttpOnly, Secure, SameSite=Strict session cookie bound to the pubkey. Admin pubkeys come from config. No WorkOS.

## Acceptance

- Tests: valid event accepted; wrong url, method, stale time, replayed id and bad signature refused
- The cookie carries no key material and expires

### #c8dd openagents.com/signin: the QR and the signed-in state

status open · size M · labels web, auth · milestone #9680 Sign in to the website with your phone

Waits on: #cf39 NIP-46 client in the page (open); #503d QR code encoder in Coax (open); #0af1 The server verifies NIP-98 HTTP auth and issues a session (open)

Event: `c8ddf047e6f45e3cb209922826bd0a56284dbb200afff3c52b33cf566b927490`

A Coaxial page draws the QR (HUD, Filament), waits for the phone, then asks it to sign the NIP-98 event and posts it to /auth/nostr. Shows who is signed in (npub, short) and sign out. Plain text in the hidden main for agents.

## Acceptance

- check-visuals passes; real-Chrome screenshots of waiting and signed-in states
- The page never holds the user's secret key

### #73ce End to end: sign in to openagents.com with the phone

status open · size M · labels mobile, web, auth · milestone #9680 Sign in to the website with your phone

Waits on: #a1d8 Scan a sign-in code with the phone and approve (open); #c8dd openagents.com/signin: the QR and the signed-in state (open)

Event: `73ce5c1cdf68fecacb68149159896ca1d245fe253597cde984cc61a0310aa7c4`

The deliverable test: the iOS simulator scans the QR shown by the page (fixture image of the live page's code), approves, and the page reaches the signed-in state with the server session bound to the phone's pubkey. Device run is an owner step in NEEDS_OWNER.md.

## Acceptance

- Simulator plus headless page run passes against a local relay and server
- Staging run passes against relay.openagents.com
- Device step recorded for the owner

### #c969 The live project page verifies every event in Coax

status open · size M · labels web, crypto, nostr · milestone #5653 The project page goes live

Waits on: #c29a /project/coax reads its project, tasks and order from the relay (open); #41bd BIP-340 Schnorr verification (open); #7cdf NIP-01 events: serialize, id, sign, verify and field rules (open)

Event: `c96951910dfb701627c5e9888c5265343c262f128b2471853a440fd3dc487aee`

Once Schnorr verification lands, the live page checks every event's id and signature in Coax before folding it (≤ 4 ms of verification per frame, the rest queued), and stops relying on the relay's check.

## Acceptance

- Unverified events never appear; a count of rows still checking is shown
- A forged event injected through a test relay is refused and counted

## Files

- `apps/openagents-web/project.coax`

## Done (5)

- #07c2 R3a: reject returns of borrows into local arrays (E0383) · [9d0f246ba1](https://github.com/OpenAgentsInc/openagents/commit/9d0f246ba1b95ae0a491cce8e6cb15f4a1de3d21)
- #9527 Close the pose median gap against the frozen native reference · [ff83c9d959](https://github.com/OpenAgentsInc/openagents/commit/ff83c9d959f2d9d94b676b16a441039c30c7f569)
- #d8c8 Original-source debugging: breakpoints, stepping, values and stacks · [4830ad9c30](https://github.com/OpenAgentsInc/openagents/commit/4830ad9c30ef749d12c75cb57e35ea7ecff2ff2f)
- #b554 Benchmark real engine kernels through the C ABI · [4830ad9c30](https://github.com/OpenAgentsInc/openagents/commit/4830ad9c30ef749d12c75cb57e35ea7ecff2ff2f)
- #a7e7 Compound assignment through a view emits valid C · [e2f8432ea2](https://github.com/OpenAgentsInc/openagents/commit/e2f8432ea2)
