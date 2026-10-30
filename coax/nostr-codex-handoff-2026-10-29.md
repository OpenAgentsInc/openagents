# Coax Nostr epic: Codex stopping handoff, 2026-10-29

The owner asked for a stopping point. **The Nostr migration is not complete:**
not every live web surface uses Coax Nostr. The project page can use a Coax
UI, but its complete verified event admission is still open. The API still
uses the existing C NIP-98 verifier; project/agent publication still has C or
TypeScript paths; encrypted presentation and signing have not been cut over.
The signed [live project queue](https://openagents.com/projects/coax) and
`coax/nostr.md` are the current work map. Re-read `AGENTS.md`,
`INVARIANTS.md` and `coax/web/project/QUEUE.md` before taking work.

The project page reads queue events live; **do not run a site release just
for claims, notes or completions**. Refresh the Markdown snapshot with
`coax/web/project/publish-data.sh coax` after the queue changes. Work from a
fresh `origin/main` worktree, use signed claim/plan/note/done events, release
claims when blocked, do not touch other workers' files, and add no sats.
One task has a two-hour limit; each command has a 15-minute limit. Retain
passing results for unchanged inputs rather than repeating them on unrelated
remote commits. Do not assume all historical handoff entries remain current.

## Shipped this run

- `73b250c8013f3ca79557d798d5e957e50d1ffcd1` and follow-up
  `f8f707650e905a38ff20e1e2f5bf5aa80292e217` fix project-page completed-task history. The actual project page
  was staged and released. `863ff6699de37f8c5a3e7ae4b2fe18cd8fe31583`
  fixes project detail line breaks; it passed focused native, sanitizer,
  Wasm and TypeScript tests and was released as `/build/project.85b69688e318.wasm`.
  The golden selector did not select the project page, so the actual project
  Wasm was also built and rendered in headless Chrome; this was a startup and
  rendering check, not verified relay admission. Unrelated Everglade and Fairy
  Forest pages were held back by their visual checks.
- `3e87b74ec6f172b9aa3642edf5b899d1f6397ed3` adds an untrusted,
  source-preserving NIP-MP project protobuf profile and descriptor-exclusion
  support. Protoc and Coax native/Wasm codec checks passed, with old descriptor
  snapshots preserved. This profile is not live admission and does not verify
  signatures or grant repository authority. #4d3e remains open.
- #5869 is done at `4d3008c0fec82c75c5cc9e4e989c1e34bf395347`, with
  1,723/1,723 full-reference mutants and its protoc/iOS regressions. #ef04 is
  done at `6b2631d0078e3b16f3c0b5d76dac2f80c5b74647`: opt-in Nostr kind
  range, old descriptor compatibility, 1,751/1,751 full-reference mutants,
  native/Wasm/protoc checks and an iOS simulator app build. See the matching
  records under `docs/verification/`.
- `a3d51d32a3520b3dba928976ecb974890e53a29e` inventories the actual live web
  surfaces and adds queue tasks for API public-verifier parity and cutover,
  API and agent publishing, and final surface retirement. Production signing
  paths retain their existing security dependencies.
- `09f48db7ccabe6e18f03d614542dab522a0d9dd5` records partial #092b Mac caller
  revalidation. Affected public correctness fixtures passed across the
  reported native, Wasm and simulator coverage. The ChaCha release Wasm
  throughput gate **failed**, 81.93 MB/s against 250 MB/s, under a heavily
  loaded host; the cause is unproven and #092b remains open. Do not repeat
  passing suites or infer timing/constant-time approval. Details and exact
  identities are in `docs/verification/2026-10-29-load32-total.md`.
- `ebbcf15ae8eab23f9528e75a6717eecb1a1e65b1` adds an independently reviewed,
  untrusted frame-rewrite primitive, with 347/347 reference mutants and
  checked/release native, sanitizer and Wasm fixtures. It is not verified
  store admission, tombstone handling, trusted recovery or #f539 completion.
- `cbc956f2452d3895d3fc04af41835f6acce5dae0` adds a checked public JSON probe
  for the future API bridge, **not linked to the deployed API**. Local
  experimental linking passed 923 service checks, but the Linux image build
  exceeded 900 seconds; the experimental integration was not shipped.
- `e51ccde315362ec3d5e031dc675213dd2e4ba228` and
  `ce925d6ffa3a49adab41bdb287354a79b998f1cb` add independently reviewed
  public Nostr event and legacy NIP-98 request fixtures. They expose differences
  including duplicate fields/tags, present-empty payload and input bounds;
  they are not production request parity. See
  `docs/verification/2026-10-29-api-event-parity.md`.
- `b7ee71bd423fe50ba5f36ee4f930b90e991aa5b8` adds a non-authoritative
  Coax request-field observer. It passed 6 checks, 92/92 reference mutants,
  21 native checked/release signed-fixture comparisons, 14 observer-only
  Wasm cases per mode, and sanitizer checks. The composition uses test-only
  Python for tag extraction and body hashing, not a production adapter.
  `docs/verification/2026-10-29-nip98-observation.md` states the differences.
- `718d00aac01369a93f2e1714f9209b7eb62f80c8` fixes the existing API deploy
  script masking failed Cloud Run deploy commands. Focused offline tests
  exercise the actual script and show a failed staging deploy stops before
  health polling and production promotion. This alone does not prove immutable
  image identity or prevent stale-health success.
- `53626fdf5dae31ed8d927051fe4223aeb8cce191` adds a separately reviewed
  C-image deploy guard: a tested detached source snapshot, digest-pinned
  staging/production deployment, project/region/UID and revision/traffic
  checks, and service/revision-bound health identity. The frozen local API
  regression passed 954 service checks and offline rollout/snapshot fixtures.
  A build-only C Cloud Build succeeded; no actual Cloud Run rollout or API
  deployment was performed. The first full API test attempt was invalid, as
  its script was edited while running; only the frozen rerun passed. See
  `docs/verification/2026-10-30-api-deploy-identity.md` for exact evidence.
  This is not an authenticated Coax compiler artifact or Coax Nostr cutover.

## Important boundaries

- Automatic approval review rejected the #4828 signer source-model
  investigation as a potential cybersecurity risk. Do not retry, rephrase,
  delegate, or otherwise bypass that rejection. #89db falls in the same
  source-bound signer self-verification investigation. Public verification and
  unrelated functional work do not confer approval of production secret paths.
- Do not conflate Coax's event parsing, protobuf projections, or local
  field observations with signature verification, authorization, replay
  protection, session management, or a deployed end-to-end path.
- The independently owned phone key custody, sign-in challenge/link endpoints,
  `/signin` and TestFlight work should not be taken over without coordination.
- `coax/provenance.sh` explicitly records consistency, **not authentication**.
  A locally checked compiler and its unsigned hash record do not authenticate
  its publisher. A controlled build, artifact identity, immutable image digest,
  source/toolchain binding and actual rollout verification remain necessary.

## Open work and saved evidence

- #f96d (`svc-coax-in-api`) remains open: link the Coax probe into a real API
  image, establish a controlled authenticated compiler artifact, test the
  actual digest-based staging rollout, then perform the requested live cutover.
  The original 900-second Cloud Build timeout and local Mac service results
  are documented in `coax/web/api/COAX.md`. A same-Debian full bootstrap passed
  under the cap on frozen commit `ff8752c7882bcc6d5b0dff042f339d080693d8eb`,
  Linux compiler input fingerprint
  `c79bc96bfad61fa8ac096cda2a3ab985c76453b99b2f6e6b2402ba10852a9d79` and
  compiler SHA-256
  `284113bc0b7f0306d7af0b472485079b8cb365cb5ebb3d4db53b71df5c889580`.
  That local artifact is **not authenticated for production**. Evidence and an
  unintegrated guard live in the local handoff
  `/Users/christopherdavid/.codex/handoffs/nostr-api-linux-bootstrap-20261029/README.md`.
  The guard needed service/revision-bound health, project/region/UID binding,
  bounded reads, strict snapshots, and actual integration; the recorded review
  and queue notes distinguish prototype work from enforcement.
- #db7e remains open for actual source-bound Coax NIP-98 request parity,
  API integration, replay and authorization. Existing C and Coax parsing
  differ; changing production behavior requires explicit compatibility and
  security review. The user-facing API has not been migrated.
- #4d3e is released open. Its source-preserving MP adapter is **not** on
  main and its full mutation gate did not pass. The local archive
  `/Users/christopherdavid/.codex/handoffs/nostr-mp-20261029.tar.gz` has
  SHA-256 `d637f30924e147e40f57173143ccfadc6b9210bbfad77fcaf7329a0df691bf5f`;
  its adjacent README records exact source identities, partial passing and
  failing records, restore steps and the remaining generated iterator mutant.
  Do not merge old shard records into a different test graph.
- #0845 remains open for complete verified project admission. The project page
  display fixes above do not verify relay events.
- #f539 remains open for verified store admission, tombstones, and safe
  recovery. Partial verified-incoming work and completed shard records are in
  `/Users/christopherdavid/.codex/handoffs/nostr-dedup-f539-20261029/`.
- #092b needs the failed ChaCha release-Wasm throughput test on a demonstrably
  quiet host with original inputs; a passing test is not a security approval.
  The preserved worktree is `/private/tmp/codex-nostr-mac-092b`. The narrow
  existing unittest method also runs its checked-Wasm portion; no release-only
  option currently exists. Record host load and actual samples, and do not
  waive the threshold or infer why the original run was slow.

## Final worker updates

- `0d89face3e8918ff7ae8fb0a1162f5b80871fa84` adds the bounded,
  non-authoritative NIP-98 tag extractor. It passed 28 Coax checks,
  1,072/1,072 full-reference mutants with no skipped cases, checked/release
  native and Wasm fixtures, and ASan/UBSan. Its `valid` flag covers structural
  and selected-field extraction, not complete Unicode admission: unknown
  strings containing escaped lone surrogates can pass this isolated parser.
  A verified original event, separately checked duplicate policy, body hashing,
  replay and authorization are still required. The experimental source-bound
  wrapper's valid-signature Coax checks failed with `E0400` at the fixed
  20,000,000-step per-test cap; that wrapper was not shipped. The exact wrapper,
  fixtures, failed log and successful/failing mutation evidence are recorded
  in `docs/verification/2026-10-29-nip98-tags.md`. #db7e is released open.
- #4d3e is released open after the MP adapter's mutation gate exposed
  generated-reader survivors. The final diagnostic identified the generated
  `MP_Project_iter_members` error-path `done` behavior; the final full gate
  has not run, and no incomplete adapter was pushed. Use the local archive
  linked above; reconcile source/tests and never merge shards from different
  input graphs.
- #f96d is released open. The C-image deployment integrity code at
  `53626fdf5dae31ed8d927051fe4223aeb8cce191` is on main, but no actual
  digest-based Cloud Run rollout, Coax image or API deployment was completed.
  The authenticated artifact, real staging verification and Coax bridge
  remain necessary before the web Nostr cutover.
