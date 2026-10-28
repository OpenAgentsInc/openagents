# Coax Nostr epic: handoff

On 2026-10-28 the owner moved the whole Coax Nostr epic (the `nostr`
initiative on https://openagents.com/projects/coax) to the Codex chat that
reviewed it (`codex://threads/01a1841b-45df-7ab0-ba74-2b5b3fc3d900`). That
chat owns the plan, the work file and the queue order from here. The Claude
queue worker has stopped. The live-page worker (`claude-live-1`) finishes
#c29a and then stops.

## Current execution update

`codex-nostr` owns the taskmaster and root worker loop. Other active workers are
`codex-nostr-codecs`, `codex-nostr-profiles` and `codex-nostr-vectors`; use the live
queue, not the historical lists below, to avoid duplicate claims. The shared
checkout stays untouched; isolated worktrees rebase/push only their own commits.

Since the original handoff: ct_view now reachesE0409 safely (`cfc36f3ab7`);
page capacity is256; TS pagination and Coax bounded catch-up no longer mistake
short capped responses for complete history. Vectors, native vector runner,
profiles, byte helpers, wrapping intrinsics, SHA-256, hex/base64, Bech32/NIP-19,
entropy, random import and byte storage have shipped in scoped slices. Structural
JSON admission is shipped (`a165c9bc26`); the existing decoder mutation closure
is being closed separately. Actual signed queue comments/plans/statuses and
`coax/nostr.md` record evidence and remaining gates.

Read-only event verification is still distinct from signing. Full profiles,
production secret approval, independent review, native/iOS/Wasm target evidence
and domain authority integration remain open until their own checks pass. No
amounts or rewards have been added. Test only affected changes; an unrelated
remote commit does not justify repeating the compiler/app suites.

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
